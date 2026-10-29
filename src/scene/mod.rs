use crate::contract::*;
use glam::{Mat4, Vec3};

const DYNAMIC: u32 = 1;
const DOUBLE_SIDED: u32 = 2;
const TRANSPARENT: u32 = 4;
const CAR_MESH: u32 = 2;
const DRONE_MESH: u32 = 3;
const DETAIL_MESH_START: u32 = 4;
// Append-only material ABI: 19..43 remain the catalog sign materials.
const WINDOW_DARK: u32 = 43;
const WINDOW_COOL: u32 = 44;
const WINDOW_WARM: u32 = 45;
const FACADE_FRAME: u32 = 46;
const AWNING_PINK: u32 = 47;
const AWNING_CYAN: u32 = 48;
const ROAD_PAINT: u32 = 49;
const PRACTICAL_LAMP: u32 = 50;
const CAR_PAINT_START: u32 = 51;

pub struct SceneBuilder {
    config: SceneConfig,
    cursor: u32,
    total: u32,
    scene: SceneCpu,
    done: bool,
}

impl SceneBuilder {
    pub fn new(config: SceneConfig) -> Self {
        let buildings = config.buildings_per_side.max(1).saturating_mul(2);
        let total = buildings.saturating_mul(21).saturating_add(24);
        let mut scene = SceneCpu::default();
        scene.meshes.push(unit_box()); // Stable primitive ABI: mesh zero is unit box.
        scene.meshes.push(puddle_ellipse_mesh()); // Shared wet-ground ellipse, mesh one.
        // Bounded shared raster geometry; no extra instances, lights or animation bindings.
        scene.meshes.push(car_mesh());
        scene.meshes.push(drone_mesh());
        for category in 0..10 {
            scene.meshes.push(detail_mesh(category));
        }
        scene.materials = materials();
        let seed = config.seed;
        scene.emitters = vec![
            emitter(
                EmitterKind::Rain,
                [0.0, 12.0, 0.0],
                [0.0, -1.0, 0.0],
                180.0,
                seed,
                1,
            ),
            emitter(
                EmitterKind::Steam,
                [-5.0, 0.0, -16.0],
                [-0.1, 1.0, 0.0],
                0.12,
                seed,
                2,
            ),
            emitter(
                EmitterKind::Steam,
                [5.2, 0.0, 11.0],
                [0.15, 1.0, 0.0],
                0.10,
                seed,
                3,
            ),
            emitter(
                EmitterKind::Steam,
                [-5.4, 0.0, 39.0],
                [0.0, 1.0, 0.1],
                0.08,
                seed,
                4,
            ),
            emitter(
                EmitterKind::Ripple,
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                2.0,
                seed,
                5,
            ),
        ];
        // Continuous practicals and distant atmospheric sources; bounded below 64.
        for (i, z) in [-42.0, -14.0, 14.0, 42.0].into_iter().enumerate() {
            scene.lights.push(light(
                [0.0, 7.0, z],
                if i % 2 == 0 {
                    [0.22, 0.72, 1.0]
                } else {
                    [0.62, 0.48, 1.0]
                },
                22.0,
                90.0,
                0,
                i as u32,
            ));
        }
        Self {
            config,
            cursor: 0,
            total,
            scene,
            done: false,
        }
    }

    pub fn step(&mut self, work_items: u32) -> Result<BuildPoll<SceneCpu>, SceneError> {
        if self.done {
            return Ok(BuildPoll::Ready(self.scene.clone()));
        }
        let end = self
            .cursor
            .saturating_add(work_items.max(1))
            .min(self.total);
        while self.cursor < end {
            let i = self.cursor;
            let buildings = self.config.buildings_per_side.max(1).saturating_mul(2);
            if i < buildings {
                self.add_building(i);
            } else {
                self.add_dressing_sign_or_vehicle(i - buildings);
            }
            self.cursor += 1;
        }
        if self.cursor == self.total {
            self.add_ground_and_route();
            self.done = true;
            Ok(BuildPoll::Ready(self.scene.clone()))
        } else {
            Ok(BuildPoll::Pending {
                progress: self.cursor as f32 / self.total as f32,
            })
        }
    }

    fn add_building(&mut self, index: u32) {
        let per = self.config.buildings_per_side.max(1);
        let side_index = index / per;
        let along_index = index % per;
        let (side, z) = building_anchor(&self.config, side_index, along_index);
        let h = 11.0 + hash01(self.config.seed, index as u64) * 13.0;
        let w = 10.0 + hash01(self.config.seed, 100 + index as u64) * 2.0;
        let depth = self.config.street_length_m / per as f32 * 0.94;
        let x = side * (self.config.street_width_m * 0.5 + w * 0.5 + 0.45);
        add_instance(
            &mut self.scene,
            0,
            0,
            Vec3::new(x, h * 0.5, z),
            Vec3::new(w, h, depth),
            0,
        );
        self.add_facade(index, side, z, h, depth);
        // Rear/setback volume creates distinct silhouettes without expanding mesh count.
        if index % 3 == 1 {
            add_instance(
                &mut self.scene,
                0,
                0,
                Vec3::new(x - side * 1.2, h + 3.0, z + depth * 0.12),
                Vec3::new(w * 0.68, 6.0, depth * 0.65),
                0,
            );
        }
    }

    fn add_facade(&mut self, index: u32, side: f32, z: f32, h: f32, depth: f32) {
        // Four material batches per building, not one instance per pane/frame.
        // At most 42 boxes: 12 panes, 24 frame/mullion parts, four courses, two canopy parts.
        let mut batches: [MeshData; 4] = std::array::from_fn(|_| MeshData::default());
        let rows = (((h - 8.0) / 3.0).floor() as usize + 1).min(4);
        let pitch = depth * 0.26;
        let pane_width = (pitch * 0.65).min(1.7);
        for row in 0..rows {
            let floor_pitch = ((h - 9.2) / (rows - 1) as f32).min(3.0);
            let y = 8.2 + row as f32 * floor_pitch;
            for col in 0..3 {
                let dz = (col as f32 - 1.0) * pitch;
                // One warm pane and two unlit panes per six; cool remains dominant.
                let phase = (derive_seed(self.config.seed, 0x9000 + index as u64) % 6) as usize;
                let state = (row * 3 + col + phase) % 6;
                let batch = if state == 0 {
                    2
                } else if state <= 2 {
                    0
                } else {
                    1
                };
                mesh_box(
                    &mut batches[3],
                    Vec3::new(0.0, y, dz),
                    Vec3::new(0.06, 1.85, pane_width + 0.16),
                );
                mesh_box(
                    &mut batches[batch],
                    Vec3::new(-side * 0.045, y, dz),
                    Vec3::new(0.035, 1.65, pane_width),
                );
                mesh_box(
                    &mut batches[3],
                    Vec3::new(-side * 0.075, y, dz),
                    Vec3::new(0.025, 1.65, 0.055),
                );
            }
            mesh_box(
                &mut batches[3],
                Vec3::new(-side * 0.03, y - 1.05, 0.0),
                Vec3::new(0.10, 0.10, depth * 0.94),
            );
        }
        let origin = Vec3::new(street_facing_x(&self.config, side, 0.10), 0.0, z);
        for (batch, mesh) in batches.into_iter().enumerate() {
            if mesh.indices.is_empty() {
                continue;
            }
            let mesh_id = self.scene.meshes.len() as u32;
            self.scene.meshes.push(mesh);
            add_instance(
                &mut self.scene,
                mesh_id,
                [WINDOW_DARK, WINDOW_COOL, WINDOW_WARM, FACADE_FRAME][batch],
                origin,
                Vec3::ONE,
                0,
            );
        }
        // Sign panels occupy y=5..6.8: canopy and valance stay entirely below 4.25.
        let canopy_material = if index % 2 == 0 {
            AWNING_PINK
        } else {
            AWNING_CYAN
        };
        add_instance(
            &mut self.scene,
            0,
            canopy_material,
            Vec3::new(street_facing_x(&self.config, side, 0.85), 4.10, z),
            Vec3::new(1.65, 0.16, depth * 0.86),
            0,
        );
        add_instance(
            &mut self.scene,
            0,
            canopy_material,
            Vec3::new(street_facing_x(&self.config, side, 1.63), 3.95, z),
            Vec3::new(0.09, 0.30, depth * 0.86),
            0,
        );
    }

