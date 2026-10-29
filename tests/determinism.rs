use glam::Vec3;
use neon_city::contract::{AnimationKind, BuildPoll, SceneConfig};
use neon_city::scene::SceneBuilder;

fn build(seed: u64) -> neon_city::contract::SceneCpu {
    let mut builder = SceneBuilder::new(SceneConfig {
        seed,
        ..SceneConfig::default()
    });
    loop {
        match builder.step(7).expect("scene generation") {
            BuildPoll::Pending { progress } => assert!((0.0..1.0).contains(&progress)),
            BuildPoll::Ready(scene) => return scene,
        }
    }
}

#[test]
fn generation_is_repeatable_for_a_fixed_seed() {
    let a = build(0x1234);
    let b = build(0x1234);
    assert_eq!(a.instances.len(), b.instances.len());
    for (a, b) in a.instances.iter().zip(&b.instances) {
        assert_eq!(
            (a.id, a.mesh, a.material, a.flags),
            (b.id, b.mesh, b.material, b.flags)
        );
        assert_eq!(a.world_from_local, b.world_from_local);
    }
    assert_eq!(
        a.signs
            .iter()
            .map(|s| (&s.text, s.kind, s.seed))
            .collect::<Vec<_>>(),
        b.signs
            .iter()
            .map(|s| (&s.text, s.kind, s.seed))
            .collect::<Vec<_>>()
    );
}

#[test]
fn cached_composite_geometry_is_bounded_and_repeatable() {
    let scene = build(0x1234);
    let again = build(0x1234);
    for (a, b) in scene.meshes.iter().zip(&again.meshes) {
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.vertices.len(), b.vertices.len());
        for (a, b) in a.vertices.iter().zip(&b.vertices) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.normal, b.normal);
            assert_eq!(a.uv, b.uv);
        }
    }
    assert_eq!(
        scene.meshes[0].indices.len(),
        36,
        "canonical building box survives"
    );
    for mesh in &scene.meshes[2..14] {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for v in &mesh.vertices {
            let p = Vec3::from_array(v.position);
            assert!(p.is_finite());
            assert!((Vec3::from_array(v.normal).length() - 1.0).abs() < 1e-5);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        assert_eq!(lo, Vec3::splat(-0.5));
        assert_eq!(hi, Vec3::splat(0.5));
        for tri in mesh.indices.chunks_exact(3) {
            let a = &mesh.vertices[tri[0] as usize];
            let b = &mesh.vertices[tri[1] as usize];
            let c = &mesh.vertices[tri[2] as usize];
            let cross = (Vec3::from_array(b.position) - Vec3::from_array(a.position))
                .cross(Vec3::from_array(c.position) - Vec3::from_array(a.position));
            assert!(
                cross.dot(Vec3::from_array(a.normal)) > 0.,
                "nondegenerate outward-wound face"
            );
        }
    }
    for binding in &scene.animation_bindings {
        let expected = match binding.kind {
            AnimationKind::Vehicle => 2,
            AnimationKind::Drone => 3,
            _ => continue,
        };
        let instance = &scene.instances[binding.instance as usize];
        assert_eq!(
            instance.mesh, expected,
            "approved raster silhouette; rays retain transform-box proxy"
        );
        assert!(scene.meshes[expected as usize].indices.len() > 36);
    }
    for category in 0..10 {
        let instances: Vec<_> = scene
            .instances
            .iter()
            .filter(|i| (4..14).contains(&i.mesh) && i.material == 9 + category)
            .collect();
        assert_eq!(instances.len(), 48);
        assert!(
            instances.iter().all(|i| i.mesh == 4 + category),
            "one shared mesh per dressing kind"
        );
    }
    let static_triangles: usize = scene
        .instances
        .iter()
        .filter(|i| i.flags & 1 == 0)
        .map(|i| scene.meshes[i.mesh as usize].indices.len() / 3)
        .sum();
    assert!(
        static_triangles < 100_000,
        "static triangles: {static_triangles}"
    );
}

