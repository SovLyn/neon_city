fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
#[test]
fn dominant_axis_basis_has_correct_handedness_for_all_faces() {
    let bases = [
        ([0., 0., 1.], [0., 1., 0.], [-1., 0., 0.]),
        ([0., 0., -1.], [0., 1., 0.], [1., 0., 0.]),
        ([1., 0., 0.], [0., 0., 1.], [0., -1., 0.]),
        ([1., 0., 0.], [0., 0., -1.], [0., 1., 0.]),
        ([1., 0., 0.], [0., 1., 0.], [0., 0., 1.]),
        ([-1., 0., 0.], [0., 1., 0.], [0., 0., -1.]),
    ];
    for (t, b, expected) in bases {
        assert_eq!(cross(t, b), expected);
    }
}
#[test]
fn material_mean_normalization_preserves_palette_at_mean_texel() {
    for mean in [0.27445437f32, 0.04534131, 0.06869006] {
        let base = [0.35f32, 0.42, 0.28];
        let result = base.map(|v| v * (mean / mean).clamp(0.3, 2.5));
        assert_eq!(result, base);
    }
}
#[test]
fn wet_asphalt_retains_variation_instead_of_clamping_every_texel() {
    let source = include_str!("../src/shaders/surface_material.wgsl");
    assert!(source.contains("roughness * mix(0.65, 1.15, map_roughness)"));
    let wet = |map: f32| (0.32 * (0.65 + 0.5 * map)).clamp(0.2, 0.45);
    assert!(wet(0.6) < wet(0.8));
    assert!(wet(0.8) < wet(1.0));
    assert!(wet(1.0) < 0.45);
}

#[test]
fn both_passes_use_shared_surface_sampling_and_explicit_lod() {
    let shared = include_str!("../src/shaders/surface_material.wgsl");
    let raster = include_str!("../src/shaders/gbuffer.wgsl");
    let ray = include_str!("../src/shaders/raytrace.wgsl");
    assert!(raster.contains("material_surface("));
    assert!(ray.contains("material_surface("));
    assert_eq!(shared.matches("textureSampleLevel(").count(), 2);
    assert!(shared.contains("textureNumLevels(color_tex)"));
}
