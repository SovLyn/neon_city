use super::gpu::*;
pub type MeshId = u32;
pub type MaterialId = u32;
pub type InstanceId = u32;
#[derive(Clone, Debug)]
pub struct SceneConfig {
    pub seed: u64,
    pub street_length_m: f32,
    pub street_width_m: f32,
    pub buildings_per_side: u32,
}
impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            seed: 0x4e454f4e,
            street_length_m: 120.0,
            street_width_m: 24.0,
            buildings_per_side: 12,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub vertices: Vec<VertexGpu>,
    pub indices: Vec<u32>,
}
#[derive(Clone, Debug)]
pub struct InstanceDesc {
    pub id: InstanceId,
    pub mesh: MeshId,
    pub material: MaterialId,
    pub world_from_local: glam::Mat4,
    pub flags: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SignKind {
    Tube = 1,
    Led = 2,
    Hologram = 3,
    FaultText = 4,
}
#[derive(Clone, Debug)]
pub struct SignDesc {
    pub instance: InstanceId,
    pub text: String,
    pub kind: SignKind,
    pub color_linear: [f32; 3],
    pub intensity: f32,
    pub seed: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmitterKind {
    Rain,
    Steam,
    Ripple,
}
#[derive(Clone, Debug)]
pub struct EmitterDesc {
    pub kind: EmitterKind,
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub rate: f32,
    pub seed: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationKind {
    Vehicle,
    Drone,
    Sign,
}
#[derive(Clone, Debug)]
pub struct AnimationBinding {
    pub instance: InstanceId,
    pub kind: AnimationKind,
    pub light_indices: Vec<u32>,
    pub seed: u64,
}
#[derive(Clone, Debug, Default)]
pub struct SceneCpu {
    pub meshes: Vec<MeshData>,
    pub instances: Vec<InstanceDesc>,
    pub materials: Vec<MaterialGpu>,
    pub lights: Vec<LightGpu>,
    pub signs: Vec<SignDesc>,
    pub emitters: Vec<EmitterDesc>,
    pub animation_bindings: Vec<AnimationBinding>,
}
#[derive(Clone, Debug)]
pub enum BuildPoll<T> {
    Pending { progress: f32 },
    Ready(T),
}
#[derive(Clone, Debug, Default)]
pub struct PackedScene {
    pub words: Vec<u32>,
}
#[derive(Clone, Debug, Default)]
pub struct DynamicFrame {
    pub instances: Vec<InstanceGpu>,
    pub ray_objects: Vec<DynamicObjectGpu>,
    pub lights: Vec<LightGpu>,
    pub particles: Vec<ParticleGpu>,
    pub ripples: Vec<RippleGpu>,
    pub reactive_material_ids: Vec<MaterialId>,
}
