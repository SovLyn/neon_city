use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityTier {
    Balanced,
    Performance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DebugView {
    #[default]
    Final,
    Albedo,
    Normal,
    Roughness,
    Depth,
    Motion,
    Reflection,
    AmbientOcclusion,
    Shadow,
    HistoryRejection,
    Bvh,
}
#[derive(Clone, Debug)]
pub struct RenderConfig {
    pub output_size: [u32; 2],
    pub internal_size: [u32; 2],
    pub quality: QualityTier,
    pub debug_view: DebugView,
}
impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            output_size: [1920, 1080],
            internal_size: [1280, 720],
            quality: QualityTier::Balanced,
            debug_view: DebugView::Final,
        }
    }
}
#[derive(Clone, Debug)]
pub struct FrameInput {
    pub frame_index: u32,
    pub time_s: f32,
    pub dt_s: f32,
    pub view_proj: glam::Mat4,
    pub prev_view_proj: glam::Mat4,
    pub inv_view_proj: glam::Mat4,
    pub camera_world: glam::Vec3,
    pub camera_cut: bool,
}
#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    pub cpu_ms: f32,
    pub gpu_ms: Option<f32>,
    pub pass_gpu_ms: Vec<(String, f32)>,
    pub history_reject_ratio: f32,
    pub bvh_stack_overflows: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryResetReason {
    Resize,
    CameraCut,
    QualityChange,
    DeviceRecreated,
    Resume,
}
macro_rules! error_type {
    ($name:ident) => {
        #[derive(Clone, Debug)]
        pub enum $name {
            Capability(String),
            Surface(String),
            DataOutOfBounds(String),
            Asset(String),
            GpuValidation(String),
            Other(String),
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{self:?}")
            }
        }
        impl std::error::Error for $name {}
    };
}
error_type!(SceneError);
error_type!(InitError);
error_type!(RenderError);