    fn add_dressing_sign_or_vehicle(&mut self, n: u32) {
        let building_count = self.config.buildings_per_side.max(1).saturating_mul(2);
        if n < 24 {
            self.add_sign(n);
        } else {
            self.add_dressing(n - 24);
        }
        debug_assert!(n < 24 + building_count * 20);
    }

    fn add_sign(&mut self, n: u32) {
        let entry = sign_catalog()
            .into_iter()
            .find(|entry| entry.region_id == n)
            .expect("catalog region is present");
        let kind = entry.kind;
        let building_count = self.config.buildings_per_side.max(1).saturating_mul(2);
        let building = n % building_count;
        let (side, center_z) = building_anchor(
            &self.config,
            building / self.config.buildings_per_side.max(1),
            building % self.config.buildings_per_side.max(1),
        );
        let depth =
            self.config.street_length_m / self.config.buildings_per_side.max(1) as f32 * 0.94;
        let end_offset = (depth * 0.5 - 1.9 - 0.1).max(0.0).min(2.2);
        let z = center_z + if n % 2 == 0 { -end_offset } else { end_offset };
        let x = facade_surface_x(&self.config, side) - side * 0.05;
        // Independent YZ quad; the 3.8m x 1.8m panel matches the atlas's ~2.1:1 rect.
        let mesh = quad_mesh(side);
        let mesh_id = self.scene.meshes.len() as u32;
        self.scene.meshes.push(mesh);
        let mat = 19 + n;
        let id = add_instance(
            &mut self.scene,
            mesh_id,
            mat,
            Vec3::new(x, 5.0, z - 1.9),
            Vec3::new(1.0, 1.8, 3.8),
            0,
        );
        self.scene.signs.push(SignDesc {
            instance: id,
            text: entry.text.to_string(),
            kind,
            color_linear: entry.color_linear,
            intensity: entry.intensity,
            seed: derive_seed(self.config.seed, entry.seed),
        });
        self.scene.animation_bindings.push(AnimationBinding {
            instance: id,
            kind: AnimationKind::Sign,
            light_indices: Vec::new(),
            seed: derive_seed(self.config.seed, 0x1000 + n as u64),
        });
        if self.scene.lights.len() < 64 {
            let li = self.scene.lights.len() as u32;
            let col = self.scene.signs.last().unwrap().color_linear;
            self.scene.lights.push(light(
                [street_facing_x(&self.config, side, 0.08), 5.5, z],
                col,
                7.0,
                self.scene.signs.last().unwrap().intensity * 0.35,
                0,
                id,
            ));
            if let Some(b) = self.scene.animation_bindings.last_mut() {
                b.light_indices.push(li);
            }
        }
    }

    fn add_dressing(&mut self, n: u32) {
        let per = self.config.buildings_per_side.max(1);
        let building = n / 20;
        let ordinal = n % 20;
        let (side, center_z) = building_anchor(&self.config, building / per, building % per);
        let front_x = street_facing_x(&self.config, side, 0.35);
        let along = (ordinal as f32 - 9.5) * 0.72;
        let z = center_z + along;
        let category = ordinal % 10;
        let (mat, mut pos, scale) = match category {
            0 => (9, Vec3::new(front_x, 2.8, z), Vec3::new(0.42, 0.55, 0.72)), // HVAC condenser
            1 => (10, Vec3::new(front_x, 5.2, z), Vec3::new(0.12, 4.8, 0.12)), // drain pipe
            2 => (11, Vec3::new(front_x, 7.8, z), Vec3::new(0.09, 0.08, 1.25)), // cable run
            3 => (12, Vec3::new(front_x, 6.2, z), Vec3::new(0.10, 0.62, 0.92)), // louver vent
            4 => (13, Vec3::new(front_x, 1.25, z), Vec3::new(0.55, 1.0, 0.58)), // vending machine
            5 => (
                14,
                Vec3::new(side * (self.config.street_width_m * 0.5 + 1.0), 0.55, z),
                Vec3::new(0.48, 0.85, 0.42),
            ), // refuse bin
            6 => (15, Vec3::new(front_x, 4.1, z), Vec3::new(0.035, 1.05, 0.78)), // runoff stain
            7 => (16, Vec3::new(front_x, 3.4, z), Vec3::new(0.025, 0.38, 0.28)), // sticker cluster
            8 => (17, Vec3::new(front_x, 8.7, z), Vec3::new(0.22, 0.22, 0.22)), // junction box
            _ => (18, Vec3::new(front_x, 9.5, z), Vec3::new(0.08, 1.2, 0.08)), // conduit
        };
        let safe_z = safe_detail_z(
            &self.config,
            &self.scene,
            building,
            center_z,
            pos.y,
            scale.y,
            z,
            scale.z,
            ordinal,
        );
        pos.z = safe_z;
        add_instance(
            &mut self.scene,
            DETAIL_MESH_START + category,
            mat,
            pos,
            scale,
            0,
        );
    }

