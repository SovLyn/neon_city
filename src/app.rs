use crate::{
    accel::BvhBuild,
    animation::AnimationState,
    config,
    contract::{
        BuildPoll, DynamicFrame, FrameInput, HistoryResetReason, QualityTier, RenderConfig,
    },
    platform::gpu::GpuContext,
    render::Renderer,
    scene::SceneBuilder,
};
use glam::{Mat4, Vec3};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, HtmlCanvasElement, KeyboardEvent, MouseEvent, Window};

fn window() -> Result<Window, String> {
    web_sys::window().ok_or("window unavailable".into())
}
fn document() -> Result<Document, String> {
    window()?.document().ok_or("document unavailable".into())
}
thread_local! {
    static FATAL_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
    static ERROR_HISTORY: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
}
fn fatal_error() -> Option<String> {
    FATAL_ERROR.with(|fatal| fatal.borrow().clone())
}
fn status(text: &str) {
    if fatal_error().is_some() {
        return;
    }
    if let Ok(doc) = document() {
        if let Some(node) = doc.get_element_by_id("status") {
            node.set_text_content(Some(text));
        }
    }
}
fn metrics() -> Option<js_sys::Object> {
    let window = web_sys::window()?;
    let key = JsValue::from_str("__neonMetrics");
    let existing = js_sys::Reflect::get(window.as_ref(), &key).ok()?;
    if existing.is_object() {
        return existing.dyn_into().ok();
    }
    let object = js_sys::Object::new();
    js_sys::Reflect::set(window.as_ref(), &key, &object).ok()?;
    Some(object)
}
fn metric(name: &str, value: &JsValue) {
    if let Some(object) = metrics() {
        let _ = js_sys::Reflect::set(&object, &name.into(), value);
    }
}
fn metric_size(name: &str, size: [u32; 2]) {
    let array = js_sys::Array::new();
    array.push(&size[0].into());
    array.push(&size[1].into());
    metric(name, array.as_ref());
}
pub fn show_error(error: &str) {
    let first = FATAL_ERROR.with(|fatal| {
        let mut fatal = fatal.borrow_mut();
        if fatal.is_none() {
            *fatal = Some(error.to_owned());
            true
        } else {
            false
        }
    });
    let unique = ERROR_HISTORY.with(|history| {
        let mut history = history.borrow_mut();
        if history.contains(&error.to_owned()) {
            return false;
        }
        if history.len() == 20 {
            history.pop_front();
        }
        history.push_back(error.to_owned());
        true
    });
    if first {
        if let Ok(doc) = document() {
            if let Some(node) = doc.get_element_by_id("error") {
                node.set_text_content(Some(&format!("致命错误: {error}")));
            }
            if let Some(node) = doc.get_element_by_id("status") {
                node.set_text_content(Some("已停止：发生致命错误"));
            }
        }
        metric("fatal_error", &error.into());
    }
    if let Some(object) = metrics() {
        if unique {
            let errors = js_sys::Reflect::get(&object, &"errors".into())
                .ok()
                .filter(js_sys::Array::is_array)
                .map(|v| js_sys::Array::from(&v))
                .unwrap_or_default();
            errors.push(&error.into());
            while errors.length() > 20 {
                errors.shift();
            }
            let _ = js_sys::Reflect::set(&object, &"errors".into(), errors.as_ref());
        }
        if first {
            let _ = js_sys::Reflect::set(&object, &"last_error".into(), &error.into());
        }
    }
    if unique {
        web_sys::console::error_1(&error.into());
    }
}
fn now_ms() -> Result<f64, String> {
    Ok(window()?
        .performance()
        .ok_or("performance unavailable")?
        .now())
}

async fn yield_to_browser() -> Result<(), String> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| match web_sys::window() {
        Some(w) => {
            if let Err(error) = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0)
            {
                let _ = reject.call1(&JsValue::UNDEFINED, &error);
            }
        }
        None => {
            let _ = reject.call1(
                &JsValue::UNDEFINED,
                &JsValue::from_str("window unavailable"),
            );
        }
    });
    JsFuture::from(promise)
        .await
        .map_err(|e| format!("timer: {e:?}"))?;
    Ok(())
}

