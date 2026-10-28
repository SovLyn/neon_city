pub use neon_city::contract;
#[path = "../src/accel/mod.rs"]
mod accel;
use accel::{BvhBuild, reference::*, set_atlas_rects};
use contract::*;
use glam::{Mat4, Vec3};

fn fixture() -> SceneCpu {
    let mut mesh = MeshData::default();
    for z in 0..12 {
        for x in 0..12 {
            let base = mesh.vertices.len() as u32;
            for p in [
                [x as f32, 0., z as f32],
                [x as f32 + 0.8, 0., z as f32],
                [x as f32, 0., z as f32 + 0.8],
            ] {
                mesh.vertices.push(VertexGpu {
                    position: p,
                    normal: [0., 1., 0.],
                    ..Default::default()
                });
            }
            mesh.indices.extend([base, base + 1, base + 2]);
        }
    }
    SceneCpu {
        meshes: vec![mesh],
        materials: vec![MaterialGpu::default()],
        instances: vec![InstanceDesc {
            id: 0,
            mesh: 0,
            material: 0,
            world_from_local: Mat4::from_scale_rotation_translation(
                Vec3::new(2., 1., 0.5),
                glam::Quat::IDENTITY,
                Vec3::new(1., 2., 3.),
            ),
            flags: 2,
        }],
        ..Default::default()
    }
}
fn build(scene: &SceneCpu) -> PackedScene {
    let mut b = BvhBuild::new(scene).unwrap();
    loop {
        if let BuildPoll::Ready(p) = b.step(13).unwrap() {
            return p;
        }
    }
}
fn decode<T: bytemuck::Pod>(w: &[u32], start: usize, count: usize) -> Vec<T> {
    let size = std::mem::size_of::<T>() / 4;
    w[start..start + count * size]
        .chunks_exact(size)
        .map(|x| {
            let bytes: Vec<u8> = x.iter().flat_map(|v| v.to_le_bytes()).collect();
            bytemuck::pod_read_unaligned(&bytes)
        })
        .collect()
}
fn unpack(p: &PackedScene) -> (Vec<BvhNodeGpu>, Vec<TriangleGpu>) {
    let w = &p.words;
    (
        decode(w, w[4] as usize, w[5] as usize),
        decode(w, w[6] as usize, w[7] as usize),
    )
}

#[test]
fn compare_ten_thousand_rays() {
    let p = build(&fixture());
    let w = &p.words;
    let nodes: Vec<BvhNodeGpu> = decode(w, w[4] as usize, w[5] as usize);
    let triangles: Vec<TriangleGpu> = decode(w, w[6] as usize, w[7] as usize);
    assert_eq!(triangles.len(), 144);
    let mut s = 1234567u32;
    for _ in 0..10000 {
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s as f32 / u32::MAX as f32
        };
        let ray = Ray {
            origin: Vec3::new(next() * 28. - 1., 8., next() * 9.),
            direction: Vec3::new((next() - 0.5) * 0.2, -1., (next() - 0.5) * 0.2),
            t_min: 0.001,
            t_max: 100.,
        };
        let a = brute_force(ray, &triangles, false);
        let b = traverse(ray, &nodes, &triangles, false);
        assert_eq!(a.is_some(), b.is_some());
        if let (Some(a), Some(b)) = (a, b) {
            assert!((a.t - b.t).abs() < 1e-4);
        }
    }
}
#[test]
fn patch_and_invalid_indices() {
    let mut p = build(&fixture());
    set_atlas_rects(&mut p, &[[0.1, 0.2, 0.8, 0.9]]).unwrap();
    assert_eq!(p.words[2] as usize, p.words.len());
    let before = p.words.clone();
    assert!(set_atlas_rects(&mut p, &[]).is_err());
    assert_eq!(p.words, before);
    let mut scene = fixture();
    scene.meshes[0].indices[0] = u32::MAX;
    let mut b = BvhBuild::new(&scene).unwrap();
    assert!(b.step(1).is_err());
}

#[test]
fn shared_atlas_regions() {
    let mut scene = fixture();
    scene.materials = (0..30)
        .map(|i| MaterialGpu {
            atlas_region: i % 2,
            ..Default::default()
        })
        .collect();
    let mut packed = build(&scene);
    let rects = [[0.0, 0.0, 0.5, 1.0], [0.5, 0.0, 1.0, 1.0]];
    set_atlas_rects(&mut packed, &rects).unwrap();
    assert_eq!(packed.words[11], 2);
    let before = packed.words.clone();
    assert!(set_atlas_rects(&mut packed, &rects[..1]).is_err());
    assert_eq!(packed.words, before);
}