    fn add_ground_and_route(&mut self) {
        // Damp asphalt main road, localized puddles and dry raised sidewalks.
        add_instance(
            &mut self.scene,
            0,
            2,
            Vec3::new(0.0, -0.18, 0.0),
            Vec3::new(
                self.config.street_width_m,
                0.35,
                self.config.street_length_m,
            ),
            0,
        );
        self.add_puddles();
        self.add_road_markings_and_practicals();
        for side in [-1.0, 1.0] {
            add_instance(
                &mut self.scene,
                0,
                2,
                Vec3::new(side * 2.1, 0.015, 0.0),
                Vec3::new(2.2, 0.03, self.config.street_length_m),
                0,
            );
            add_instance(
                &mut self.scene,
                0,
                3,
                Vec3::new(side * 9.4, 0.18, 0.0),
                Vec3::new(5.2, 0.35, self.config.street_length_m),
                0,
            );
        }
        // Small, per-building shopfront glazing; never a continuous wall across the block.
        let per = self.config.buildings_per_side.max(1);
        for building in 0..self.config.buildings_per_side.saturating_mul(2) {
            let (side, center_z) = building_anchor(&self.config, building / per, building % per);
            let x = street_facing_x(&self.config, side, 0.08);
            for dz in [-2.0, 2.0] {
                add_instance(
                    &mut self.scene,
                    0,
                    4,
                    Vec3::new(x, 2.0, center_z + dz),
                    Vec3::new(0.035, 2.8, 1.6),
                    DOUBLE_SIDED | TRANSPARENT,
                );
            }
        }
        // Approved raster/proxy deviation: cars and drones use composite silhouettes.
        // The analytic ray proxy intentionally remains the SAME world-transform unit box,
        // conservatively enclosing the normalized raster mesh (including its empty gaps).
        // Mesh zero stays canonical for buildings; do not synchronize new geometry to rays.
        for i in 0..8 {
            let id = add_instance(
                &mut self.scene,
                CAR_MESH,
                CAR_PAINT_START + i % 3,
                Vec3::new(
                    if i % 2 == 0 { -3.5 } else { 3.5 },
                    0.55,
                    -45.0 + i as f32 * 12.0,
                ),
                Vec3::new(2.0, 1.1, 4.1),
                DYNAMIC,
            );
            let lights = self.add_vehicle_lights(id);
            self.scene.animation_bindings.push(AnimationBinding {
                instance: id,
                kind: AnimationKind::Vehicle,
                light_indices: lights,
                seed: derive_seed(self.config.seed, 0x2000 + i as u64),
            });
        }
        for i in 0..4 {
            let id = add_instance(
                &mut self.scene,
                DRONE_MESH,
                0,
                Vec3::new(
                    -6.0 + i as f32 * 4.0,
                    8.0 + (i % 2) as f32 * 2.0,
                    -30.0 + i as f32 * 20.0,
                ),
                Vec3::splat(0.7),
                DYNAMIC,
            );
            self.scene.animation_bindings.push(AnimationBinding {
                instance: id,
                kind: AnimationKind::Drone,
                light_indices: Vec::new(),
                seed: derive_seed(self.config.seed, 0x3000 + i as u64),
            });
        }
        debug_assert!(self.scene.lights.len() <= 64);
    }

    fn add_road_markings_and_practicals(&mut self) {
        let half_length = self.config.street_length_m * 0.5;
        let mut paint = MeshData::default();
        let mut z = -half_length + 3.0;
        while z < half_length - 3.0 {
            if (z + 24.0).abs() > 4.0 && (z - 24.0).abs() > 4.0 {
                // Asphalt top is -0.005; paint bottom touches it, top is +0.007.
                mesh_box(
                    &mut paint,
                    Vec3::new(0.0, 0.001, z),
                    Vec3::new(0.14, 0.012, 2.1),
                );
            }
            z += 4.5;
        }
        for crossing in [-24.0_f32, 24.0] {
            if crossing.abs() + 3.0 >= half_length {
                continue;
            }
            for stripe in 0..9 {
                let x = -5.6 + stripe as f32 * 1.4;
                // Existing wet lane strips reach y=0.03 at |x|=1..3.2.
                // Split stripes at their edges so no paint floats above bare road.
                let lo = x - 0.4;
                let hi = x + 0.4;
                let edges = [lo, -3.2, -1.0, 1.0, 3.2, hi];
                let mut cuts: Vec<f32> =
                    edges.into_iter().filter(|v| *v >= lo && *v <= hi).collect();
                cuts.sort_by(f32::total_cmp);
                cuts.dedup();
                for pair in cuts.windows(2) {
                    let cx = (pair[0] + pair[1]) * 0.5;
                    let base = if (1.0..3.2).contains(&cx.abs()) {
                        0.03
                    } else {
                        -0.005
                    };
                    mesh_box(
                        &mut paint,
                        Vec3::new(cx, base + 0.006, crossing),
                        Vec3::new(pair[1] - pair[0], 0.012, 2.8),
                    );
                }
            }
        }
        let mesh = self.scene.meshes.len() as u32;
        self.scene.meshes.push(paint);
        add_instance(&mut self.scene, mesh, ROAD_PAINT, Vec3::ZERO, Vec3::ONE, 0);
        for (index, z) in [-42.0, -14.0, 14.0, 42.0].into_iter().enumerate() {
            let side = if index % 2 == 0 { -1.0 } else { 1.0 };
            let pole_x = street_facing_x(&self.config, side, 5.0);
            add_instance(
                &mut self.scene,
                0,
                FACADE_FRAME,
                Vec3::new(pole_x, 3.9, z),
                Vec3::new(0.14, 7.1, 0.14),
                0,
            );
            add_instance(
                &mut self.scene,
                0,
                FACADE_FRAME,
                Vec3::new(pole_x * 0.5, 7.4, z),
                Vec3::new(pole_x.abs() + 0.3, 0.12, 0.14),
                0,
            );
            add_instance(
                &mut self.scene,
                0,
                FACADE_FRAME,
                Vec3::new(0.0, 7.25, z),
                Vec3::new(1.4, 0.22, 0.65),
                0,
            );
            let id = add_instance(
                &mut self.scene,
                0,
                PRACTICAL_LAMP,
                Vec3::new(0.0, 7.10, z),
                Vec3::new(1.15, 0.08, 0.48),
                0,
            );
            self.scene.lights[index].source_id = id;
        }
    }

    fn add_puddles(&mut self) {
        // Bounded shallow-water islands: 36 deterministic, irregular ellipses, not continuous strips.
        for i in 0..36u64 {
            let side = if i % 2 == 0 { -1.0 } else { 1.0 };
            let width = 3.2 + hash01(self.config.seed, 0x5200 + i) * 2.0;
            // Keep the full conservative ellipse AABB street-side of the 6.8m curb.
            // Do not lift water onto the raised sidewalk to hide an overlap.
            let max_center_x = (6.75 - width * 0.5).min(5.1);
            let x = side * (3.0 + hash01(self.config.seed, 0x5000 + i) * (max_center_x - 3.0));
            let z = -self.config.street_length_m * 0.5
                + hash01(self.config.seed, 0x5100 + i) * self.config.street_length_m;
            let length = 3.0 + hash01(self.config.seed, 0x5300 + i) * 3.0;
            add_instance(
                &mut self.scene,
                1,
                1,
                Vec3::new(x, 0.002, z),
                Vec3::new(width, 1.0, length),
                0,
            );
        }
    }

    fn add_vehicle_lights(&mut self, id: u32) -> Vec<u32> {
        let mut indices = Vec::new();
        let vehicle_world = self.scene.instances[id as usize].world_from_local;
        let forward = if vehicle_world.w_axis.x < 0.0 {
            1.0
        } else {
            -1.0
        };
        for dx in [-0.28, 0.28] {
            if self.scene.lights.len() == 64 {
                break;
            }
            let local_headlight = Vec3::new(dx, 0.10, forward * 0.42);
            let world_position = vehicle_world.transform_point3(local_headlight);
            let index = self.scene.lights.len() as u32;
            let mut headlight = light(
                world_position.to_array(),
                [0.72, 0.86, 1.0],
                12.0,
                8.0,
                1,
                id,
            );
            headlight.direction = [0.0, 0.0, forward];
            headlight.cos_outer = 18.0_f32.to_radians().cos();
            headlight.cos_inner_bits = 12.0_f32.to_radians().cos().to_bits();
            self.scene.lights.push(headlight);
            indices.push(index);
        }
        indices
    }
}

#[derive(Clone, Copy)]
struct CatalogSign {
    region_id: u32,
    text: &'static str,
    kind: SignKind,
    color_linear: [f32; 3],
    intensity: f32,
    seed: u64,
}

