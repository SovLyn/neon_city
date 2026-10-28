use crate::contract::InitError;

pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub surface: wgpu::Surface<'static>,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
}

impl GpuContext {
    pub async fn create(canvas: web_sys::HtmlCanvasElement) -> Result<Self, InitError> {
        let size = (canvas.width().max(1), canvas.height().max(1));
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| InitError::Surface(e.to_string()))?;
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), power_preference: wgpu::PowerPreference::HighPerformance, force_fallback_adapter: false }).await.ok_or_else(|| InitError::Capability("WebGPU adapter unavailable; use a WebGPU-enabled browser over HTTPS or localhost".into()))?;
        let limits = adapter.limits();
        if limits.max_storage_buffers_per_shader_stage < 4
            || limits.max_storage_textures_per_shader_stage < 2
            || limits.max_color_attachments < 4
            || limits.max_color_attachment_bytes_per_sample < 28
        {
            return Err(InitError::Capability(format!(
                "insufficient WebGPU limits: storage buffers {}, storage textures {}, color attachments {}, bytes/sample {} (required 4/2/4/28)",
                limits.max_storage_buffers_per_shader_stage,
                limits.max_storage_textures_per_shader_stage,
                limits.max_color_attachments,
                limits.max_color_attachment_bytes_per_sample
            )));
        }
        for (format, usages) in [
            (
                wgpu::TextureFormat::Rgba16Float,
                wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
            (
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
            (
                wgpu::TextureFormat::Depth32Float,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
        ] {
            if !adapter
                .get_texture_format_features(format)
                .allowed_usages
                .contains(usages)
            {
                return Err(InitError::Capability(format!(
                    "texture format {format:?} lacks required usages {usages:?}"
                )));
            }
        }
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("neon city device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .map_err(|e| InitError::Capability(format!("device request failed: {e}")))?;
        device.on_uncaptured_error(Box::new(|error| {
            crate::app::show_error(&format!("WebGPU uncaptured error: {error}"));
        }));
        device.set_device_lost_callback(|reason, message| {
            crate::app::show_error(&format!("WebGPU device lost ({reason:?}): {message}"));
        });
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| InitError::Surface("surface has no supported format".into()))?;
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.0,
            height: size.1,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);
        Ok(Self {
            instance,
            surface,
            adapter,
            device,
            queue,
            surface_config,
        })
    }

    pub fn clear(&self) -> Result<(), InitError> {
        let frame = self
            .surface
            .get_current_texture()
            .map_err(|e| InitError::Surface(e.to_string()))?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("startup clear"),
            });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("startup clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.025,
                            g: 0.008,
                            b: 0.05,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}