async fn fetch_bytes(path: &str) -> Result<Vec<u8>, String> {
    let base = document()?
        .base_uri()
        .map_err(|e| format!("base URI: {e:?}"))?
        .ok_or("document has no base URI")?;
    let url = web_sys::Url::new_with_base(path, &base).map_err(|e| format!("asset URL: {e:?}"))?;
    let response: web_sys::Response = JsFuture::from(window()?.fetch_with_str(&url.href()))
        .await
        .map_err(|e| format!("fetch {path}: {e:?}"))?
        .dyn_into()
        .map_err(|_| "invalid fetch response")?;
    if !response.ok() {
        return Err(format!("fetch {path}: HTTP {}", response.status()));
    }
    let buffer = JsFuture::from(
        response
            .array_buffer()
            .map_err(|e| format!("array buffer: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("asset body: {e:?}"))?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

// The atlas metadata stores top-left-origin, normalized, texel-center-inset UVs.
// Region IDs must match the material atlas_region indices produced by the scene.
async fn load_atlas() -> Result<(Vec<u8>, u32, u32, Vec<[f32; 4]>), String> {
    let png = fetch_bytes("assets/fonts/signs-rgba.png").await?;
    let metadata = fetch_bytes("assets/fonts/signs-rgba.json").await?;
    let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        .map_err(|e| format!("atlas PNG: {e}"))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let json: serde_json::Value =
        serde_json::from_slice(&metadata).map_err(|e| format!("atlas JSON: {e}"))?;
    if json.get("width").and_then(|v| v.as_u64()) != Some(width as u64)
        || json.get("height").and_then(|v| v.as_u64()) != Some(height as u64)
    {
        return Err("atlas dimensions do not match metadata".into());
    }
    let entries = json
        .get("regions")
        .and_then(|v| v.as_array())
        .ok_or("atlas JSON missing regions array")?;
    let mut rects = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        if entry.get("region_id").and_then(|v| v.as_u64()) != Some(index as u64) {
            return Err("atlas region IDs must be contiguous and ordered".into());
        }
        let v = entry
            .get("uv_rect")
            .and_then(|v| v.as_array())
            .filter(|v| v.len() == 4)
            .ok_or("invalid atlas rect")?;
        let mut rect = [0.0; 4];
        for (dst, src) in rect.iter_mut().zip(v) {
            *dst = src
                .as_f64()
                .filter(|n| n.is_finite() && *n >= 0.0 && *n <= 1.0)
                .ok_or("invalid normalized atlas coordinate")? as f32;
        }
        if rect[0] > rect[2] || rect[1] > rect[3] {
            return Err("inverted atlas rect".into());
        }
        rects.push(rect);
    }
    Ok((image.into_raw(), width, height, rects))
}

#[derive(Default)]
struct Controls {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    dragging: bool,
    last_mouse: (i32, i32),
    yaw_delta: f32,
    pitch_delta: f32,
    paused: bool,
    quality_toggle: bool,
    visibility_changed: bool,
}
struct Runtime {
    renderer: Renderer,
    animation: AnimationState,
    dynamic: DynamicFrame,
    canvas: HtmlCanvasElement,
    controls: Rc<RefCell<Controls>>,
    quality: QualityTier,
    position: Vec3,
    yaw: f32,
    pitch: f32,
    prev_view_proj: Mat4,
    has_previous: bool,
    time: f32,
    accumulator: f32,
    last_ms: Option<f64>,
    frame: u32,
    was_paused: bool,
    size: [u32; 2],
    internal_size: [u32; 2],
}

fn on<E: JsCast + wasm_bindgen::convert::FromWasmAbi + 'static>(
    target: &web_sys::EventTarget,
    event: &str,
    callback: impl FnMut(E) + 'static,
) -> Result<(), String> {
    let closure = Closure::<dyn FnMut(E)>::new(callback);
    target
        .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
        .map_err(|e| format!("event {event}: {e:?}"))?;
    closure.forget();
    Ok(())
}
fn install_controls(
    canvas: &HtmlCanvasElement,
    controls: Rc<RefCell<Controls>>,
) -> Result<(), String> {
    let w = window()?;
    for down in [true, false] {
        let c = controls.clone();
        on::<KeyboardEvent>(&w, if down { "keydown" } else { "keyup" }, move |e| {
            let mut c = c.borrow_mut();
            match e.code().as_str() {
                "KeyW" => c.forward = down,
                "KeyS" => c.back = down,
                "KeyA" => c.left = down,
                "KeyD" => c.right = down,
                "KeyQ" if down && !e.repeat() => c.quality_toggle = true,
                "Space" if down && !e.repeat() => {
                    c.paused = !c.paused;
                    e.prevent_default();
                }
                _ => {}
            }
        })?;
    }
    let c = controls.clone();
    let doc = document()?;
    on::<web_sys::Event>(&doc, "visibilitychange", move |_| {
        c.borrow_mut().visibility_changed = true
    })?;
    let c = controls.clone();
    on::<web_sys::Event>(&w, "blur", move |_| {
        let mut c = c.borrow_mut();
        c.forward = false;
        c.back = false;
        c.left = false;
        c.right = false;
        c.dragging = false;
    })?;
    let c = controls.clone();
    on::<MouseEvent>(canvas, "mousedown", move |e| {
        let mut c = c.borrow_mut();
        c.dragging = true;
        c.last_mouse = (e.client_x(), e.client_y());
    })?;
    let c = controls.clone();
    on::<MouseEvent>(&w, "mouseup", move |_| c.borrow_mut().dragging = false)?;
    let c = controls.clone();
    on::<MouseEvent>(&w, "mousemove", move |e| {
        let mut c = c.borrow_mut();
        if c.dragging {
            c.yaw_delta += (e.client_x() - c.last_mouse.0) as f32 * 0.003;
            c.pitch_delta += (e.client_y() - c.last_mouse.1) as f32 * 0.003;
            c.last_mouse = (e.client_x(), e.client_y());
        }
    })?;
    for (id, is_quality) in [("pause", false), ("quality", true)] {
        let target = document()?
            .get_element_by_id(id)
            .ok_or(format!("missing {id}"))?;
        let c = controls.clone();
        on::<web_sys::Event>(&target, "click", move |_| {
            let mut c = c.borrow_mut();
            if is_quality {
                c.quality_toggle = true;
            } else {
                c.paused = !c.paused;
            }
        })?;
    }
    Ok(())
}

impl Runtime {
    fn tick(&mut self, now: f64) -> Result<(), String> {
        if fatal_error().is_some() {
            return Ok(());
        }
        let visible = document()?.visibility_state() == web_sys::VisibilityState::Visible;
        let mut controls = self.controls.borrow_mut();
        if controls.visibility_changed {
            self.was_paused = true;
            controls.visibility_changed = false;
        }
        let paused = controls.paused || !visible;
        let observed_frame_ms = self.last_ms.map(|last| (now - last).max(0.0));
        let dt = if self.was_paused || paused {
            0.0
        } else {
            self.last_ms
                .map_or(0.0, |last| ((now - last) / 1000.0).clamp(0.0, 0.1) as f32)
        };
        self.last_ms = Some(now);
        if !visible {
            self.was_paused = true;
            return Ok(());
        }
        if self.was_paused && !paused {
            self.renderer.invalidate_history(HistoryResetReason::Resume);
            self.accumulator = 0.0;
            self.has_previous = false;
        }
        self.was_paused = paused;
        if controls.quality_toggle {
            self.quality = match self.quality {
                QualityTier::Balanced => QualityTier::Performance,
                QualityTier::Performance => QualityTier::Balanced,
            };
            self.renderer.set_quality(self.quality);
            self.renderer
                .invalidate_history(HistoryResetReason::QualityChange);
            controls.quality_toggle = false;
            if let Some(button) = document()?.get_element_by_id("quality") {
                button.set_text_content(Some(&format!("画质: {:?}", self.quality)));
            }
        }
        let dpr = window()?.device_pixel_ratio();
        let size = [
            ((self.canvas.client_width() as f64 * dpr).round() as u32).max(1),
            ((self.canvas.client_height() as f64 * dpr).round() as u32).max(1),
        ];
        if size != self.size {
            self.canvas.set_width(size[0]);
            self.canvas.set_height(size[1]);
            self.renderer.resize(size);
            self.renderer.invalidate_history(HistoryResetReason::Resize);
            self.has_previous = false;
            self.size = size;
        }
        self.yaw -= controls.yaw_delta;
        self.pitch = (self.pitch - controls.pitch_delta).clamp(-1.4, 1.4);
        controls.yaw_delta = 0.0;
        controls.pitch_delta = 0.0;
        let forward = Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos());
        let right = Vec3::new(forward.z * -1.0, 0.0, forward.x);
        let movement = forward * (controls.forward as i32 - controls.back as i32) as f32
            + right * (controls.right as i32 - controls.left as i32) as f32;
        self.position += movement.normalize_or_zero() * dt * 8.0;
        drop(controls);
        // AnimationState owns the fixed-step accumulator and render interpolation;
        // call once per rendered frame to retain the last submitted transforms.
        self.animation.advance(dt, &mut self.dynamic);
        self.time += dt;
        self.renderer
            .set_material_states(&self.animation.material_states());
        let direction = Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        );
        let view = Mat4::look_to_rh(self.position, direction, Vec3::Y);
        let projection = Mat4::perspective_rh(
            60.0_f32.to_radians(),
            size[0] as f32 / size[1] as f32,
            0.1,
            250.0,
        );
        let vp = projection * view;
        let frame = FrameInput {
            frame_index: self.frame,
            time_s: self.time,
            dt_s: dt,
            view_proj: vp,
            prev_view_proj: if self.has_previous {
                self.prev_view_proj
            } else {
                vp
            },
            inv_view_proj: vp.inverse(),
            camera_world: self.position,
            camera_cut: !self.has_previous,
        };
        let render_start_ms = now_ms()?;
        let stats = self
            .renderer
            .render(&frame, &self.dynamic)
            .map_err(|e| format!("render: {e}"))?;
        let cpu_submit_ms = now_ms()? - render_start_ms;
        self.prev_view_proj = vp;
        self.has_previous = true;
        self.frame = self.frame.wrapping_add(1);
        metric("frame", &self.frame.into());
        metric("simulation_time_s", &self.time.into());
        metric_size("output", size);
        metric_size("internal", self.internal_size);
        metric("quality", &format!("{:?}", self.quality).into());
        metric("cpu_ms", &JsValue::from_f64(cpu_submit_ms));
        metric(
            "observed_frame_ms",
            &observed_frame_ms
                .map(JsValue::from_f64)
                .unwrap_or(JsValue::NULL),
        );
        if self.frame % 30 == 0 {
            if let Some(node) = document()?.get_element_by_id("stats") {
                node.set_text_content(Some(&format!(
                    "{:?} | 输出 {}×{} | 内部 1280×720 | CPU {:.1}ms | GPU {} | BVH overflow 未测",
                    self.quality,
                    size[0],
                    size[1],
                    cpu_submit_ms,
                    stats
                        .gpu_ms
                        .map_or("不可用".into(), |ms| format!("{ms:.1}ms")),
                )));
            }
        }
        Ok(())
    }
}

