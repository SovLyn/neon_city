#[cfg(target_arch = "wasm32")]
mod post;
#[cfg(target_arch = "wasm32")]
mod temporal;
#[cfg(target_arch = "wasm32")]
mod web {
    use super::{post::Post, temporal::Temporal};
    use crate::{contract::*, platform::gpu::GpuContext};
    use wgpu::util::DeviceExt;

    struct Target {
        _texture: wgpu::Texture,
        view: wgpu::TextureView,
    }
    fn target(
        device: &wgpu::Device,
        size: [u32; 2],
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> Target {
        let t = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size[0].max(1),
                height: size[1].max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });
        let view = t.create_view(&Default::default());
        Target { _texture: t, view }
    }
    fn buffer(device: &wgpu::Device, bytes: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
        let zero = [0u8; 256];
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: if bytes.is_empty() { &zero } else { bytes },
            usage,
        })
    }
    fn shader(device: &wgpu::Device, s: &str) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(s.into()),
        })
    }
    struct Mesh {
        vertices: wgpu::Buffer,
        indices: wgpu::Buffer,
        count: u32,
    }
    struct Images {
        g: [Target; 4],
        depth: Target,
        reflection: Target,
        ao: Target,
    }
    fn ray_size(size: [u32; 2], quality: QualityTier) -> [u32; 2] {
        let divisor = if quality == QualityTier::Balanced {
            2
        } else {
            4
        };
        [(size[0] / divisor).max(1), (size[1] / divisor).max(1)]
    }
    fn images(device: &wgpu::Device, size: [u32; 2], quality: QualityTier) -> Images {
        let r = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let s = wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING;
        let divisor = if quality == QualityTier::Balanced {
            2
        } else {
            4
        };
        let rs = [(size[0] / divisor).max(1), (size[1] / divisor).max(1)];
        Images {
            g: [
                target(device, size, wgpu::TextureFormat::Rgba8Unorm, r),
                target(device, size, wgpu::TextureFormat::Rgba16Float, r),
                target(device, size, wgpu::TextureFormat::Rgba16Float, r),
                target(device, size, wgpu::TextureFormat::Rgba16Float, r),
            ],
            depth: target(device, size, wgpu::TextureFormat::Depth32Float, r),
            reflection: target(device, rs, wgpu::TextureFormat::Rgba16Float, s),
            ao: target(device, rs, wgpu::TextureFormat::Rgba16Float, s),
        }
    }
    fn entry<'a>(binding: u32, resource: wgpu::BindingResource<'a>) -> wgpu::BindGroupEntry<'a> {
        wgpu::BindGroupEntry { binding, resource }
    }
    fn be<'a>(binding: u32, b: &'a wgpu::Buffer) -> wgpu::BindGroupEntry<'a> {
        entry(binding, b.as_entire_binding())
    }
    fn te<'a>(binding: u32, t: &'a Target) -> wgpu::BindGroupEntry<'a> {
        entry(binding, wgpu::BindingResource::TextureView(&t.view))
    }

    pub struct Renderer {
        gpu: GpuContext,
        config: RenderConfig,
        images: Images,
        meshes: Vec<Mesh>,
        materials: wgpu::Buffer,
        static_data: wgpu::Buffer,
        camera: wgpu::Buffer,
        ray_frame: wgpu::Buffer,
        ripples: wgpu::Buffer,
        atlas: Target,
        sampler: wgpu::Sampler,
        rects: wgpu::Buffer,
        states: wgpu::Buffer,
        raster: wgpu::RenderPipeline,
        ray: wgpu::ComputePipeline,
        temporal: Temporal,
        post: Post,
    }
    impl Renderer {
        pub fn new(
            mut gpu: GpuContext,
            config: RenderConfig,
            scene: &SceneCpu,
            accel: &PackedScene,
        ) -> Result<Self, RenderError> {
            if scene.materials.is_empty() || scene.materials.len() >= 2048 || accel.words.len() < 16
            {
                return Err(RenderError::DataOutOfBounds(
                    "missing materials or packed scene header".into(),
                ));
            }
            if scene
                .meshes
                .iter()
                .any(|m| m.indices.iter().any(|i| *i as usize >= m.vertices.len()))
            {
                return Err(RenderError::DataOutOfBounds(
                    "mesh index out of bounds".into(),
                ));
            }
            let d = &gpu.device;
            let meshes = scene
                .meshes
                .iter()
                .map(|m| Mesh {
                    vertices: buffer(
                        d,
                        bytemuck::cast_slice(&m.vertices),
                        wgpu::BufferUsages::VERTEX,
                    ),
                    indices: buffer(
                        d,
                        bytemuck::cast_slice(&m.indices),
                        wgpu::BufferUsages::INDEX,
                    ),
                    count: m.indices.len() as u32,
                })
                .collect();
            let materials = buffer(
                d,
                bytemuck::cast_slice(&scene.materials),
                wgpu::BufferUsages::STORAGE,
            );
            let static_data = buffer(
                d,
                bytemuck::cast_slice(&accel.words),
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            );
            let camera = buffer(
                d,
                &[0; 144],
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            let ray_frame = buffer(
                d,
                &[0; 112],
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            let ripples = buffer(
                d,
                &[0; 4112],
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            let atlas = target(
                d,
                [1, 1],
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            );
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &atlas._texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &[255, 255, 255, 255],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            let sampler = d.create_sampler(&wgpu::SamplerDescriptor::default());
            let rects = buffer(
                d,
                bytemuck::cast_slice(&[[0.0f32, 0.0, 1.0, 1.0]]),
                wgpu::BufferUsages::STORAGE,
            );
            let state = MaterialStateGpu {
                emission_multiplier: [1.0; 3],
                ..Default::default()
            };
            let states = buffer(
                d,
                bytemuck::cast_slice(&vec![state; scene.materials.len()]),
                wgpu::BufferUsages::STORAGE,
            );
            let common = include_str!("../shaders/common.wgsl");
            let effects = include_str!("../shaders/material_effects.wgsl");
            let gs = shader(
                d,
                &format!(
                    "{common}\n{effects}\n{}",
                    include_str!("../shaders/gbuffer.wgsl")
                ),
            );
            let rs = shader(
                d,
                &format!(
                    "{common}\n{effects}\n{}",
                    include_str!("../shaders/raytrace.wgsl")
                ),
            );
            let attrs = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2];
            // VertexGpu has explicit padding between position, normal and UV.
            let attrs = [
                wgpu::VertexAttribute {
                    offset: 0,
                    ..attrs[0]
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    ..attrs[1]
                },
                wgpu::VertexAttribute {
                    offset: 32,
                    ..attrs[2]
                },
            ];
            let raster = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("gbuffer"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &gs,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 48,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attrs,
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &gs,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                        Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                        Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                        Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba16Float,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                    ],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            });
            let ray = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("software rays"),
                layout: None,
                module: &rs,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
            let temporal = Temporal::new(d, ray_size(config.internal_size, config.quality));
            let mut post = Post::new(d, gpu.surface_config.format);
            post.set_materials(d, &scene.materials);
            let images = images(d, config.internal_size, config.quality);
            gpu.surface_config.width = config.output_size[0].max(1);
            gpu.surface_config.height = config.output_size[1].max(1);
            gpu.surface.configure(&gpu.device, &gpu.surface_config);
            Ok(Self {
                gpu,
                config,
                images,
                meshes,
                materials,
                static_data,
                camera,
                ray_frame,
                ripples,
                atlas,
                sampler,
                rects,
                states,
                raster,
                ray,
                temporal,
                post,
            })
        }
        pub fn resize(&mut self, output_size: [u32; 2]) {
            self.config.output_size = output_size;
            self.temporal.reset();
            self.gpu.surface_config.width = output_size[0].max(1);
            self.gpu.surface_config.height = output_size[1].max(1);
            self.gpu
                .surface
                .configure(&self.gpu.device, &self.gpu.surface_config);
        }
        pub fn set_quality(&mut self, quality: QualityTier) {
            self.config.quality = quality;
            self.images = images(&self.gpu.device, self.config.internal_size, quality);
            self.temporal = Temporal::new(
                &self.gpu.device,
                ray_size(self.config.internal_size, quality),
            );
        }
        pub fn invalidate_history(&mut self, _reason: HistoryResetReason) {
            self.temporal.reset();
        }
        pub fn set_effects(&mut self, bloom: bool, atmosphere: bool) {
            self.post.set_effects(bloom, atmosphere);
        }
        pub fn set_material_states(&mut self, states: &[MaterialStateGpu]) {
            if !states.is_empty() {
                self.states = buffer(
                    &self.gpu.device,
                    bytemuck::cast_slice(states),
                    wgpu::BufferUsages::STORAGE,
                );
            }
        }
        pub fn upload_sign_atlas(
            &mut self,
            rgba: &[u8],
            width: u32,
            height: u32,
            rects: &[[f32; 4]],
        ) -> Result<(), RenderError> {
            if width == 0
                || height == 0
                || rgba.len() != width as usize * height as usize * 4
                || rects.is_empty()
            {
                return Err(RenderError::Asset("invalid atlas".into()));
            }
            self.atlas = target(
                &self.gpu.device,
                [width, height],
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            self.gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas._texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            self.rects = buffer(
                &self.gpu.device,
                bytemuck::cast_slice(rects),
                wgpu::BufferUsages::STORAGE,
            );
            Ok(())
        }
        pub fn render(
            &mut self,
            frame: &FrameInput,
            dynamic: &DynamicFrame,
        ) -> Result<FrameStats, RenderError> {
            let clock = web_sys::window()
                .and_then(|window| window.performance())
                .ok_or_else(|| {
                    RenderError::Other("browser performance clock unavailable".into())
                })?;
            let start_ms = clock.now();
            let d = &self.gpu.device;
            let q = &self.gpu.queue;
            if dynamic
                .instances
                .iter()
                .any(|i| i.mesh_id as usize >= self.meshes.len())
            {
                return Err(RenderError::DataOutOfBounds("instance mesh".into()));
            }
            let mut camera = Vec::new();
            camera.extend_from_slice(bytemuck::bytes_of(&frame.view_proj.to_cols_array()));
            camera.extend_from_slice(bytemuck::bytes_of(&frame.prev_view_proj.to_cols_array()));
            camera.extend_from_slice(bytemuck::bytes_of(&[
                frame.camera_world.x,
                frame.camera_world.y,
                frame.camera_world.z,
                frame.time_s,
            ]));
            q.write_buffer(&self.camera, 0, &camera);
            let count = dynamic.ripples.len().min(128);
            let mut ripple_bytes = vec![0u8; 4112];
            ripple_bytes[..4].copy_from_slice(&(count as u32).to_ne_bytes());
            ripple_bytes[16..16 + count * 32]
                .copy_from_slice(bytemuck::cast_slice(&dynamic.ripples[..count]));
            q.write_buffer(&self.ripples, 0, &ripple_bytes);
            let mut ray = vec![0u8; 112];
            ray[..64].copy_from_slice(bytemuck::bytes_of(&frame.inv_view_proj.to_cols_array()));
            ray[64..80].copy_from_slice(bytemuck::bytes_of(&[
                frame.camera_world.x,
                frame.camera_world.y,
                frame.camera_world.z,
                0.0,
            ]));
            ray[80..84].copy_from_slice(&frame.frame_index.to_ne_bytes());
            q.write_buffer(&self.ray_frame, 0, &ray);
            let instances = buffer(
                d,
                bytemuck::cast_slice(&dynamic.instances),
                wgpu::BufferUsages::STORAGE,
            );
            let mut words = vec![0u32; 16];
            words[0] = crate::contract::bindings::DYNAMIC_MAGIC;
            words[1] = 1;
            words[4] = 16;
            words[5] = dynamic.ray_objects.len() as u32;
            words.extend_from_slice(bytemuck::cast_slice(&dynamic.ray_objects));
            words[6] = words.len() as u32;
            words[7] = dynamic.lights.len().min(64) as u32;
            words.extend_from_slice(bytemuck::cast_slice(
                &dynamic.lights[..dynamic.lights.len().min(64)],
            ));
            words[2] = words.len() as u32;
            let dynbuf = buffer(d, bytemuck::cast_slice(&words), wgpu::BufferUsages::STORAGE);
            let gb = d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.raster.get_bind_group_layout(0),
                entries: &[
                    be(0, &self.camera),
                    be(1, &instances),
                    be(2, &self.materials),
                    be(3, &self.states),
                    entry(4, wgpu::BindingResource::TextureView(&self.atlas.view)),
                    entry(5, wgpu::BindingResource::Sampler(&self.sampler)),
                    be(6, &self.rects),
                    be(7, &self.ripples),
                ],
            });
            let rb = d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.ray.get_bind_group_layout(0),
                entries: &[
                    be(0, &self.ray_frame),
                    be(1, &self.static_data),
                    be(2, &dynbuf),
                    be(3, &self.rects),
                    be(4, &self.states),
                ],
            });
            let rt = d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.ray.get_bind_group_layout(1),
                entries: &[
                    te(0, &self.images.g[1]),
                    te(1, &self.images.depth),
                    entry(2, wgpu::BindingResource::TextureView(&self.atlas.view)),
                    entry(3, wgpu::BindingResource::Sampler(&self.sampler)),
                ],
            });
            let ro = d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.ray.get_bind_group_layout(2),
                entries: &[te(0, &self.images.reflection), te(1, &self.images.ao)],
            });
            let surface = self
                .gpu
                .surface
                .get_current_texture()
                .map_err(|e| RenderError::Surface(e.to_string()))?;
            let view = surface.texture.create_view(&Default::default());
            let mut encoder = d.create_command_encoder(&Default::default());
            {
                let attachments = self.images.g.each_ref().map(|t| {
                    Some(wgpu::RenderPassColorAttachment {
                        view: &t.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })
                });
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("gbuffer"),
                    color_attachments: &attachments,
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.images.depth.view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&self.raster);
                pass.set_bind_group(0, &gb, &[]);
                for (i, instance) in dynamic.instances.iter().enumerate() {
                    let mesh = &self.meshes[instance.mesh_id as usize];
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.count, 0, i as u32..i as u32 + 1);
                }
            }
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("rays"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.ray);
                pass.set_bind_group(0, &rb, &[]);
                pass.set_bind_group(1, &rt, &[]);
                pass.set_bind_group(2, &ro, &[]);
                let divisor = if self.config.quality == QualityTier::Balanced {
                    2
                } else {
                    4
                };
                let w = (self.config.internal_size[0] / divisor).max(1);
                let h = (self.config.internal_size[1] / divisor).max(1);
                pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
            }
            self.temporal.encode(
                d,
                q,
                &mut encoder,
                &self.images.reflection.view,
                &self.images.ao.view,
                &self.images.g[3].view,
                &self.images.g[1].view,
                &self.images.depth.view,
                frame,
            );
            self.post.encode(
                d,
                q,
                &mut encoder,
                &view,
                self.images.g.each_ref().map(|g| &g.view),
                &self.images.depth.view,
                self.temporal.reflection_view(),
                self.temporal.ao_view(),
                frame,
                dynamic,
                self.config.debug_view,
            );
            q.submit(Some(encoder.finish()));
            surface.present();
            Ok(FrameStats {
                cpu_ms: (clock.now() - start_ms) as f32,
                ..FrameStats::default()
            })
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub use web::Renderer;
