//! Keep the analytic sky and architectural fill consistent across raster and ray hits.
#[test]
fn raster_and_bvh_share_night_palette_without_exposure_hack() {
    let post = include_str!("../src/shaders/post.wgsl");
    let ray = include_str!("../src/shaders/raytrace.wgsl");
    for color in [
        "vec3<f32>(.012,.022,.055)",
        "vec3<f32>(.075,.035,.105)",
        "vec3<f32>(.22,.28,.40)",
    ] {
        assert!(post.contains(color), "raster palette missing {color}");
        assert!(ray.contains(color), "ray palette missing {color}");
    }
    assert!(post.contains("sky_fill*clamp(a.r,0.,1.)*surface_scale"));
    assert!(ray.contains("color=em+"));
    // Keep non-lighting comparisons independent of the artistic fill.
    assert!(post.contains("if(frame.counts.y!=0u){return Lit"));
}
