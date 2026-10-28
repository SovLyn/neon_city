// Compile the isolated module before renderer integration, on both host and wasm.
mod contract {
    pub use neon_city::contract::*;
}
#[allow(dead_code)]
#[path = "../src/render/post.rs"]
mod post;

#[test]
fn effects_api_keeps_independent_boolean_switches() {
    let _: fn(&mut post::Post, bool, bool) = post::Post::set_effects;
}

#[test]
fn gpu_struct_layouts_match_contract() {
    use neon_city::contract::{LightGpu, MaterialGpu, ParticleGpu};
    use std::mem::{offset_of, size_of};
    let module = naga::front::wgsl::parse_str(include_str!("../src/shaders/post.wgsl")).unwrap();
    let check = |name: &str, size: usize, expected: &[(&str, usize)]| {
        let ty = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some(name))
            .unwrap()
            .1;
        let naga::TypeInner::Struct { members, span } = &ty.inner else {
            panic!("not a struct")
        };
        assert_eq!(*span as usize, size, "{name} stride");
        assert_eq!(members.len(), expected.len());
        for (member, (field, offset)) in members.iter().zip(expected) {
            assert_eq!(member.name.as_deref(), Some(*field));
            assert_eq!(member.offset as usize, *offset, "{name}.{field}");
        }
    };
    check(
        "Light",
        size_of::<LightGpu>(),
        &[
            ("position", offset_of!(LightGpu, position)),
            ("range", offset_of!(LightGpu, range)),
            ("direction", offset_of!(LightGpu, direction)),
            ("cos_outer", offset_of!(LightGpu, cos_outer)),
            ("color", offset_of!(LightGpu, color)),
            ("intensity", offset_of!(LightGpu, intensity)),
            ("kind", offset_of!(LightGpu, kind)),
            ("source_id", offset_of!(LightGpu, source_id)),
            ("cos_inner_bits", offset_of!(LightGpu, cos_inner_bits)),
            ("flags", offset_of!(LightGpu, flags)),
        ],
    );
    check(
        "Material",
        size_of::<MaterialGpu>(),
        &[
            ("base_color", offset_of!(MaterialGpu, base_color)),
            ("metallic", offset_of!(MaterialGpu, metallic)),
            ("emission", offset_of!(MaterialGpu, emission)),
            ("roughness", offset_of!(MaterialGpu, roughness)),
            ("ior", offset_of!(MaterialGpu, ior)),
            ("opacity", offset_of!(MaterialGpu, opacity)),
            ("normal_scale", offset_of!(MaterialGpu, normal_scale)),
            ("wetness", offset_of!(MaterialGpu, wetness)),
            ("kind", offset_of!(MaterialGpu, kind)),
            ("atlas_region", offset_of!(MaterialGpu, atlas_region)),
            ("bloom_class", offset_of!(MaterialGpu, bloom_class)),
            ("flags", offset_of!(MaterialGpu, flags)),
        ],
    );
    check(
        "Particle",
        size_of::<ParticleGpu>(),
        &[
            ("position", offset_of!(ParticleGpu, position)),
            ("age", offset_of!(ParticleGpu, age)),
            ("velocity", offset_of!(ParticleGpu, velocity)),
            ("lifetime", offset_of!(ParticleGpu, lifetime)),
            ("color", offset_of!(ParticleGpu, color)),
            ("opacity", offset_of!(ParticleGpu, opacity)),
            ("size", offset_of!(ParticleGpu, size)),
            ("rotation", offset_of!(ParticleGpu, rotation)),
            ("kind_bits", offset_of!(ParticleGpu, kind_bits)),
            ("seed_bits", offset_of!(ParticleGpu, seed_bits)),
        ],
    );
    check(
        "Frame",
        size_of::<post::FrameUniform>(),
        &[
            ("inv_vp", offset_of!(post::FrameUniform, inv_vp)),
            ("vp", offset_of!(post::FrameUniform, vp)),
            ("camera_time", offset_of!(post::FrameUniform, camera_time)),
            ("counts", offset_of!(post::FrameUniform, counts)),
            ("effects", offset_of!(post::FrameUniform, effects)),
        ],
    );
}