#[test]
fn facade_windows_canopies_and_road_paint_are_real_static_geometry() {
    let scene = build(SceneConfig::default().seed);
    let bounds = |i: &neon_city::contract::InstanceDesc| {
        scene.meshes[i.mesh as usize].vertices.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(lo, hi), v| {
                let p = i
                    .world_from_local
                    .transform_point3(Vec3::from_array(v.position));
                (lo.min(p), hi.max(p))
            },
        )
    };
    let mut panes = [0; 3];
    for i in scene
        .instances
        .iter()
        .filter(|i| (43..=48).contains(&i.material) && !(i.material == 46 && i.mesh == 0))
    {
        assert_eq!(i.flags, 0, "new architecture participates in static BVH");
        let (lo, hi) = bounds(i);
        assert!(lo.x.abs().max(hi.x.abs()) < 12.45);
        assert!(lo.x.abs().min(hi.x.abs()) > 10.7);
        if i.material <= 45 {
            panes[i.material as usize - 43] += scene.meshes[i.mesh as usize].indices.len() / 36;
            assert!(lo.y >= 7.3 && hi.y < 19.0);
            let building = scene
                .instances
                .iter()
                .find(|b| {
                    b.mesh == 0
                        && b.material == 0
                        && b.flags == 0
                        && b.world_from_local.w_axis.x.signum()
                            == i.world_from_local.w_axis.x.signum()
                        && (b.world_from_local.w_axis.z - i.world_from_local.w_axis.z).abs() < 0.001
                })
                .unwrap();
            assert!(hi.y < bounds(building).1.y, "windows stay below the roof");
        }
        if (47..=48).contains(&i.material) {
            assert!(lo.y > 3.4 && hi.y < 4.3);
        }
        // Test actual mesh bounds, not unit-box assumptions on combined batches.
        for sign in &scene.signs {
            let si = &scene.instances[sign.instance as usize];
            if si.world_from_local.w_axis.x.signum() != i.world_from_local.w_axis.x.signum() {
                continue;
            }
            let (sl, sh) = bounds(si);
            assert!(
                !(lo.y < sh.y + 0.18
                    && hi.y > sl.y - 0.18
                    && lo.z < sh.z + 0.18
                    && hi.z > sl.z - 0.18)
            );
        }
    }
    assert!(panes[0] > 0 && panes[2] > 0 && panes[1] > panes[2]);
    assert_eq!(
        scene.instances.iter().filter(|i| i.material == 47).count(),
        24
    );
    assert_eq!(
        scene.instances.iter().filter(|i| i.material == 48).count(),
        24
    );
    assert_eq!(scene.materials[43].emission, [0.0; 3]);
    assert!(scene.materials[44].emission[2] > scene.materials[44].emission[0]);
    let paint = scene.instances.iter().find(|i| i.material == 49).unwrap();
    assert_eq!(paint.flags, 0);
    let mesh = &scene.meshes[paint.mesh as usize];
    assert!(mesh.indices.len() / 36 > 30, "dashes and two crossings");
    for part in mesh.vertices.chunks_exact(24) {
        let lo = part.iter().fold(Vec3::splat(f32::INFINITY), |a, v| {
            a.min(Vec3::from_array(v.position))
        });
        let hi = part.iter().fold(Vec3::splat(f32::NEG_INFINITY), |a, v| {
            a.max(Vec3::from_array(v.position))
        });
        let cx = (lo.x + hi.x) * 0.5;
        let surface = if (1.0..3.2).contains(&cx.abs()) {
            0.03
        } else {
            -0.005
        };
        assert!((lo.y - surface).abs() < 1e-6 && hi.y > surface);
        assert!(lo.x > -6.8 && hi.x < 6.8);
    }
    for light in &scene.lights[..4] {
        let source = &scene.instances[light.source_id as usize];
        assert_eq!(source.material, 50);
        assert_eq!(source.flags, 0);
        assert!(
            source
                .world_from_local
                .w_axis
                .truncate()
                .distance(Vec3::from_array(light.position))
                < 0.2
        );
        assert_eq!(light.intensity, 90.0);
        assert_eq!(light.range, 22.0);
    }
    assert!(scene.lights.len() <= 64);
    let mut bvh = neon_city::accel::BvhBuild::new(&scene).unwrap();
    let packed = loop {
        if let BuildPoll::Ready(packed) = bvh.step(100_000).unwrap() {
            break packed;
        }
    };
    let mut material_counts = vec![0usize; scene.materials.len()];
    let start = packed.words[6] as usize;
    let count = packed.words[7] as usize;
    assert!(count < 100_000);
    for triangle in packed.words[start..start + count * 32].chunks_exact(32) {
        material_counts[triangle[30] as usize] += 1;
    }
    for material in 43..=50 {
        let expected: usize = scene
            .instances
            .iter()
            .filter(|i| i.material == material as u32)
            .map(|i| scene.meshes[i.mesh as usize].indices.len() / 3)
            .sum();
        assert!(expected > 0);
        assert_eq!(
            material_counts[material], expected,
            "new material {material} reaches real BVH triangles"
        );
    }
}

#[test]
fn step_budget_does_not_change_the_result() {
    let mut builder = SceneBuilder::new(SceneConfig::default());
    let scene = loop {
        match builder.step(1).expect("scene generation") {
            BuildPoll::Pending { .. } => {}
            BuildPoll::Ready(scene) => break scene,
        }
    };
    let batched = build(SceneConfig::default().seed);
    assert_eq!(scene.instances.len(), batched.instances.len());
    assert_eq!(scene.signs.len(), batched.signs.len());
    assert_eq!(scene.lights.len(), batched.lights.len());
}

