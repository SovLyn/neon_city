//! Deterministic, single-threaded simulation. Rendering may call `advance(0.0, ..)`
//! to obtain a complete initial frame; large resume deltas never trigger catch-up storms.
mod actors;
mod signs;
mod weather;

use crate::contract::*;
use glam::{Mat4, Vec3};

pub const FIXED_DT: f32 = 1.0 / 60.0;
const MAX_STEPS: usize = 8;

#[derive(Clone)]
pub(super) struct Rng(u64);
impl Rng {
    fn new(seed: u64, system: u64, id: u64) -> Self {
        Self(mix(seed ^ mix(system) ^ mix(id)))
    }
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        ((mix(self.0) >> 40) as f32) / 16777216.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}
fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

pub struct AnimationState {
    instances: Vec<InstanceDesc>,
    last_world: Vec<Mat4>,
    submitted: bool,
    lights: Vec<LightGpu>,
    actors: Vec<actors::Actor>,
    signs: signs::Signs,
    sign_lights: Vec<(usize, Vec<usize>)>,
    weather: weather::Weather,
    time: f64,
    accumulator: f64,
}
impl AnimationState {
    pub fn new(scene: &SceneCpu, seed: u64) -> Self {
        Self {
            instances: scene.instances.clone(),
            last_world: scene.instances.iter().map(|i| i.world_from_local).collect(),
            submitted: false,
            lights: scene.lights.clone(),
            actors: actors::build(scene, seed),
            signs: signs::Signs::new(scene, seed),
            sign_lights: scene
                .animation_bindings
                .iter()
                .filter(|binding| binding.kind == AnimationKind::Sign)
                .filter_map(|binding| {
                    let instance = scene.instances.iter().find(|i| i.id == binding.instance)?;
                    Some((
                        instance.material as usize,
                        binding.light_indices.iter().map(|&i| i as usize).collect(),
                    ))
                })
                .collect(),
            weather: weather::Weather::new(scene, seed),
            time: 0.0,
            accumulator: 0.0,
        }
    }

    /// At most eight fixed ticks per call. Excess wall time is deliberately discarded.
    /// Actor transforms interpolate continuously; previous matrices always refer to
    /// the last submitted render frame, not the last simulation tick.
    pub fn advance(&mut self, dt_s: f32, out: &mut DynamicFrame) {
        let dt = if dt_s.is_finite() { dt_s.max(0.0) } else { 0.0 };
        self.accumulator += f64::from(dt.min(FIXED_DT * MAX_STEPS as f32));
        let tick = f64::from(FIXED_DT);
        for _ in 0..MAX_STEPS {
            if self.accumulator + 1e-9 < tick {
                break;
            }
            self.accumulator = (self.accumulator - tick).max(0.0);
            self.time += tick;
            self.signs.step(FIXED_DT);
            self.weather.step(FIXED_DT);
        }
        let time = (self.time + self.accumulator) as f32;
        let mut worlds: Vec<Mat4> = self.instances.iter().map(|i| i.world_from_local).collect();
        out.lights.clone_from(&self.lights);
        for actor in &self.actors {
            let world = actor.world(time);
            worlds[actor.index] = world;
            let relative = world * self.instances[actor.index].world_from_local.inverse();
            for &light in &actor.lights {
                if let (Some(base), Some(output)) =
                    (self.lights.get(light), out.lights.get_mut(light))
                {
                    output.position = relative
                        .transform_point3(Vec3::from_array(base.position))
                        .to_array();
                    output.direction = relative
                        .transform_vector3(Vec3::from_array(base.direction))
                        .normalize_or_zero()
                        .to_array();
                }
            }
        }
        // Use the exact raster/ray material multiplier. Start from immutable base
        // colours each submission: neither fixed-step nor render frequency may
        // compound the attenuation. Intensity remains the scene's base intensity.
        for (material, lights) in &self.sign_lights {
            let Some(state) = self.signs.states.get(*material) else {
                continue;
            };
            for &index in lights {
                if let (Some(base), Some(output)) =
                    (self.lights.get(index), out.lights.get_mut(index))
                {
                    output.color = std::array::from_fn(|channel| {
                        base.color[channel] * state.emission_multiplier[channel]
                    });
                    output.intensity = base.intensity;
                }
            }
        }
        out.instances.clear();
        out.ray_objects.clear();
        for (index, (desc, world)) in self.instances.iter().zip(&worlds).enumerate() {
            let last = self.last_world[index];
            // A traffic wrap is a teleport, not a 120-metre motion vector.
            let teleported = world.w_axis.truncate().distance(last.w_axis.truncate()) > 30.0;
            let previous = if !self.submitted || teleported {
                *world
            } else {
                last
            };
            let normal = world.inverse().transpose();
            out.instances.push(InstanceGpu {
                world: world.to_cols_array_2d(),
                prev_world: previous.to_cols_array_2d(),
                normal_columns: [
                    normal.x_axis.truncate().extend(0.0).to_array(),
                    normal.y_axis.truncate().extend(0.0).to_array(),
                    normal.z_axis.truncate().extend(0.0).to_array(),
                ],
                mesh_id: desc.mesh,
                material_id: desc.material,
                instance_id: desc.id,
                flags: desc.flags,
            });
            if desc.flags & 1 != 0 && out.ray_objects.len() < 32 {
                out.ray_objects.push(DynamicObjectGpu {
                    world_from_local: world.to_cols_array_2d(),
                    local_from_world: world.inverse().to_cols_array_2d(),
                    primitive_kind: 0,
                    material_id: desc.material,
                    instance_id: desc.id,
                    flags: desc.flags,
                });
            }
        }
        self.last_world = worlds;
        self.submitted = true;
        out.particles.clone_from(&self.weather.particles);
        out.ripples.clone_from(&self.weather.ripples);
        out.reactive_material_ids = self.signs.reactive();
    }

    /// Upload this same MaterialId-indexed array to both raster and ray-tracing passes.
    pub fn material_states(&self) -> Vec<MaterialStateGpu> {
        self.signs.states.clone()
    }
}
