#[test]
fn gbuffer_uses_explicit_lod_for_conditional_atlas_sampling() {
    let source = include_str!("../src/shaders/gbuffer.wgsl");
    assert!(!source.contains("textureSample("));
    assert!(source.contains("sign_emission("));
    assert!(include_str!("../src/shaders/material_effects.wgsl").contains("textureSampleLevel("));
    assert!(source.contains("material_surface("));
}

#[test]
fn gbuffer_emits_reprojection_motion_and_previous_clip_w() {
    let source = include_str!("../src/shaders/gbuffer.wgsl");
    assert!(source.contains("prev_uv-current_uv,i.prev.w"));
    assert!(source.contains("camera.prev_view_proj*inst.prev_world"));
}

#[test]
fn raytrace_keeps_reflections_ao_and_ordered_bvh() {
    let source = include_str!("../src/shaders/raytrace.wgsl");
    assert!(source.contains("if (rough<=0.55)"));
    assert!(source.contains("hit=trace(origin,dir,200.0,false)"));
    assert!(source.contains("trace(origin,ao_dir,2.0,true)"));
    assert!(source.contains("entry > result.t"));
    assert!(source.contains("if (left_t <= right_t)"));
    assert!(source.contains("if (occlusion) { return result; }"));
    assert!(source.contains("dynamic[l+12u]==1u"));
    assert!(source.contains("min(dynamic[7u],64u)"));
}

#[test]
fn shaders_validate() {
    for (name, source) in [
        (
            "raytrace",
            format!(
                "{}\n{}\n{}\n{}",
                include_str!("../src/shaders/common.wgsl"),
                include_str!("../src/shaders/material_effects.wgsl"),
                include_str!("../src/shaders/raytrace.wgsl"),
                include_str!("../src/shaders/surface_material.wgsl")
            ),
        ),
        (
            "gbuffer",
            format!(
                "{}\n{}\n{}\n{}",
                include_str!("../src/shaders/common.wgsl"),
                include_str!("../src/shaders/material_effects.wgsl"),
                include_str!("../src/shaders/gbuffer.wgsl"),
                include_str!("../src/shaders/surface_material.wgsl")
            ),
        ),
        (
            "composite",
            include_str!("../src/shaders/composite.wgsl").to_owned(),
        ),
        (
            "temporal",
            include_str!("../src/shaders/temporal.wgsl").to_owned(),
        ),
        ("post", include_str!("../src/shaders/post.wgsl").to_owned()),
    ] {
        let module = naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    }
}