#[test]
fn scene_regressions_and_catalog_mapping() {
    let scene = build(SceneConfig::default().seed);
    assert_eq!(scene.instances.len(), 798);
    assert_eq!(
        scene.meshes.len(),
        135,
        "original 38 meshes plus four facade batches per building and road paint"
    );
    assert_eq!(scene.materials.len(), 54);
    assert_eq!(scene.lights.len(), 44);
    assert_eq!(scene.signs.len(), 24);
    let puddles: Vec<_> = scene.instances.iter().filter(|i| i.material == 1).collect();
    assert!((32..=40).contains(&puddles.len()));
    assert!(puddles.iter().all(|i| i.mesh == 1));
    for puddle in &puddles {
        let center = puddle.world_from_local.w_axis;
        assert!((3.0..=5.1).contains(&center.x.abs()));
        assert!((center.y - 0.002).abs() < 1e-6, "water stays on asphalt");
        for v in &scene.meshes[puddle.mesh as usize].vertices {
            let world = puddle
                .world_from_local
                .transform_point3(Vec3::from_array(v.position));
            assert!(
                world.x.abs() <= 6.75,
                "puddle AABB must not hide under sidewalk"
            );
        }
        assert!(center.x.abs() + puddle.world_from_local.x_axis.x.abs() * 0.5 <= 6.75);
    }
    assert_eq!(
        scene
            .signs
            .iter()
            .map(|s| s.kind)
            .filter(|k| *k == neon_city::contract::SignKind::Tube)
            .count(),
        6
    );
    assert_eq!(
        scene
            .signs
            .iter()
            .map(|s| s.kind)
            .filter(|k| *k == neon_city::contract::SignKind::Led)
            .count(),
        6
    );
    assert_eq!(
        scene
            .signs
            .iter()
            .map(|s| s.kind)
            .filter(|k| *k == neon_city::contract::SignKind::Hologram)
            .count(),
        6
    );
    assert_eq!(
        scene
            .signs
            .iter()
            .map(|s| s.kind)
            .filter(|k| *k == neon_city::contract::SignKind::FaultText)
            .count(),
        6
    );

    for (region, sign) in scene.signs.iter().enumerate() {
        let instance = &scene.instances[sign.instance as usize];
        let material = &scene.materials[instance.material as usize];
        assert_eq!(material.atlas_region, region as u32);
        assert_eq!(material.kind, 5);
        assert_eq!(material.bloom_class, sign.kind as u32);
        assert_eq!(material.base_color, sign.color_linear);
        for channel in 0..3 {
            assert!(
                (material.emission[channel] - sign.color_linear[channel] * sign.intensity).abs()
                    < 1e-5
            );
        }
        let mesh = &scene.meshes[instance.mesh as usize];
        assert_ne!(instance.mesh, 0, "each sign has its own rectangular mesh");
        let a = &mesh.vertices[0];
        let b = &mesh.vertices[1];
        let world_a = instance
            .world_from_local
            .transform_point3(Vec3::from_array(a.position));
        let world_b = instance
            .world_from_local
            .transform_point3(Vec3::from_array(b.position));
        let uv_delta = b.uv[0] - a.uv[0];
        let uv_u_tangent = (world_b - world_a) / uv_delta;
        let side = if instance.world_from_local.w_axis.x < 0.0 {
            -1.0
        } else {
            1.0
        };
        let view_direction = Vec3::new(side, 0.0, 0.0);
        assert!(
            uv_u_tangent.dot(view_direction.cross(Vec3::Y)) > 0.0,
            "atlas U runs camera-left to camera-right"
        );
        assert!(
            mesh.vertices
                .iter()
                .all(|v| (0.0..=1.0).contains(&v.uv[0]) && (0.0..=1.0).contains(&v.uv[1]))
        );
        for tri in mesh.indices.chunks_exact(3) {
            let a = Vec3::from_array(mesh.vertices[tri[0] as usize].position);
            let b = Vec3::from_array(mesh.vertices[tri[1] as usize].position);
            let c = Vec3::from_array(mesh.vertices[tri[2] as usize].position);
            let normal = Vec3::from_array(mesh.vertices[tri[0] as usize].normal);
            assert!(
                (b - a).cross(c - a).dot(normal) > 0.0,
                "winding agrees with declared normal"
            );
        }
        let width = instance
            .world_from_local
            .transform_point3(Vec3::new(0.0, 0.0, 1.0))
            .distance(instance.world_from_local.transform_point3(Vec3::ZERO));
        assert!((width - 3.8).abs() < 1e-5);
    }

    let detail_counts = scene
        .instances
        .iter()
        .filter(|i| (4..14).contains(&i.mesh) && (9..=18).contains(&i.material))
        .fold([0usize; 10], |mut counts, i| {
            counts[i.material as usize - 9] += 1;
            counts
        });
    assert_eq!(
        detail_counts, [48; 10],
        "10 detail instances per category per building across 24 buildings"
    );
    assert!(
        scene
            .instances
            .iter()
            .filter(|i| (4..14).contains(&i.mesh) && (9..=18).contains(&i.material))
            .all(|i| {
                let mat = &scene.materials[i.material as usize];
                mat.kind == 0 && mat.emission == [0.0; 3]
            })
    );
    assert_eq!(
        scene.instances.iter().filter(|i| i.material == 4).count(),
        48,
        "two local glass panels per building"
    );
    let facade_x = SceneConfig::default().street_width_m * 0.5 + 0.45;
    for instance in scene.instances.iter().filter(|i| i.material == 4) {
        let x = instance.world_from_local.w_axis.x;
        let half_width = instance.world_from_local.x_axis.truncate().length() * 0.5;
        assert!(
            facade_x - (x.abs() + half_width) >= 0.02,
            "glass street-facing surface clears wall"
        );
    }
    for instance in scene
        .instances
        .iter()
        .filter(|i| (4..14).contains(&i.mesh) && (9..=18).contains(&i.material) && i.material != 14)
    {
        let x = instance.world_from_local.w_axis.x;
        let half_width = instance.world_from_local.x_axis.truncate().length() * 0.5;
        assert!(
            facade_x - (x.abs() + half_width) >= 0.02,
            "detail street-facing extent clears wall"
        );
        let y = instance.world_from_local.w_axis.y;
        let z = instance.world_from_local.w_axis.z;
        let half_y = instance.world_from_local.y_axis.truncate().length() * 0.5;
        let half_z = instance.world_from_local.z_axis.truncate().length() * 0.5;
        let side = if x < 0.0 { -1.0 } else { 1.0 };
        for sign in &scene.signs {
            let si = &scene.instances[sign.instance as usize];
            if (if si.world_from_local.w_axis.x < 0.0 {
                -1.0
            } else {
                1.0
            }) != side
            {
                continue;
            }
            let a = si.world_from_local.transform_point3(Vec3::ZERO);
            let b = si
                .world_from_local
                .transform_point3(Vec3::new(0.0, 1.0, 1.0));
            let overlaps = z - half_z < b.z + 0.18
                && z + half_z > a.z - 0.18
                && y - half_y < b.y + 0.18
                && y + half_y > a.y - 0.18;
            assert!(!overlaps, "facade detail must not obscure sign text");
        }
        for glass in scene.instances.iter().filter(|i| i.material == 4) {
            if (if glass.world_from_local.w_axis.x < 0.0 {
                -1.0
            } else {
                1.0
            }) != side
            {
                continue;
            }
            let gy = glass.world_from_local.w_axis.y;
            let gz = glass.world_from_local.w_axis.z;
            let ghy = glass.world_from_local.y_axis.truncate().length() * 0.5;
            let ghz = glass.world_from_local.z_axis.truncate().length() * 0.5;
            let overlaps = z - half_z < gz + ghz + 0.18
                && z + half_z > gz - ghz - 0.18
                && y - half_y < gy + ghy + 0.18
                && y + half_y > gy - ghy - 0.18;
            assert!(!overlaps, "facade detail must not cover shopfront glazing");
        }
    }
    for sign in &scene.signs {
        let x = scene.instances[sign.instance as usize]
            .world_from_local
            .w_axis
            .x;
        assert!(
            facade_x - x.abs() >= 0.02,
            "sign must be at least 2cm street-side of wall"
        );
    }
    assert_eq!(
        scene.instances.iter().filter(|i| i.flags & 1 != 0).count(),
        12
    );

    let car = scene
        .animation_bindings
        .iter()
        .find(|b| b.kind == AnimationKind::Vehicle && !b.light_indices.is_empty())
        .unwrap();
    let center = scene.instances[car.instance as usize]
        .world_from_local
        .w_axis
        .truncate();
    let forward = if center.x < 0.0 { 1.0 } else { -1.0 };
    assert_eq!(car.light_indices.len(), 2);
    for index in &car.light_indices {
        let lamp = &scene.lights[*index as usize];
        assert!(Vec3::from_array(lamp.position).distance(center) < 3.0);
        assert_eq!(lamp.kind, 1, "spotlight kind");
        assert!(
            lamp.direction[2] * forward > 0.99,
            "spotlight points in vehicle travel direction"
        );
        assert!(lamp.cos_outer > 0.0 && lamp.cos_inner_bits != 0);
    }
}
