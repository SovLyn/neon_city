use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct VertexGpu {
    pub position: [f32; 3],
    pub position_pad: f32,
    pub normal: [f32; 3],
    pub normal_pad: f32,
    pub uv: [f32; 2],
    pub uv_pad: [f32; 2],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct InstanceGpu {
    pub world: [[f32; 4]; 4],
    pub prev_world: [[f32; 4]; 4],
    pub normal_columns: [[f32; 4]; 3],
    pub mesh_id: u32,
    pub material_id: u32,
    pub instance_id: u32,
    pub flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MaterialGpu {
    pub base_color: [f32; 3],
    pub metallic: f32,
    pub emission: [f32; 3],
    pub roughness: f32,
    pub ior: f32,
    pub opacity: f32,
    pub normal_scale: f32,
    pub wetness: f32,
    pub kind: u32,
    pub atlas_region: u32,
    pub bloom_class: u32,
    pub flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct BvhNodeGpu {
    pub min: [f32; 3],
    pub left_or_first: u32,
    pub max: [f32; 3],
    pub right_or_count: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct TriangleGpu {
    pub p0: [f32; 4],
    pub p1: [f32; 4],
    pub p2: [f32; 4],
    pub n0: [f32; 4],
    pub n1: [f32; 4],
    pub n2: [f32; 4],
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
    pub uv2: [f32; 2],
    pub material_id: u32,
    pub flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct DynamicObjectGpu {
    pub world_from_local: [[f32; 4]; 4],
    pub local_from_world: [[f32; 4]; 4],
    pub primitive_kind: u32,
    pub material_id: u32,
    pub instance_id: u32,
    pub flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct LightGpu {
    pub position: [f32; 3],
    pub range: f32,
    pub direction: [f32; 3],
    pub cos_outer: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub kind: u32,
    pub source_id: u32,
    pub cos_inner_bits: u32,
    pub flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct ParticleGpu {
    pub position: [f32; 3],
    pub age: f32,
    pub velocity: [f32; 3],
    pub lifetime: f32,
    pub color: [f32; 3],
    pub opacity: f32,
    pub size: f32,
    pub rotation: f32,
    pub kind_bits: u32,
    pub seed_bits: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct RippleGpu {
    pub center: [f32; 3],
    pub age: f32,
    pub lifetime: f32,
    pub amplitude: f32,
    pub wavelength: f32,
    pub speed: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MaterialStateGpu {
    pub emission_multiplier: [f32; 3],
    pub phase: f32,
    pub fault_amount: f32,
    pub scan_offset: f32,
    pub reactive_weight: f32,
    pub reserved: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct StaticHeaderGpu {
    pub magic: u32,
    pub version: u32,
    pub total_words: u32,
    pub reserved0: u32,
    pub nodes_offset: u32,
    pub nodes_count: u32,
    pub triangles_offset: u32,
    pub triangles_count: u32,
    pub materials_offset: u32,
    pub materials_count: u32,
    pub atlas_rects_offset: u32,
    pub atlas_rects_count: u32,
    pub reserved1: [u32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct DynamicHeaderGpu {
    pub magic: u32,
    pub version: u32,
    pub total_words: u32,
    pub reserved0: u32,
    pub objects_offset: u32,
    pub objects_count: u32,
    pub lights_offset: u32,
    pub lights_count: u32,
    pub material_states_offset: u32,
    pub material_states_count: u32,
    pub reserved1: [u32; 2],
    pub reserved2: [u32; 4],
}
