pub use neon_city::contract;
#[path = "../src/render/temporal.rs"]
#[allow(dead_code)]
mod temporal;

#[test]
fn temporal_shader_validates() {
    for source in [
        include_str!("../src/shaders/temporal.wgsl"),
        include_str!("../src/shaders/temporal_filter.wgsl"),
    ] {
        let module = naga::front::wgsl::parse_str(source).expect("parse temporal WGSL");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("validate temporal WGSL");
    }
}
