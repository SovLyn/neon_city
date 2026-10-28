pub use crate::contract::frame::{DebugView, QualityTier, RenderConfig};

#[cfg(target_arch = "wasm32")]
pub fn parameter(name: &str) -> Option<String> {
    let query = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&query)
        .ok()?
        .get(name)
}

#[cfg(target_arch = "wasm32")]
pub fn debug_view() -> DebugView {
    match parameter("debug").as_deref() {
        Some("albedo") => DebugView::Albedo,
        Some("normal") => DebugView::Normal,
        Some("roughness") => DebugView::Roughness,
        Some("depth") => DebugView::Depth,
        Some("motion") => DebugView::Motion,
        Some("reflection") => DebugView::Reflection,
        Some("ao") => DebugView::AmbientOcclusion,
        Some("shadow") => DebugView::Shadow,
        Some("history") => DebugView::HistoryRejection,
        Some("bvh") => DebugView::Bvh,
        _ => DebugView::Final,
    }
}

#[cfg(target_arch = "wasm32")]
pub fn scene_config() -> crate::contract::SceneConfig {
    let mut config = crate::contract::SceneConfig::default();
    if let Some(window) = web_sys::window() {
        if let Ok(query) = window.location().search() {
            if let Ok(params) = web_sys::UrlSearchParams::new_with_str(&query) {
                if let Some(seed) = params.get("seed") {
                    let parsed = seed
                        .strip_prefix("0x")
                        .map(|v| u64::from_str_radix(v, 16))
                        .unwrap_or_else(|| seed.parse());
                    if let Ok(seed) = parsed {
                        config.seed = seed;
                    }
                }
            }
        }
    }
    config
}