fn install_global_error_handlers() -> Result<(), String> {
    let window = window()?;
    for event_name in ["error", "unhandledrejection"] {
        let name = event_name;
        let callback = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            let value = js_sys::Reflect::get(event.as_ref(), &"reason".into())
                .ok()
                .filter(|value| !value.is_undefined())
                .or_else(|| {
                    let error = js_sys::Reflect::get(event.as_ref(), &"error".into()).ok()?;
                    if error.is_undefined() {
                        None
                    } else {
                        Some(error)
                    }
                })
                .or_else(|| js_sys::Reflect::get(event.as_ref(), &"message".into()).ok());
            let detail = value.map_or_else(
                || "(no error detail)".to_owned(),
                |value| {
                    value
                        .as_string()
                        .or_else(|| {
                            js_sys::Reflect::get(&value, &"stack".into())
                                .ok()
                                .and_then(|v| v.as_string())
                        })
                        .or_else(|| {
                            js_sys::Reflect::get(&value, &"message".into())
                                .ok()
                                .and_then(|v| v.as_string())
                        })
                        .or_else(|| {
                            js_sys::JSON::stringify(&value)
                                .ok()
                                .and_then(|v| v.as_string())
                        })
                        .unwrap_or_else(|| format!("{value:?}"))
                },
            );
            show_error(&format!("Global {name} event: {detail}"));
        });
        window
            .add_event_listener_with_callback(event_name, callback.as_ref().unchecked_ref())
            .map_err(|error| format!("install {event_name} handler: {error:?}"))?;
        callback.forget();
    }
    Ok(())
}