#[test]
fn output_encoding_is_surface_dependent_and_shared_by_debug_views() {
    let rust = include_str!("../src/render/post.rs");
    let shader = include_str!("../src/shaders/post.wgsl");
    assert!(rust.contains("encode_srgb: !format.is_srgb()"));
    assert!(rust.contains("u32::from(self.encode_srgb)"));
    assert_eq!(std::mem::size_of::<post::FrameUniform>(), 176);
    // Regression guard: conversion follows tone mapping, outside the debug/final branches.
    let finish = shader.split("@fragment fn finish").nth(1).unwrap();
    let finish = finish.split("struct POut").next().unwrap();
    assert!(
        finish.contains("if(frame.effects.z!=0u){c=linear_to_srgb(c);}\n return vec4<f32>(c,1);")
    );
    // Exact piecewise sRGB transfer, not an approximate pow(1/2.2).
    assert!(
        shader
            .contains("select(1.055*pow(c,vec3<f32>(1./2.4))-.055,12.92*c,c<=vec3<f32>(.0031308))")
    );
}

#[test]
fn auxiliary_metadata_is_not_used_as_shadow_data() {
    let source = include_str!("../src/shaders/post.wgsl");
    for metadata in ["a.g", "a.b", "a.a"] {
        assert!(
            !source.contains(metadata),
            "unexpected aux metadata use: {metadata}"
        );
    }
    assert!(source.contains("case 8u:{c=vec3<f32>(1);}"));
}

#[test]
fn light_rejection_precedes_expensive_brdf_work() {
    let source = include_str!("../src/shaders/post.wgsl");
    let lighting = source.split("@fragment fn lighting").nth(1).unwrap();
    let range_reject = lighting
        .find("if(light.range<=0. || dist2>light.range*light.range){continue;}")
        .unwrap();
    let direction = lighting.find("let l=delta*inverseSqrt(d2)").unwrap();
    let cone_reject = lighting
        .find("if(angle<=light.cos_outer){continue;}")
        .unwrap();
    let half_vector = lighting.find("let h=safe_normalize(l+v)").unwrap();
    let specular = lighting.find("let spec=").unwrap();
    assert!(range_reject < direction);
    assert!(direction < cone_reject && cone_reject < half_vector && half_vector < specular);
    let lighting = lighting.split("@fragment fn downsample").next().unwrap();
    assert!(!lighting.contains("=normalize("));
    assert!(source.contains("inverseSqrt(max(dot(v,v),1e-12))"));
}

#[test]
fn debug_returns_before_lighting_and_bloom_passes_are_conditional() {
    let source = include_str!("../src/shaders/post.wgsl");
    let lighting = source.split("@fragment fn lighting").nth(1).unwrap();
    let early_return = lighting
        .find("if(frame.counts.y!=0u){return Lit(vec4<f32>(debug_color(i.uv),1),vec4<f32>(0));}")
        .unwrap();
    assert!(early_return < lighting.find("let p=coord").unwrap());
    let debug = source.split("fn debug_color").nth(1).unwrap();
    let debug = debug.split("@fragment fn lighting").next().unwrap();
    for final_only in ["lights[", "shafts", "fog", "bloom", "emission"] {
        assert!(!debug.contains(final_only));
    }
    let rust = include_str!("../src/render/post.rs");
    assert!(rust.contains(
        "if self.bloom_enabled && debug == DebugView::Final {\n            for level in 0..3 {"
    ));
    assert!(rust.contains(
        "if self.atmosphere_enabled && debug == DebugView::Final && !dynamic.particles.is_empty()"
    ));
}

#[test]
fn squared_distance_falloff_preserves_quartic_attenuation() {
    for range in [0.01_f32, 0.5, 2.0, 20.0, 100.0] {
        for fraction in [0.0_f32, 0.01, 0.2, 0.5, 0.99, 1.0] {
            let d2 = ((range * fraction).powi(2)).max(0.04);
            let before = (1.0 - (d2.sqrt() / range).powi(4)).clamp(0.0, 1.0).powi(2);
            let ratio = d2 / (range * range);
            let after = (1.0 - ratio * ratio).clamp(0.0, 1.0).powi(2);
            assert!((before - after).abs() < 1e-6);
        }
    }
}

