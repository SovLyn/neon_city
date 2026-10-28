// Compiles the owned module even before the integration owner registers it in lib.rs.
use neon_city::contract;
#[path = "../src/animation/mod.rs"]
mod animation;
use animation::AnimationState;
use contract::*;
use glam::{Mat4, Vec3};

fn scene() -> SceneCpu {
    let mut scene = SceneCpu {
        materials: vec![
            MaterialGpu::default(),
            MaterialGpu {
                kind: 1,
                ..Default::default()
            },
            MaterialGpu {
                kind: 5,
                ..Default::default()
            },
        ],
        lights: vec![
            LightGpu {
                position: [-3.0, 1.0, 2.0],
                direction: [0.0, 0.0, 1.0],
                ..Default::default()
            },
            LightGpu {
                position: [10.0, 5.0, 10.0],
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    // An asymmetric, non-rectangular footprint at a deliberately non-special mesh ID.
    scene.meshes = vec![
        MeshData::default(),
        MeshData::default(),
        MeshData {
            vertices: [
                [0.0, 0.0, 0.0],
                [-0.45, 0.0, 0.0],
                [0.0, 0.0, -0.3],
                [0.4, 0.0, 0.0],
                [0.0, 0.0, 0.5],
            ]
            .map(|position| VertexGpu {
                position,
                ..Default::default()
            })
            .to_vec(),
            indices: vec![0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 1],
        },
    ];
    for (id, material, pos, scale, flags) in [
        (0, 0, Vec3::new(-3.0, 0.8, 0.0), Vec3::new(2.0, 1.0, 4.0), 1),
        (1, 0, Vec3::new(4.0, 5.0, 10.0), Vec3::ONE, 1),
        (2, 1, Vec3::new(7.0, 0.0, 13.0), Vec3::new(4.0, 0.1, 8.0), 0),
        (3, 2, Vec3::new(9.0, 4.0, 5.0), Vec3::ONE, 0),
    ] {
        scene.instances.push(InstanceDesc {
            id,
            mesh: if material == 1 { 2 } else { 0 },
            material,
            world_from_local: Mat4::from_scale_rotation_translation(
                scale,
                glam::Quat::from_rotation_y(0.37),
                pos,
            ),
            flags,
        });
    }
    scene.animation_bindings = vec![
        AnimationBinding {
            instance: 0,
            kind: AnimationKind::Vehicle,
            light_indices: vec![0],
            seed: 11,
        },
        AnimationBinding {
            instance: 1,
            kind: AnimationKind::Drone,
            light_indices: vec![],
            seed: 12,
        },
    ];
    scene.signs.push(SignDesc {
        instance: 3,
        text: "OPEN".into(),
        kind: SignKind::Tube,
        color_linear: [1.0; 3],
        intensity: 1.0,
        seed: 50,
    });
    for (kind, rate, seed) in [
        (EmitterKind::Rain, 900.0, 1),
        (EmitterKind::Steam, 40.0, 2),
        (EmitterKind::Steam, 40.0, 3),
        (EmitterKind::Ripple, 10.0, 4),
    ] {
        scene.emitters.push(EmitterDesc {
            kind,
            position: [0.0, if kind == EmitterKind::Rain { 15.0 } else { 0.1 }, 0.0],
            direction: if kind == EmitterKind::Rain {
                [0.0, -1.0, 0.0]
            } else {
                [0.3, 1.0, 0.0]
            },
            rate,
            seed,
        });
    }
    scene
}
fn point_in_mesh(mesh: &MeshData, point: Vec3) -> bool {
    mesh.indices.chunks_exact(3).any(|indices| {
        let [a, b, c] =
            std::array::from_fn(|i| Vec3::from_array(mesh.vertices[indices[i] as usize].position));
        let normal = (b - a).cross(c - a);
        if normal.length_squared() == 0.0 || (point - a).dot(normal).abs() > 1e-5 {
            return false;
        }
        [(a, b), (b, c), (c, a)]
            .iter()
            .all(|&(start, end)| (end - start).cross(point - start).dot(normal) >= -1e-6)
    })
}

fn bytes<T: bytemuck::Pod>(items: &[T]) -> &[u8] {
    bytemuck::cast_slice(items)
}
#[test]
fn replay_and_frame_consistency() {
    let scene = scene();
    let (mut a, mut b) = (
        AnimationState::new(&scene, 42),
        AnimationState::new(&scene, 42),
    );
    let (mut x, mut y) = (DynamicFrame::default(), DynamicFrame::default());
    a.advance(0.0, &mut x);
    b.advance(0.0, &mut y);
    assert_eq!(x.instances.len(), scene.instances.len());
    let mut previous = x.instances.clone();
    for _ in 0..240 {
        a.advance(1.0 / 60.0, &mut x);
        b.advance(1.0 / 60.0, &mut y);
        assert_eq!(bytes(&x.instances), bytes(&y.instances));
        assert_eq!(bytes(&x.particles), bytes(&y.particles));
        assert_eq!(bytes(&x.ripples), bytes(&y.ripples));
        assert_eq!(bytes(&a.material_states()), bytes(&b.material_states()));
        for (now, prev) in x.instances.iter().zip(&previous) {
            assert_eq!(now.prev_world, prev.world);
        }
        for proxy in &x.ray_objects {
            let raster = x
                .instances
                .iter()
                .find(|i| i.instance_id == proxy.instance_id)
                .unwrap();
            assert_eq!(proxy.world_from_local, raster.world);
            let identity = Mat4::from_cols_array_2d(&proxy.local_from_world)
                * Mat4::from_cols_array_2d(&raster.world);
            assert!(identity.abs_diff_eq(Mat4::IDENTITY, 1e-5));
        }
        assert_eq!(x.lights[1].position, scene.lights[1].position);
        let car = Mat4::from_cols_array_2d(&x.instances[0].world);
        let offset = car * scene.instances[0].world_from_local.inverse();
        assert!(Vec3::from_array(x.lights[0].position).abs_diff_eq(
            offset.transform_point3(Vec3::from_array(scene.lights[0].position)),
            1e-5
        ));
        previous = x.instances.clone();
    }
}
#[test]
fn weather_lifetimes_puddles_and_independent_phases() {
    let scene = scene();
    let mut animation = AnimationState::new(&scene, 42);
    let mut frame = DynamicFrame::default();
    let mut saw_steam = false;
    let mut saw_expansion = false;
    let mut saw_fade = false;
    let mut saw_ripple = false;
    let mut initial_rain_seed = None;
    let mut initial_alive_at_end = false;
    for tick in 0..1800 {
        animation.advance(1.0 / 60.0, &mut frame);
        assert!(frame.particles.iter().filter(|p| p.kind_bits == 0).count() <= 4000);
        assert!(frame.particles.iter().filter(|p| p.kind_bits == 1).count() <= 256);
        for p in &frame.particles {
            assert!(p.age >= 0.0 && p.age < p.lifetime);
            if tick == 0 && p.kind_bits == 0 {
                initial_rain_seed = Some(p.seed_bits);
            }
            if tick == 1799 && Some(p.seed_bits) == initial_rain_seed {
                initial_alive_at_end = true;
            }
            if p.kind_bits == 1 {
                saw_steam = true;
                saw_expansion |= p.age / p.lifetime > 0.4 && p.size > 0.4;
                saw_fade |= p.age / p.lifetime > 0.9 && p.opacity < 0.02;
            }
        }
        let rain: Vec<_> = frame
            .particles
            .iter()
            .filter(|p| p.kind_bits == 0)
            .take(2)
            .collect();
        if rain.len() == 2 {
            assert_ne!(rain[0].age, rain[1].age);
        }
        assert!(frame.ripples.len() <= 128);
        for ripple in &frame.ripples {
            saw_ripple = true;
            let local = scene.instances[2]
                .world_from_local
                .inverse()
                .transform_point3(Vec3::from_array(ripple.center));
            assert!(point_in_mesh(
                &scene.meshes[scene.instances[2].mesh as usize],
                local
            ));
            assert!((0.6..=1.8).contains(&ripple.lifetime));
            assert!(ripple.age < ripple.lifetime);
        }
    }
    assert!(saw_steam && saw_expansion && saw_fade && saw_ripple);
    assert!(!initial_alive_at_end);
}
#[test]
fn ripple_mesh_sampling_is_instance_specific_and_keeps_other_streams_independent() {
    let scene = scene();
    let mut more = scene.clone();
    // A different triangle mesh and transform must use its own footprint, not mesh 2's.
    more.meshes.push(MeshData {
        vertices: [[-0.2, 0.0, -0.4], [0.3, 0.0, -0.1], [0.1, 0.0, 0.45]]
            .map(|position| VertexGpu {
                position,
                ..Default::default()
            })
            .to_vec(),
        indices: vec![0, 1, 2],
    });
    more.instances.push(InstanceDesc {
        id: 200,
        mesh: 3,
        material: 1,
        flags: 0,
        world_from_local: Mat4::from_scale_rotation_translation(
            Vec3::new(3.0, 0.2, 5.0),
            glam::Quat::from_rotation_y(-0.8),
            Vec3::new(-10.0, 0.03, -15.0),
        ),
    });
    let (mut a, mut b) = (
        AnimationState::new(&scene, 42),
        AnimationState::new(&more, 42),
    );
    let (mut x, mut y) = (DynamicFrame::default(), DynamicFrame::default());
    let mut visited = [false; 2];
    for _ in 0..1800 {
        a.advance(1.0 / 60.0, &mut x);
        b.advance(1.0 / 60.0, &mut y);
        assert_eq!(bytes(&x.particles), bytes(&y.particles));
        assert_eq!(bytes(&a.material_states()), bytes(&b.material_states()));
        for ripple in &y.ripples {
            let mut inside = false;
            for (slot, index) in [2, more.instances.len() - 1].into_iter().enumerate() {
                let instance = &more.instances[index];
                let local = instance
                    .world_from_local
                    .inverse()
                    .transform_point3(Vec3::from_array(ripple.center));
                if point_in_mesh(&more.meshes[instance.mesh as usize], local) {
                    visited[slot] = true;
                    inside = true;
                }
            }
            assert!(inside, "ripple centre is outside both real triangle meshes");
        }
    }
    assert!(visited.into_iter().all(|seen| seen));

    // Missing, empty, and zero-area geometry must not fall back to an AABB.
    for mesh in [0, 4, u32::MAX] {
        more.meshes.truncate(4);
        more.meshes.push(MeshData {
            vertices: vec![VertexGpu::default()],
            indices: vec![0, 0, 0],
        });
        for instance in &mut more.instances {
            if instance.material == 1 {
                instance.mesh = mesh;
            }
        }
        let mut empty = AnimationState::new(&more, 42);
        for _ in 0..60 {
            empty.advance(1.0 / 60.0, &mut y);
            assert!(y.ripples.is_empty());
        }
    }
}

#[test]
fn sign_stream_does_not_depend_on_traffic_count_and_resume_is_bounded() {
    let scene = scene();
    let mut more = scene.clone();
    more.instances.push(InstanceDesc {
        id: 100,
        mesh: 0,
        material: 0,
        flags: 1,
        world_from_local: Mat4::from_translation(Vec3::new(2.0, 1.0, 0.0)),
    });
    more.animation_bindings.push(AnimationBinding {
        instance: 100,
        kind: AnimationKind::Vehicle,
        light_indices: vec![],
        seed: 78,
    });
    let (mut a, mut b) = (
        AnimationState::new(&scene, 3),
        AnimationState::new(&more, 3),
    );
    let mut frame = DynamicFrame::default();
    for _ in 0..900 {
        a.advance(1.0 / 60.0, &mut frame);
        b.advance(1.0 / 60.0, &mut frame);
        assert_eq!(bytes(&a.material_states()), bytes(&b.material_states()));
    }
    let (mut a, mut b) = (
        AnimationState::new(&scene, 3),
        AnimationState::new(&scene, 3),
    );
    let mut x = DynamicFrame::default();
    a.advance(600.0, &mut frame);
    b.advance(8.0 / 60.0, &mut x);
    assert_eq!(bytes(&frame.instances), bytes(&x.instances));
    assert_eq!(bytes(&frame.particles), bytes(&x.particles));
    a.advance(f32::NAN, &mut frame);
    assert_eq!(bytes(&frame.instances), bytes(&x.instances));
}

#[test]
fn sign_lights_share_material_multiplier_without_compounding() {
    let mut scene = scene();
    scene.lights[1].color = [0.8, 0.25, 0.6];
    scene.lights[1].intensity = 17.0;
    scene.animation_bindings.push(AnimationBinding {
        instance: 3,
        kind: AnimationKind::Sign,
        light_indices: vec![1, u32::MAX],
        seed: 50,
    });
    let base = scene.lights[1];
    let mut state = AnimationState::new(&scene, 42);
    let mut frame = DynamicFrame::default();
    let mut saw_dim = false;
    let mut saw_recovery = false;
    for _ in 0..1800 {
        state.advance(1.0 / 60.0, &mut frame);
        let multiplier = state.material_states()[2].emission_multiplier;
        saw_recovery |= saw_dim && multiplier[0] == 1.0;
        saw_dim |= multiplier[0] < 0.5;
        let expected: [f32; 3] =
            std::array::from_fn(|channel| base.color[channel] * multiplier[channel]);
        assert_eq!(frame.lights[1].color, expected);
        assert_eq!(frame.lights[1].intensity, base.intensity);
        assert_eq!(frame.lights[1].position, base.position);
        // A render without another simulation tick must not multiply a second time.
        state.advance(0.0, &mut frame);
        assert_eq!(frame.lights[1].color, expected);
        assert_eq!(frame.lights[1].intensity, base.intensity);
    }
    assert!(saw_dim && saw_recovery);
}

#[test]
fn traffic_speed_caps_and_sign_events_are_independent() {
    let mut scene = scene();
    scene.materials.push(MaterialGpu {
        kind: 5,
        ..Default::default()
    });
    scene.instances.push(InstanceDesc {
        id: 4,
        mesh: 0,
        material: 3,
        flags: 0,
        world_from_local: Mat4::IDENTITY,
    });
    let mut second = scene.signs[0].clone();
    second.instance = 4;
    second.seed = 999;
    scene.signs.push(second);
    for id in 100..140 {
        scene.instances.push(InstanceDesc {
            id,
            mesh: 0,
            material: 0,
            flags: 1,
            world_from_local: Mat4::IDENTITY,
        });
    }
    let mut state = AnimationState::new(&scene, 8);
    let mut frame = DynamicFrame::default();
    let mut independent_fault = false;
    for tick in 0..1800 {
        state.advance(1.0 / 60.0, &mut frame);
        assert_eq!(frame.ray_objects.len(), 32);
        assert_eq!(frame.instances.len(), scene.instances.len());
        if tick > 0 {
            let current = Mat4::from_cols_array_2d(&frame.instances[0].world).w_axis.z;
            let previous = Mat4::from_cols_array_2d(&frame.instances[0].prev_world)
                .w_axis
                .z;
            if current != previous {
                let speed = (current - previous).abs() * 60.0;
                assert!((4.99..=12.01).contains(&speed));
            }
        }
        let states = state.material_states();
        independent_fault |= states[2].fault_amount != states[3].fault_amount;
    }
    assert!(independent_fault);
    // Emitters alone cannot make a dry surface ripple.
    scene.materials[1].kind = 2;
    let mut dry = AnimationState::new(&scene, 8);
    for _ in 0..60 {
        dry.advance(1.0 / 60.0, &mut frame);
    }
    assert!(frame.ripples.is_empty());
}