pub async fn start() -> Result<(), String> {
    install_global_error_handlers()?;
    metric("frame", &0u32.into());
    metric("last_error", &JsValue::NULL);
    metric("errors", js_sys::Array::new().as_ref());
    metric("observed_frame_ms", &JsValue::NULL);
    metric("cpu_ms", &JsValue::NULL);
    metric("gpu_ms", &JsValue::NULL);
    metric("bvh_stack_overflows", &JsValue::NULL);
    metric("fatal_error", &JsValue::NULL);
    let canvas: HtmlCanvasElement = document()?
        .get_element_by_id("canvas")
        .ok_or("canvas missing")?
        .dyn_into()
        .map_err(|_| "invalid canvas")?;
    let gpu = GpuContext::create(canvas.clone())
        .await
        .map_err(|e| e.to_string())?;
    let config = config::scene_config();
    let seed = config.seed;
    let mut scene_builder = SceneBuilder::new(config);
    let mut slice_start = now_ms()?;
    let scene = loop {
        match scene_builder.step(1).map_err(|e| format!("scene: {e}"))? {
            BuildPoll::Ready(scene) => break scene,
            BuildPoll::Pending { progress } => {
                if now_ms()? - slice_start >= 8.0 {
                    status(&format!("生成场景 {:.0}%", progress * 100.0));
                    yield_to_browser().await?;
                    slice_start = now_ms()?;
                }
            }
        }
    };
    let mut builder = BvhBuild::new(&scene).map_err(|e| format!("BVH: {e}"))?;
    slice_start = now_ms()?;
    let packed = loop {
        match builder.step(1).map_err(|e| format!("BVH: {e}"))? {
            BuildPoll::Ready(packed) => break packed,
            BuildPoll::Pending { progress } => {
                if now_ms()? - slice_start >= 8.0 {
                    status(&format!("构建 BVH {:.0}%", progress * 100.0));
                    yield_to_browser().await?;
                    slice_start = now_ms()?;
                }
            }
        }
    };
    let mut render_config = RenderConfig::default();
    render_config.output_size = [canvas.width(), canvas.height()];
    render_config.debug_view = config::debug_view();
    let internal_size = render_config.internal_size;
    let (position, yaw, pitch) = match config::parameter("camera").as_deref() {
        Some("showcase") => (Vec3::new(-3.0, 4.5, -48.0), -1.48, 0.12),
        Some("side") => (Vec3::new(3.0, 4.5, -43.0), 1.48, 0.12),
        Some("reflection") => (Vec3::new(0.0, 2.0, 35.0), std::f32::consts::FRAC_PI_2, -0.6),
        _ => (Vec3::new(0.0, 2.0, 50.0), 0.0, 0.0),
    };
    let mut renderer =
        Renderer::new(gpu, render_config, &scene, &packed).map_err(|e| format!("renderer: {e}"))?;
    renderer.set_effects(
        config::parameter("bloom").as_deref() != Some("0"),
        config::parameter("atmosphere").as_deref() != Some("0"),
    );
    match load_atlas().await {
        Ok((rgba, width, height, rects)) => {
            renderer
                .upload_sign_atlas(&rgba, width, height, &rects)
                .map_err(|e| format!("atlas upload: {e}"))?;
            status("运行中");
        }
        Err(error) => return Err(format!("atlas unavailable: {error}")),
    }
    let controls = Rc::new(RefCell::new(Controls {
        paused: config::parameter("paused").as_deref() == Some("1"),
        ..Default::default()
    }));
    install_controls(&canvas, controls.clone())?;
    let runtime = Rc::new(RefCell::new(Runtime {
        renderer,
        animation: AnimationState::new(&scene, seed),
        dynamic: DynamicFrame::default(),
        size: [canvas.width(), canvas.height()],
        internal_size,
        canvas,
        controls,
        quality: QualityTier::Balanced,
        position,
        yaw,
        pitch,
        prev_view_proj: Mat4::IDENTITY,
        has_previous: false,
        time: config::parameter("time")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .unwrap_or(0.0),
        accumulator: 0.0,
        last_ms: None,
        frame: 0,
        was_paused: false,
    }));
    let callback: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let next = callback.clone();
    *callback.borrow_mut() = Some(Closure::new(move |now| {
        if fatal_error().is_some() {
            return;
        }
        if let Err(error) = runtime.borrow_mut().tick(now) {
            show_error(&error);
            return;
        }
        if let (Ok(window), Some(callback)) = (window(), next.borrow().as_ref()) {
            if fatal_error().is_none() {
                if let Err(error) =
                    window.request_animation_frame(callback.as_ref().unchecked_ref())
                {
                    show_error(&format!("requestAnimationFrame: {error:?}"));
                }
            }
        }
    }));
    window()?
        .request_animation_frame(callback.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .map_err(|e| format!("requestAnimationFrame: {e:?}"))?;
    Ok(())
}