#[test]
fn material_api_and_empty_table_keep_existing_post_resources_safe() {
    let _: fn(&mut post::Post, &wgpu::Device, &[neon_city::contract::MaterialGpu]) =
        post::Post::set_materials;
    let rust = include_str!("../src/render/post.rs");
    assert_eq!(std::mem::size_of::<neon_city::contract::MaterialGpu>(), 64);
    assert!(rust.contains("let zero = [0; 64]"));
    assert!(rust.contains("material_count: 0"));
    assert!(rust.contains("self.material_count = materials.len() as u32"));
    assert!(rust.contains("self.material_count,"));
    assert!(
        include_str!("../src/render/mod.rs").contains("post.set_materials(d, &scene.materials)")
    );
}

#[test]
fn glazing_uses_bounded_g3_material_lookup_and_only_two_lighting_storage_buffers() {
    let source = include_str!("../src/shaders/post.wgsl");
    let module = naga::front::wgsl::parse_str(source).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap();
    let index = module
        .entry_points
        .iter()
        .position(|e| e.name == "lighting")
        .unwrap();
    let lighting_info = info.get_entry_point(index);
    let storage: Vec<_> = module
        .global_variables
        .iter()
        .filter(|(handle, var)| {
            matches!(var.space, naga::AddressSpace::Storage { .. })
                && !lighting_info[*handle].is_empty()
        })
        .map(|(_, var)| {
            (
                var.name.as_deref().unwrap(),
                var.binding.as_ref().unwrap().binding,
            )
        })
        .collect();
    assert_eq!(storage, [("lights", 9), ("materials", 18)]);
    let materials = module
        .global_variables
        .iter()
        .find(|(_, v)| v.name.as_deref() == Some("materials"))
        .unwrap()
        .1;
    assert_eq!(
        materials.space,
        naga::AddressSpace::Storage {
            access: naga::StorageAccess::LOAD
        }
    );
    assert!(source.contains(
        "material_id>=0. && material_id<f32(min(frame.effects.w,arrayLength(&materials)))"
    ));
    assert!(source.contains("textureLoad(motion,coord(i.uv,textureDimensions(motion)),0).w"));
    assert!(source.contains("material.kind==4u"));
    assert!(source.contains("let dielectric=(ior-1.)/(ior+1.)"));
    assert!(source.contains("opacity=clamp(material.opacity,0.,1.)"));
    assert!(source.contains("(1-opacity)*(1-fresnel)*shop_interior(w,n,v)*tint"));
    assert!(source.contains("surface_emission=vec3<f32>(0)"));
    assert!(source.contains("let reflected_energy=r*fresnel*(1-.65*rough)"));
    assert!(source.contains("Procedural shallow shop-interior approximation"));
}

#[test]
fn glazing_fresnel_and_opacity_partition_energy() {
    let ior = 1.45_f32;
    let f0 = ((ior - 1.) / (ior + 1.)).powi(2);
    assert!((f0 - 0.03373594).abs() < 1e-6);
    for cosine in [0.0_f32, 0.2, 0.5, 1.0] {
        let fresnel = f0 + (1. - f0) * (1. - cosine).powi(5);
        for opacity in [0.0, 0.22, 1.0] {
            let surface = opacity * (1. - fresnel);
            let transmission = (1. - opacity) * (1. - fresnel);
            assert!((surface + transmission + fresnel - 1.).abs() < 1e-6);
        }
    }
}

#[test]
fn post_shader_validates_with_uniformity_checks() {
    let source = include_str!("../src/shaders/post.wgsl");
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{}", e.emit_to_string(source)));
    for entry in [
        "lighting",
        "downsample",
        "blur_h",
        "blur_v",
        "finish",
        "particle_vs",
        "particle_fs",
    ] {
        assert!(module.entry_points.iter().any(|e| e.name == entry));
    }
}