// Frozen transcription of assets/signs/catalog.json (schema version 1), so scene
// generation does not require a runtime JSON dependency on native or wasm.
const SIGN_CATALOG: [CatalogSign; 24] = [
    CatalogSign {
        region_id: 0,
        text: "夜市",
        kind: SignKind::Tube,
        color_linear: [1.0, 0.025, 0.28],
        intensity: 12.0,
        seed: 101,
    },
    CatalogSign {
        region_id: 1,
        text: "ラーメン",
        kind: SignKind::Tube,
        color_linear: [1.0, 0.04, 0.36],
        intensity: 11.0,
        seed: 102,
    },
    CatalogSign {
        region_id: 2,
        text: "OPEN 24H",
        kind: SignKind::Tube,
        color_linear: [0.02, 0.65, 1.0],
        intensity: 9.0,
        seed: 103,
    },
    CatalogSign {
        region_id: 3,
        text: "深夜茶室",
        kind: SignKind::Tube,
        color_linear: [1.0, 0.025, 0.24],
        intensity: 10.5,
        seed: 104,
    },
    CatalogSign {
        region_id: 4,
        text: "NEON BAR",
        kind: SignKind::Tube,
        color_linear: [1.0, 0.035, 0.4],
        intensity: 10.0,
        seed: 105,
    },
    CatalogSign {
        region_id: 5,
        text: "すし",
        kind: SignKind::Tube,
        color_linear: [0.02, 0.8, 1.0],
        intensity: 9.5,
        seed: 106,
    },
    CatalogSign {
        region_id: 6,
        text: "電脳遊戯",
        kind: SignKind::Led,
        color_linear: [1.0, 0.025, 0.32],
        intensity: 5.5,
        seed: 201,
    },
    CatalogSign {
        region_id: 7,
        text: "機械修理",
        kind: SignKind::Led,
        color_linear: [0.025, 0.7, 1.0],
        intensity: 5.0,
        seed: 202,
    },
    CatalogSign {
        region_id: 8,
        text: "FOOD 24H",
        kind: SignKind::Led,
        color_linear: [1.0, 0.04, 0.22],
        intensity: 6.0,
        seed: 203,
    },
    CatalogSign {
        region_id: 9,
        text: "冷飲",
        kind: SignKind::Led,
        color_linear: [0.015, 0.8, 1.0],
        intensity: 4.5,
        seed: 204,
    },
    CatalogSign {
        region_id: 10,
        text: "HOTEL 08",
        kind: SignKind::Led,
        color_linear: [1.0, 0.03, 0.4],
        intensity: 5.0,
        seed: 205,
    },
    CatalogSign {
        region_id: 11,
        text: "音楽",
        kind: SignKind::Led,
        color_linear: [0.38, 0.025, 1.0],
        intensity: 5.5,
        seed: 206,
    },
    CatalogSign {
        region_id: 12,
        text: "未来通信",
        kind: SignKind::Hologram,
        color_linear: [0.02, 0.8, 1.0],
        intensity: 4.0,
        seed: 301,
    },
    CatalogSign {
        region_id: 13,
        text: "夢の都市",
        kind: SignKind::Hologram,
        color_linear: [1.0, 0.035, 0.5],
        intensity: 4.5,
        seed: 302,
    },
    CatalogSign {
        region_id: 14,
        text: "LINK 2077",
        kind: SignKind::Hologram,
        color_linear: [1.0, 0.025, 0.35],
        intensity: 4.0,
        seed: 303,
    },
    CatalogSign {
        region_id: 15,
        text: "空中配送",
        kind: SignKind::Hologram,
        color_linear: [0.03, 0.7, 1.0],
        intensity: 3.5,
        seed: 304,
    },
    CatalogSign {
        region_id: 16,
        text: "仮想空間",
        kind: SignKind::Hologram,
        color_linear: [0.42, 0.025, 1.0],
        intensity: 4.0,
        seed: 305,
    },
    CatalogSign {
        region_id: 17,
        text: "MEMORY",
        kind: SignKind::Hologram,
        color_linear: [1.0, 0.025, 0.45],
        intensity: 4.0,
        seed: 306,
    },
    CatalogSign {
        region_id: 18,
        text: "NO SIGNAL",
        kind: SignKind::FaultText,
        color_linear: [1.0, 0.025, 0.3],
        intensity: 7.0,
        seed: 401,
    },
    CatalogSign {
        region_id: 19,
        text: "電気街",
        kind: SignKind::FaultText,
        color_linear: [1.0, 0.03, 0.4],
        intensity: 6.5,
        seed: 402,
    },
    CatalogSign {
        region_id: 20,
        text: "ECHO 09",
        kind: SignKind::FaultText,
        color_linear: [0.03, 0.7, 1.0],
        intensity: 6.0,
        seed: 403,
    },
    CatalogSign {
        region_id: 21,
        text: "裏路地",
        kind: SignKind::FaultText,
        color_linear: [1.0, 0.025, 0.25],
        intensity: 6.0,
        seed: 404,
    },
    CatalogSign {
        region_id: 22,
        text: "ERROR 404",
        kind: SignKind::FaultText,
        color_linear: [0.4, 0.025, 1.0],
        intensity: 7.0,
        seed: 405,
    },
    CatalogSign {
        region_id: 23,
        text: "夜行列車",
        kind: SignKind::FaultText,
        color_linear: [1.0, 0.035, 0.35],
        intensity: 6.5,
        seed: 406,
    },
];

fn sign_catalog() -> &'static [CatalogSign] {
    &SIGN_CATALOG
}

fn facade_surface_x(config: &SceneConfig, side: f32) -> f32 {
    side * (config.street_width_m * 0.5 + 0.45)
}

fn street_facing_x(config: &SceneConfig, side: f32, clearance: f32) -> f32 {
    facade_surface_x(config, side) - side * clearance
}

fn building_anchor(config: &SceneConfig, side_index: u32, along_index: u32) -> (f32, f32) {
    let per = config.buildings_per_side.max(1);
    let side = if side_index == 0 { -1.0 } else { 1.0 };
    let z = -config.street_length_m * 0.5
        + (along_index as f32 + 0.5) * config.street_length_m / per as f32;
    (side, z)
}

fn safe_detail_z(
    config: &SceneConfig,
    scene: &SceneCpu,
    building: u32,
    center_z: f32,
    y: f32,
    height: f32,
    requested_z: f32,
    depth: f32,
    ordinal: u32,
) -> f32 {
    let side_index = building / config.buildings_per_side.max(1);
    let side = if side_index == 0 { -1.0 } else { 1.0 };
    let half_y = height * 0.5;
    let half_z = depth * 0.5;
    let margin = 0.18;
    let building_depth = config.street_length_m / config.buildings_per_side.max(1) as f32 * 0.94;
    let end = (building_depth * 0.5 - half_z - margin).max(0.0);
    let candidates = [
        requested_z.clamp(center_z - end, center_z + end),
        center_z + if ordinal % 2 == 0 { -end } else { end },
        center_z + if ordinal % 2 == 0 { end } else { -end },
    ];

    let nearest_sign = scene
        .signs
        .iter()
        .filter_map(|sign| {
            let instance = &scene.instances[sign.instance as usize];
            let sign_x = instance.world_from_local.w_axis.x;
            if (if sign_x < 0.0 { -1.0 } else { 1.0 }) != side {
                return None;
            }
            let a = instance.world_from_local.transform_point3(Vec3::ZERO);
            let b = instance
                .world_from_local
                .transform_point3(Vec3::new(0.0, 1.0, 1.0));
            let sign_center_z = (a.z + b.z) * 0.5;
            Some((
                (sign_center_z - center_z).abs(),
                a.z.min(b.z),
                a.z.max(b.z),
                a.y.min(b.y),
                a.y.max(b.y),
            ))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));

    let clear = |candidate: f32| {
        let z_min = candidate - half_z;
        let z_max = candidate + half_z;
        let y_min = y - half_y;
        let y_max = y + half_y;
        if let Some((_, sign_z_min, sign_z_max, sign_y_min, sign_y_max)) = nearest_sign {
            let sign_overlap = z_min < sign_z_max + margin
                && z_max > sign_z_min - margin
                && y_min < sign_y_max + margin
                && y_max > sign_y_min - margin;
            if sign_overlap {
                return false;
            }
        }
        // Reserve both glazing panels (centred at +/-2m, 1.6m wide x 2.8m tall).
        for window_z in [center_z - 2.0, center_z + 2.0] {
            if z_min < window_z + 0.8 + margin
                && z_max > window_z - 0.8 - margin
                && y_min < 3.4 + margin
                && y_max > 0.6 - margin
            {
                return false;
            }
        }
        true
    };
    candidates
        .into_iter()
        .find(|candidate| clear(*candidate))
        .unwrap_or(candidates[1])
}

