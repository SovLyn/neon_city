pub mod accel;
pub mod animation;
pub mod config;
pub mod contract;
pub mod platform;
pub mod render;
pub mod scene;

#[cfg(target_arch = "wasm32")]
mod app;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        if let Err(error) = app::start().await {
            app::show_error(&error);
        }
    });
}