#[test]
fn empty_dynamic_and_degenerate() {
    let empty = build(&SceneCpu::default());
    assert_eq!((empty.words[5], empty.words[7]), (0, 0));
    let (n, t) = unpack(&empty);
    let ray = Ray {
        origin: Vec3::ZERO,
        direction: Vec3::Y,
        t_min: 0.0,
        t_max: 10.0,
    };
    assert!(traverse(ray, &n, &t, false).is_none());
    let mut scene = fixture();
    scene.instances[0].flags = 1;
    assert_eq!(build(&scene).words[7], 0);
    scene.instances[0].flags = 2;
    scene.meshes[0].indices[1] = scene.meshes[0].indices[0];
    assert_eq!(build(&scene).words[7], 143);
}

#[test]
fn winding_sidedness_shadow_and_axis_parallel() {
    let mut scene = fixture();
    scene.meshes[0].indices.truncate(3);
    scene.instances[0].world_from_local = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    scene.instances[0].flags = 0;
    let (nodes, triangles) = unpack(&build(&scene));
    let tri = &triangles[0];
    let point = |p: [f32; 4]| Vec3::new(p[0], p[1], p[2]);
    // Mirroring preserves transformed vertex order; geometric winding changes,
    // while inverse-transpose shading normals retain their transformed direction.
    assert!(
        (point(tri.p1) - point(tri.p0))
            .cross(point(tri.p2) - point(tri.p0))
            .dot(point(tri.n0))
            > 0.0
    );
    let down = Ray {
        origin: Vec3::new(-0.1, 1.0, 0.1),
        direction: -Vec3::Y,
        t_min: 0.0,
        t_max: 10.0,
    };
    let up = Ray {
        origin: Vec3::new(-0.1, -1.0, 0.1),
        direction: Vec3::Y,
        ..down
    };
    assert!(traverse(down, &nodes, &triangles, false).is_some());
    assert!(traverse(up, &nodes, &triangles, false).is_none());
    let mut double = triangles.clone();
    double[0].flags = 2 | 4;
    assert!(traverse(up, &nodes, &double, false).is_some());
    assert!(traverse(up, &nodes, &double, true).is_none());
    let miss = Ray {
        origin: Vec3::new(2.0, 1.0, 0.1),
        ..down
    };
    assert!(traverse(miss, &nodes, &double, false).is_none());
    assert!(
        aabb(
            Ray {
                origin: Vec3::ZERO,
                direction: Vec3::Y,
                t_min: 0.0,
                t_max: 10.0
            },
            Vec3::splat(-1.0),
            Vec3::splat(1.0)
        )
        .is_some()
    );
}

/// Run separately with --release --ignored --nocapture. This measures individual
/// step(1) calls; it is not a browser main-thread or 8ms compliance test.
#[test]
#[ignore]
fn benchmark_100k_triangles() {
    use std::time::Instant;
    let mut scene = fixture();
    scene.instances.clear();
    for i in 0..695 {
        scene.instances.push(InstanceDesc {
            id: i,
            mesh: 0,
            material: 0,
            world_from_local: Mat4::from_translation(Vec3::new(
                (i % 25) as f32 * 30.0,
                (i % 7) as f32,
                (i / 25) as f32 * 10.0,
            )),
            flags: 2,
        });
    }
    let start = Instant::now();
    let mut builder = BvhBuild::new(&scene).unwrap();
    let new_time = start.elapsed();
    let mut max = std::time::Duration::ZERO;
    let mut over = 0usize;
    let mut count = 0usize;
    let total = Instant::now();
    let packed = loop {
        let start = Instant::now();
        let result = builder.step(1).unwrap();
        let elapsed = start.elapsed();
        max = max.max(elapsed);
        count += 1;
        if elapsed.as_secs_f64() > 0.008 {
            over += 1;
        }
        if let BuildPoll::Ready(p) = result {
            break p;
        }
    };
    eprintln!(
        "triangles={} new_ms={:.3} max_step_ms={:.3} steps_over_8ms={} steps={} total_ms={:.3}",
        packed.words[7],
        new_time.as_secs_f64() * 1000.0,
        max.as_secs_f64() * 1000.0,
        over,
        count,
        total.elapsed().as_secs_f64() * 1000.0
    );
}