fn materials() -> Vec<MaterialGpu> {
    let mut m = vec![material([0.24, 0.27, 0.29], 0.0, 0.72, 0.0, 0, 0); 19];
    for (index, color, roughness) in [
        (9, [0.20, 0.22, 0.24], 0.58),
        (10, [0.30, 0.31, 0.32], 0.48),
        (11, [0.08, 0.09, 0.10], 0.62),
        (12, [0.18, 0.20, 0.21], 0.52),
        (13, [0.12, 0.28, 0.30], 0.40),
        (14, [0.08, 0.09, 0.08], 0.72),
        (15, [0.20, 0.14, 0.12], 0.92),
        (16, [0.48, 0.28, 0.20], 0.88),
        (17, [0.16, 0.18, 0.19], 0.56),
        (18, [0.22, 0.23, 0.24], 0.50),
    ] {
        m[index] = material(color, 0.0, roughness, 0.0, 0, 0);
    }
    m[0] = material([0.26, 0.30, 0.35], 0.0, 0.72, 0.0, 0, 0);
    m[1] = material([0.035, 0.045, 0.055], 0.0, 0.08, 0.0, 1, 0);
    m[2] = material([0.07, 0.08, 0.09], 0.0, 0.32, 0.0, 2, 0);
    m[3] = material([0.12, 0.12, 0.11], 0.0, 0.78, 0.0, 3, 0);
    m[4] = material([0.12, 0.20, 0.24], 0.0, 0.08, 0.0, 4, 0);
    m[4].opacity = 0.22;
    m[4].ior = 1.45;
    for kind in 1..=4u32 {
        m[4 + kind as usize] = material([1.0; 3], 0.0, 0.28, 0.0, 5, kind);
        m[4 + kind as usize].bloom_class = kind;
        m[4 + kind as usize].atlas_region = 0;
    }
    for entry in sign_catalog() {
        let mut sign = m[4 + entry.kind as usize];
        sign.base_color = entry.color_linear;
        sign.emission = entry.color_linear.map(|c| c * entry.intensity);
        sign.atlas_region = entry.region_id;
        m.push(sign);
    }
    for (color, emission, roughness) in [
        ([0.045, 0.09, 0.15], [0.0; 3], 0.32),
        ([0.10, 0.22, 0.32], [0.04, 0.16, 0.28], 0.32),
        ([0.34, 0.20, 0.09], [0.45, 0.20, 0.06], 0.38),
        ([0.22, 0.29, 0.38], [0.0; 3], 0.58),
        ([0.48, 0.045, 0.23], [0.0; 3], 0.62),
        ([0.035, 0.36, 0.44], [0.0; 3], 0.62),
        ([0.57, 0.66, 0.72], [0.0; 3], 0.68),
        ([0.23, 0.65, 0.85], [0.7, 2.2, 3.2], 0.35),
    ] {
        let mut surface = material(color, 0.0, roughness, 0.0, 0, 0);
        surface.emission = emission;
        m.push(surface);
    }
    // Cars no longer inherit the brighter concrete facade material.
    for color in [[0.28, 0.035, 0.09], [0.025, 0.16, 0.21], [0.19, 0.12, 0.29]] {
        m.push(material(color, 0.25, 0.58, 0.0, 0, 0));
    }
    m
}
fn material(
    color: [f32; 3],
    metal: f32,
    rough: f32,
    emission: f32,
    kind: u32,
    bloom: u32,
) -> MaterialGpu {
    MaterialGpu {
        base_color: color,
        metallic: metal,
        emission: [emission; 3],
        roughness: rough,
        ior: 1.0,
        opacity: 1.0,
        normal_scale: 1.0,
        wetness: if kind == 1 { 1.0 } else { 0.0 },
        kind,
        atlas_region: 0,
        bloom_class: bloom,
        flags: 0,
    }
}
fn add_instance(
    s: &mut SceneCpu,
    mesh: u32,
    material: u32,
    p: Vec3,
    scale: Vec3,
    flags: u32,
) -> u32 {
    let id = s.instances.len() as u32;
    s.instances.push(InstanceDesc {
        id,
        mesh,
        material,
        world_from_local: Mat4::from_scale_rotation_translation(scale, glam::Quat::IDENTITY, p),
        flags,
    });
    id
}
fn light(p: [f32; 3], c: [f32; 3], range: f32, intensity: f32, kind: u32, id: u32) -> LightGpu {
    LightGpu {
        position: p,
        range,
        direction: [0.0, -1.0, 0.0],
        cos_outer: -1.0,
        color: c,
        intensity,
        kind,
        source_id: id,
        cos_inner_bits: 0,
        flags: 0,
    }
}
fn emitter(
    kind: EmitterKind,
    position: [f32; 3],
    direction: [f32; 3],
    rate: f32,
    seed: u64,
    id: u64,
) -> EmitterDesc {
    EmitterDesc {
        kind,
        position,
        direction,
        rate,
        seed: derive_seed(seed, id),
    }
}
fn derive_seed(seed: u64, id: u64) -> u64 {
    let mut x = seed ^ id.wrapping_mul(0x9e3779b97f4a7c15);
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
fn hash01(seed: u64, id: u64) -> f32 {
    (derive_seed(seed, id) >> 40) as f32 / (1u32 << 24) as f32
}
fn puddle_ellipse_mesh() -> MeshData {
    const SEGMENTS: u32 = 16;
    let mut mesh = MeshData::default();
    mesh.vertices
        .push(vertex([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.5, 0.5]));
    for i in 0..SEGMENTS {
        let angle = std::f32::consts::TAU * i as f32 / SEGMENTS as f32;
        let wobble = 0.88 + 0.12 * hash01(0x50dd1e, i as u64);
        let x = angle.cos() * 0.5 * wobble;
        let z = angle.sin() * 0.5 * (0.82 + 0.18 * wobble);
        mesh.vertices
            .push(vertex([x, 0.0, z], [0.0, 1.0, 0.0], [x + 0.5, z + 0.5]));
    }
    for i in 0..SEGMENTS {
        let a = i + 1;
        let b = (i + 1) % SEGMENTS + 1;
        mesh.indices.extend_from_slice(&[0, b, a]);
    }
    mesh
}

fn unit_box() -> MeshData {
    let faces = [
        (
            [0.0, 0.0, 1.0],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
        ),
        (
            [0.0, 0.0, -1.0],
            [0.5, -0.5, -0.5],
            [-0.5, -0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [0.5, 0.5, -0.5],
        ),
        (
            [1.0, 0.0, 0.0],
            [0.5, -0.5, 0.5],
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [0.5, 0.5, 0.5],
        ),
        (
            [-1.0, 0.0, 0.0],
            [-0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [-0.5, 0.5, -0.5],
        ),
        (
            [0.0, 1.0, 0.0],
            [-0.5, 0.5, 0.5],
            [0.5, 0.5, 0.5],
            [0.5, 0.5, -0.5],
            [-0.5, 0.5, -0.5],
        ),
        (
            [0.0, -1.0, 0.0],
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, -0.5, 0.5],
            [-0.5, -0.5, 0.5],
        ),
    ];
    let mut m = MeshData::default();
    for (normal, a, b, c, d) in faces {
        let base = m.vertices.len() as u32;
        for (p, uv) in [
            (a, [0.0, 1.0]),
            (b, [1.0, 1.0]),
            (c, [1.0, 0.0]),
            (d, [0.0, 0.0]),
        ] {
            m.vertices.push(vertex(p, normal, uv));
        }
        m.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    m
}
// Small hard-edge mesh authoring helpers. All parts are baked into one cached mesh
// per kind, so detail count and instance transforms/clearance remain unchanged.
fn mesh_face(mesh: &mut MeshData, points: [Vec3; 4]) {
    let normal = (points[1] - points[0])
        .cross(points[2] - points[0])
        .normalize();
    let base = mesh.vertices.len() as u32;
    for (p, uv) in points
        .into_iter()
        .zip([[0., 1.], [1., 1.], [1., 0.], [0., 0.]])
    {
        mesh.vertices
            .push(vertex(p.to_array(), normal.to_array(), uv));
    }
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn mesh_box(mesh: &mut MeshData, center: Vec3, scale: Vec3) {
    let part = unit_box();
    let base = mesh.vertices.len() as u32;
    for mut v in part.vertices {
        v.position = (Vec3::from_array(v.position) * scale + center).to_array();
        mesh.vertices.push(v);
    }
    mesh.indices
        .extend(part.indices.into_iter().map(|i| i + base));
}

// Octagonal capped tube between arbitrary endpoints, with flat radial normals.
fn mesh_tube(mesh: &mut MeshData, a: Vec3, b: Vec3, radius: f32) {
    let axis = (b - a).normalize();
    let u = axis
        .cross(if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::Z })
        .normalize();
    let v = axis.cross(u);
    for i in 0..8 {
        let radial = |j: u32| {
            let t = j as f32 * std::f32::consts::TAU / 8.0;
            radius * (u * t.cos() + v * t.sin())
        };
        let r = radial(i);
        let s = radial(i + 1);
        mesh_face(mesh, [a + r, a + s, b + s, b + r]);
        // Separate triangle vertices preserve hard cap normals, without degenerate quads.
        for (points, n) in [([a, a + s, a + r], -axis), ([b, b + r, b + s], axis)] {
            let base = mesh.vertices.len() as u32;
            for p in points {
                mesh.vertices
                    .push(vertex(p.to_array(), n.to_array(), [0.5, 0.5]));
            }
            mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
    }
}

fn normalize_mesh(mut mesh: MeshData) -> MeshData {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for v in &mesh.vertices {
        let p = Vec3::from_array(v.position);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let extent = hi - lo;
    for v in &mut mesh.vertices {
        v.position = ((Vec3::from_array(v.position) - lo) / extent - Vec3::splat(0.5)).to_array();
        v.normal = (Vec3::from_array(v.normal) * extent).normalize().to_array();
    }
    mesh
}

fn car_mesh() -> MeshData {
    let mut mesh = MeshData::default();
    // Low chassis/hood/trunk, inset cabin with sloped front AND rear windscreens.
    mesh_box(
        &mut mesh,
        Vec3::new(0., -0.17, 0.),
        Vec3::new(0.84, 0.30, 1.0),
    );
    let cabin = [
        Vec3::new(0., -0.02, -0.34),
        Vec3::new(0., -0.02, 0.34),
        Vec3::new(0., 0.38, 0.19),
        Vec3::new(0., 0.38, -0.19),
    ];
    let left = cabin.map(|p| p - Vec3::X * 0.34);
    let right = cabin.map(|p| p + Vec3::X * 0.34);
    mesh_face(&mut mesh, left);
    mesh_face(&mut mesh, [right[3], right[2], right[1], right[0]]);
    for i in 0..4 {
        let j = (i + 1) % 4;
        mesh_face(&mut mesh, [right[i], right[j], left[j], left[i]]);
    }
    mesh_box(
        &mut mesh,
        Vec3::new(0., 0.40, 0.),
        Vec3::new(0.72, 0.04, 0.42),
    );
    // Four exposed octagonal tires, axes transverse to the direction of travel.
    for x in [-1.0, 1.0] {
        for z in [-0.32, 0.32] {
            mesh_tube(
                &mut mesh,
                Vec3::new(x * 0.35, -0.32, z),
                Vec3::new(x * 0.50, -0.32, z),
                0.16,
            );
        }
    }
    normalize_mesh(mesh)
}

fn drone_mesh() -> MeshData {
    let mut mesh = MeshData::default();
    mesh_box(&mut mesh, Vec3::ZERO, Vec3::new(0.32, 0.18, 0.42));
    mesh_box(
        &mut mesh,
        Vec3::new(0., -0.15, 0.08),
        Vec3::new(0.16, 0.12, 0.18),
    );
    for x in [-1.0, 1.0] {
        for z in [-1.0, 1.0] {
            let hub = Vec3::new(x * 0.33, 0.07, z * 0.33);
            mesh_tube(&mut mesh, Vec3::new(x * 0.10, 0., z * 0.12), hub, 0.035);
            mesh_tube(&mut mesh, hub - Vec3::Y * 0.03, hub + Vec3::Y * 0.06, 0.045);
            // Open rotor guard rings, not a solid square/disc. No extra animation system.
            for i in 0..12 {
                let radial = |j: u32, r: f32| {
                    let a = j as f32 * std::f32::consts::TAU / 12.;
                    hub + Vec3::new(a.cos() * r, 0.06, a.sin() * r)
                };
                let a = radial(i, 0.15);
                let b = radial(i + 1, 0.15);
                let c = radial(i + 1, 0.17);
                let d = radial(i, 0.17);
                mesh_face(&mut mesh, [a, b, c, d]);
                mesh_face(
                    &mut mesh,
                    [
                        d - Vec3::Y * 0.025,
                        c - Vec3::Y * 0.025,
                        b - Vec3::Y * 0.025,
                        a - Vec3::Y * 0.025,
                    ],
                );
                mesh_face(&mut mesh, [d, c, c - Vec3::Y * 0.025, d - Vec3::Y * 0.025]);
                mesh_face(&mut mesh, [b, a, a - Vec3::Y * 0.025, b - Vec3::Y * 0.025]);
            }
            mesh_box(
                &mut mesh,
                hub + Vec3::Y * 0.045,
                Vec3::new(0.27, 0.015, 0.035),
            );
        }
    }
    normalize_mesh(mesh)
}

fn detail_mesh(category: u32) -> MeshData {
    let mut mesh = MeshData::default();
    // Both X faces receive relief: shared geometry works on either street facade.
    match category {
        0 => {
            // HVAC cabinet and separated grille slats in front of a recessed face.
            mesh_box(&mut mesh, Vec3::ZERO, Vec3::new(0.72, 1., 1.));
            for x in [-0.44, 0.44] {
                for i in 0..7 {
                    mesh_box(
                        &mut mesh,
                        Vec3::new(x, -0.39 + i as f32 * 0.13, 0.),
                        Vec3::new(0.12, 0.045, 0.86),
                    );
                }
            }
        }
        1 | 9 => {
            // Octagonal drain/conduit with proud collar joints.
            mesh_tube(
                &mut mesh,
                Vec3::new(0., -0.5, 0.),
                Vec3::new(0., 0.5, 0.),
                0.30,
            );
            for y in [-0.40, 0.0, 0.40] {
                mesh_tube(
                    &mut mesh,
                    Vec3::new(0., y - 0.035, 0.),
                    Vec3::new(0., y + 0.035, 0.),
                    0.5,
                );
            }
        }
        2 => {
            // Sagging cable made of contiguous octagonal curved segments.
            let point = |i: u32| {
                let z = i as f32 / 12. - 0.5;
                Vec3::new(
                    0.05 * (z * std::f32::consts::TAU).sin(),
                    1.5 * z * z - 0.25,
                    z,
                )
            };
            for i in 0..12 {
                mesh_tube(&mut mesh, point(i), point(i + 1), 0.035);
            }
        }
        3 => {
            // Open multi-blade louver in a four-sided frame.
            for y in [-0.46, 0.46] {
                mesh_box(&mut mesh, Vec3::new(0., y, 0.), Vec3::new(1., 0.08, 1.));
            }
            for z in [-0.46, 0.46] {
                mesh_box(&mut mesh, Vec3::new(0., 0., z), Vec3::new(1., 0.84, 0.08));
            }
            for i in 0..6 {
                let y = -0.36 + i as f32 * 0.144;
                // Slope the blades in depth, leaving actual gaps between them.
                mesh_tube(
                    &mut mesh,
                    Vec3::new(-0.30, y - 0.035, -0.42),
                    Vec3::new(-0.30, y - 0.035, 0.42),
                    0.045,
                );
                mesh_box(&mut mesh, Vec3::new(0., y, 0.), Vec3::new(0.8, 0.04, 0.84));
            }
        }
        4 => {
            // Vending: recessed display, thick border and lower delivery slot.
            mesh_box(&mut mesh, Vec3::ZERO, Vec3::new(0.60, 1., 1.));
            for x in [-0.43, 0.43] {
                for z in [-0.45, 0.45] {
                    mesh_box(&mut mesh, Vec3::new(x, 0., z), Vec3::new(0.14, 1., 0.10));
                }
                for y in [-0.45, 0.45, -0.18] {
                    mesh_box(&mut mesh, Vec3::new(x, y, 0.), Vec3::new(0.14, 0.10, 0.8));
                }
                for z in [-0.22, 0., 0.22] {
                    mesh_box(
                        &mut mesh,
                        Vec3::new(x * 0.77, 0.12, z),
                        Vec3::new(0.04, 0.40, 0.12),
                    );
                }
            }
        }
        5 => {
            // Refuse bin with overhanging lid and raised handle.
            mesh_box(
                &mut mesh,
                Vec3::new(0., -0.06, 0.),
                Vec3::new(0.82, 0.84, 0.82),
            );
            mesh_box(&mut mesh, Vec3::new(0., 0.40, 0.), Vec3::new(1., 0.12, 1.));
            mesh_box(
                &mut mesh,
                Vec3::new(0., 0.49, 0.),
                Vec3::new(0.30, 0.06, 0.14),
            );
        }
        6 => {
            // Unequal runoff streaks instead of a solid rectangular stain.
            for (z, y, height) in [
                (-0.38, 0.05, 0.9),
                (-0.12, -0.04, 0.72),
                (0.16, 0., 1.),
                (0.40, 0.14, 0.58),
            ] {
                mesh_box(&mut mesh, Vec3::new(0., y, z), Vec3::new(1., height, 0.12));
            }
        }
        7 => {
            // Three layered paper/sticker rectangles, broken outer contour.
            for (y, z) in [(-0.25, -0.22), (0.20, 0.18), (-0.12, 0.27)] {
                mesh_box(&mut mesh, Vec3::new(0., y, z), Vec3::new(1., 0.43, 0.38));
            }
        }
        _ => {
            // Junction enclosure with proud seam and four bolt heads.
            mesh_box(&mut mesh, Vec3::ZERO, Vec3::new(0.8, 1., 1.));
            for x in [-0.46, 0.46] {
                mesh_box(&mut mesh, Vec3::new(x, 0., 0.), Vec3::new(0.08, 0.88, 0.88));
                for y in [-0.35, 0.35] {
                    for z in [-0.35, 0.35] {
                        mesh_tube(
                            &mut mesh,
                            Vec3::new(x, y, z),
                            Vec3::new(x.signum() * 0.53, y, z),
                            0.055,
                        );
                    }
                }
            }
        }
    }
    normalize_mesh(mesh)
}

fn quad_mesh(side: f32) -> MeshData {
    let n = [-side, 0.0, 0.0];
    let mut v = if side < 0.0 {
        vec![
            vertex([0.0, 0.0, 0.0], n, [0.0, 1.0]),
            vertex([0.0, 0.0, 1.0], n, [1.0, 1.0]),
            vertex([0.0, 1.0, 1.0], n, [1.0, 0.0]),
            vertex([0.0, 1.0, 0.0], n, [0.0, 0.0]),
        ]
    } else {
        vec![
            vertex([0.0, 0.0, 1.0], n, [0.0, 1.0]),
            vertex([0.0, 0.0, 0.0], n, [1.0, 1.0]),
            vertex([0.0, 1.0, 0.0], n, [1.0, 0.0]),
            vertex([0.0, 1.0, 1.0], n, [0.0, 0.0]),
        ]
    };
    // Atlas U increases toward camera-right on both facades: left wall camera-right is -Z,
    // right wall camera-right is +Z. Flip U only; retain vertex order and face winding.
    for vertex in &mut v {
        vertex.uv[0] = 1.0 - vertex.uv[0];
    }
    MeshData {
        vertices: v,
        // The authored vertex sequences are clockwise from both declared street-facing normals.
        indices: vec![0, 2, 1, 0, 3, 2],
    }
}
fn vertex(p: [f32; 3], n: [f32; 3], uv: [f32; 2]) -> VertexGpu {
    VertexGpu {
        position: p,
        normal: n,
        uv,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builds_incrementally_and_meets_scene_contract() {
        let mut b = SceneBuilder::new(SceneConfig::default());
        let scene = loop {
            match b.step(3).unwrap() {
                BuildPoll::Ready(s) => break s,
                BuildPoll::Pending { .. } => {}
            }
        };
        assert_eq!(
            scene.meshes[0].vertices.iter().map(|v| v.position).fold(
                [f32::INFINITY; 3],
                |mut a, p| {
                    for i in 0..3 {
                        a[i] = a[i].min(p[i]);
                    }
                    a
                }
            ),
            [-0.5; 3]
        );
        assert_eq!(scene.signs.len(), 24);
        let puddles: Vec<_> = scene.instances.iter().filter(|i| i.material == 1).collect();
        assert!((32..=40).contains(&puddles.len()));
        assert!(puddles.iter().all(|i| i.mesh == 1));
        assert_eq!(
            scene
                .instances
                .iter()
                .filter(|i| (4..14).contains(&i.mesh) && (9..=18).contains(&i.material))
                .count(),
            480
        );
        let wall_x = SceneConfig::default().street_width_m * 0.5 + 0.45;
        for sign in &scene.signs {
            let instance = &scene.instances[sign.instance as usize];
            let x = instance.world_from_local.w_axis.x;
            assert!(
                wall_x - x.abs() >= 0.02,
                "sign must be at least 2cm street-side of facade"
            );
        }
        assert_eq!(
            scene
                .instances
                .iter()
                .filter(|i| i.flags & DYNAMIC != 0)
                .count(),
            12
        );
        assert!(scene.lights.len() <= 64);
        assert_eq!(
            scene
                .animation_bindings
                .iter()
                .filter(|b| b.kind == AnimationKind::Vehicle || b.kind == AnimationKind::Drone)
                .count(),
            12
        );
        assert_eq!(
            scene
                .emitters
                .iter()
                .filter(|e| e.kind == EmitterKind::Steam)
                .count(),
            3
        );
        assert_eq!(
            scene
                .signs
                .iter()
                .map(|s| s.kind as u32)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            4
        );
        for (region, sign) in scene.signs.iter().enumerate() {
            let instance = &scene.instances[sign.instance as usize];
            let mesh = &scene.meshes[instance.mesh as usize];
            assert_eq!(mesh.indices.len(), 6);
            assert_eq!(
                scene.materials[instance.material as usize].atlas_region,
                region as u32
            );
            assert_eq!(
                scene.materials[instance.material as usize].bloom_class,
                sign.kind as u32
            );
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.uv[0] >= 0. && v.uv[0] <= 1. && v.uv[1] >= 0. && v.uv[1] <= 1.)
            );
            let tri = |ids: &[u32]| {
                let a = Vec3::from_array(mesh.vertices[ids[0] as usize].position);
                let b = Vec3::from_array(mesh.vertices[ids[1] as usize].position);
                let c = Vec3::from_array(mesh.vertices[ids[2] as usize].position);
                let declared = Vec3::from_array(mesh.vertices[ids[0] as usize].normal);
                assert!((b - a).cross(c - a).dot(declared) > 0.0);
            };
            tri(&mesh.indices[0..3]);
            tri(&mesh.indices[3..6]);
            let scale = instance
                .world_from_local
                .transform_point3(Vec3::new(0.0, 0.0, 1.0))
                - instance.world_from_local.transform_point3(Vec3::ZERO);
            assert!((scale.length() - 3.8).abs() < 1e-5);
        }
        let car = scene
            .animation_bindings
            .iter()
            .find(|b| b.kind == AnimationKind::Vehicle)
            .unwrap();
        // Raster cars are deliberately no longer mesh zero. Their normalized geometry
        // remains enclosed by the existing world-transform analytic box proxy.
        for binding in scene
            .animation_bindings
            .iter()
            .filter(|b| matches!(b.kind, AnimationKind::Vehicle | AnimationKind::Drone))
        {
            let instance = &scene.instances[binding.instance as usize];
            assert_eq!(
                instance.mesh,
                if binding.kind == AnimationKind::Vehicle {
                    CAR_MESH
                } else {
                    DRONE_MESH
                }
            );
            let mesh = &scene.meshes[instance.mesh as usize];
            assert!(mesh.indices.len() > scene.meshes[0].indices.len());
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.position.iter().all(|p| (-0.5..=0.5).contains(p)))
            );
        }
        let car_pos = scene.instances[car.instance as usize]
            .world_from_local
            .w_axis
            .truncate();
        for &light_index in &car.light_indices {
            let headlight = &scene.lights[light_index as usize];
            assert!(Vec3::from_array(headlight.position).distance(car_pos) < 3.0);
            assert_eq!(headlight.kind, 1);
            let forward = if car_pos.x < 0.0 { 1.0 } else { -1.0 };
            assert!(headlight.direction[2] * forward > 0.99);
        }
        let detail_counts = scene
            .instances
            .iter()
            .filter(|i| (4..14).contains(&i.mesh) && (9..=18).contains(&i.material))
            .fold([0usize; 10], |mut counts, i| {
                counts[i.material as usize - 9] += 1;
                counts
            });
        assert_eq!(detail_counts, [48; 10]);
        for instance in scene.instances.iter().filter(|i| {
            (4..14).contains(&i.mesh) && (9..=18).contains(&i.material) && i.material != 14
        }) {
            let x = instance.world_from_local.w_axis.x;
            let half_width = instance.world_from_local.x_axis.truncate().length() * 0.5;
            assert!(
                wall_x - (x.abs() + half_width) >= 0.02,
                "facade detail's street-facing extent clears wall"
            );
        }
        assert!(scene.instances.iter().filter(|i| i.material == 4).count() >= 24);
    }
    #[test]
    fn configurable_building_count_gets_twenty_details_per_building() {
        let mut builder = SceneBuilder::new(SceneConfig {
            buildings_per_side: 2,
            ..Default::default()
        });
        let scene = loop {
            if let BuildPoll::Ready(scene) = builder.step(1000).unwrap() {
                break scene;
            }
        };
        let details: Vec<_> = scene
            .instances
            .iter()
            .filter(|i| (4..14).contains(&i.mesh) && (9..=18).contains(&i.material))
            .collect();
        assert_eq!(details.len(), 80);
        for side in [-1.0, 1.0] {
            for building in 0..2 {
                let count = details
                    .iter()
                    .filter(|instance| {
                        let p = instance.world_from_local.w_axis;
                        if (if p.x < 0.0 { -1.0 } else { 1.0 }) != side {
                            return false;
                        }
                        let nearest = (0..2)
                            .min_by(|a, b| {
                                let za = building_anchor(
                                    &SceneConfig {
                                        buildings_per_side: 2,
                                        ..Default::default()
                                    },
                                    if side < 0.0 { 0 } else { 1 },
                                    *a,
                                )
                                .1;
                                let zb = building_anchor(
                                    &SceneConfig {
                                        buildings_per_side: 2,
                                        ..Default::default()
                                    },
                                    if side < 0.0 { 0 } else { 1 },
                                    *b,
                                )
                                .1;
                                (p.z - za).abs().total_cmp(&(p.z - zb).abs())
                            })
                            .unwrap();
                        nearest == building
                    })
                    .count();
                assert_eq!(count, 20);
            }
        }
    }

    #[test]
    fn same_seed_produces_same_scene_data() {
        let build = |seed| {
            let mut b = SceneBuilder::new(SceneConfig {
                seed,
                ..Default::default()
            });
            loop {
                if let BuildPoll::Ready(s) = b.step(1000).unwrap() {
                    break s;
                }
            }
        };
        let a = build(77);
        let b = build(77);
        assert_eq!(a.instances.len(), b.instances.len());
        for (x, y) in a.instances.iter().zip(&b.instances) {
            assert_eq!(x.id, y.id);
            assert_eq!(x.world_from_local, y.world_from_local);
            assert_eq!(x.flags, y.flags);
        }
        assert_eq!(
            a.signs
                .iter()
                .map(|s| (&s.text, s.seed))
                .collect::<Vec<_>>(),
            b.signs
                .iter()
                .map(|s| (&s.text, s.seed))
                .collect::<Vec<_>>()
        );
    }
}
