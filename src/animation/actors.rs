use super::*;

pub(super) struct Actor {
    pub index: usize,
    pub lights: Vec<usize>,
    base: Mat4,
    kind: AnimationKind,
    speed: f32,
    phase: f32,
    period: f32,
    seed: u64,
}
pub(super) fn build(scene: &SceneCpu, seed: u64) -> Vec<Actor> {
    scene
        .animation_bindings
        .iter()
        .filter_map(|binding| {
            if binding.kind == AnimationKind::Sign {
                return None;
            }
            let index = scene
                .instances
                .iter()
                .position(|i| i.id == binding.instance && i.flags & 1 != 0)?;
            let mut rng = Rng::new(seed ^ binding.seed, 1, u64::from(binding.instance));
            Some(Actor {
                index,
                lights: binding.light_indices.iter().map(|&i| i as usize).collect(),
                base: scene.instances[index].world_from_local,
                kind: binding.kind,
                speed: rng.range(5.0, 12.0),
                phase: rng.range(0.0, std::f32::consts::TAU),
                period: rng.range(3.0, 11.0),
                seed: mix(seed ^ binding.seed ^ u64::from(binding.instance)),
            })
        })
        .collect()
}
impl Actor {
    pub fn world(&self, time: f32) -> Mat4 {
        let base_pos = self.base.w_axis.truncate();
        let offset = match self.kind {
            AnimationKind::Vehicle => {
                let direction = if base_pos.x < 0.0 { 1.0 } else { -1.0 };
                let z =
                    (base_pos.z + self.speed * time * direction + 60.0).rem_euclid(120.0) - 60.0;
                Vec3::new(0.0, 0.0, z - base_pos.z)
            }
            AnimationKind::Drone => {
                let clock = time / self.period;
                let segment = clock.floor() as u64;
                // Half of each independent state is a hover, followed by a smooth move.
                let fraction = ((clock.fract() - 0.45) / 0.55).clamp(0.0, 1.0);
                let blend = fraction * fraction * (3.0 - 2.0 * fraction);
                let target = |n| {
                    let mut rng = Rng::new(self.seed, 2, n);
                    Vec3::new(
                        rng.range(-2.5, 2.5),
                        rng.range(-0.5, 1.5),
                        rng.range(-4.0, 4.0),
                    )
                };
                target(segment).lerp(target(segment + 1), blend)
                    + Vec3::new(
                        (time * 0.7 + self.phase).sin() * 0.12,
                        (time * 1.3 + self.phase).sin() * 0.16,
                        0.0,
                    )
            }
            AnimationKind::Sign => Vec3::ZERO,
        };
        Mat4::from_translation(offset) * self.base
    }
}
