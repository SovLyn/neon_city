use neon_city::contract::*;
use std::mem::{offset_of, size_of};
#[test]
fn gpu_sizes_and_offsets() {
    assert_eq!(size_of::<VertexGpu>(), 48);
    assert_eq!(offset_of!(VertexGpu, uv), 32);
    assert_eq!(size_of::<InstanceGpu>(), 192);
    assert_eq!(offset_of!(InstanceGpu, mesh_id), 176);
    assert_eq!(size_of::<MaterialGpu>(), 64);
    assert_eq!(offset_of!(MaterialGpu, kind), 48);
    assert_eq!(size_of::<BvhNodeGpu>(), 32);
    assert_eq!(offset_of!(BvhNodeGpu, right_or_count), 28);
    assert_eq!(size_of::<TriangleGpu>(), 128);
    assert_eq!(offset_of!(TriangleGpu, material_id), 120);
    assert_eq!(size_of::<DynamicObjectGpu>(), 144);
    assert_eq!(offset_of!(DynamicObjectGpu, primitive_kind), 128);
    assert_eq!(size_of::<LightGpu>(), 64);
    assert_eq!(offset_of!(LightGpu, kind), 48);
    assert_eq!(size_of::<ParticleGpu>(), 64);
    assert_eq!(offset_of!(ParticleGpu, kind_bits), 56);
    assert_eq!(size_of::<RippleGpu>(), 32);
    assert_eq!(size_of::<MaterialStateGpu>(), 32);
    assert_eq!(size_of::<StaticHeaderGpu>(), 64);
    assert_eq!(size_of::<DynamicHeaderGpu>(), 64);
}
