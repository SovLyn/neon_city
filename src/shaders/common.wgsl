// Contract V1. Matrices are column-major; storage lanes are 16 bytes.
const CONTRACT_VERSION: u32 = 1u;
struct VertexGpu { position: vec4<f32>, normal: vec4<f32>, uv: vec4<f32> };
struct InstanceGpu { world: mat4x4<f32>, prev_world: mat4x4<f32>, normal_0: vec4<f32>, normal_1: vec4<f32>, normal_2: vec4<f32>, ids: vec4<u32> };
struct MaterialGpu { base_metallic: vec4<f32>, emission_roughness: vec4<f32>, optical: vec4<f32>, ids: vec4<u32> };
struct BvhNodeGpu { min: vec3<f32>, left_or_first: u32, max: vec3<f32>, right_or_count: u32 };
struct TriangleGpu { p0: vec4<f32>, p1: vec4<f32>, p2: vec4<f32>, n0: vec4<f32>, n1: vec4<f32>, n2: vec4<f32>, uv01: vec4<f32>, uv2: vec2<f32>, material_id: u32, flags: u32 };
struct DynamicObjectGpu { world_from_local: mat4x4<f32>, local_from_world: mat4x4<f32>, ids: vec4<u32> };
struct LightGpu { position_range: vec4<f32>, direction_outer: vec4<f32>, color_intensity: vec4<f32>, ids: vec4<u32> };
struct ParticleGpu { position_age: vec4<f32>, velocity_lifetime: vec4<f32>, color_opacity: vec4<f32>, params: vec4<u32> };
struct RippleGpu { center_age: vec4<f32>, params: vec4<f32> };
