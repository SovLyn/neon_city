//! Linear-HDR deferred lighting and three-scale separable bloom.
//! Limitation: fixed 1280x720 working resolution; no resize support yet.
//! Output scales to the surface; depth lookups use the source texture dimensions.
use crate::contract::{DebugView, DynamicFrame, FrameInput, MaterialGpu};
use wgpu::util::DeviceExt;

struct Image {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}
fn image(device: &wgpu::Device, size: [u32; 2]) -> Image {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("post HDR"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    Image {
        _texture: texture,
        view,
    }
}
fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    fragment: &str,
    format: wgpu::TextureFormat,
    mrt: bool,
    particles: bool,
) -> wgpu::RenderPipeline {
    let target = Some(wgpu::ColorTargetState {
        format,
        blend: particles.then_some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    });
    let targets = if mrt {
        vec![target.clone(), target]
    } else {
        vec![target]
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: None,
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(if particles { "particle_vs" } else { "vs" }),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: &targets,
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}
fn texture(binding: u32, view: &wgpu::TextureView) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::TextureView(view),
    }
}
fn buffer(device: &wgpu::Device, bytes: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    let zero = [0; 64];
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("post data"),
        contents: if bytes.is_empty() { &zero } else { bytes },
        usage,
    })
}
fn group(
    device: &wgpu::Device,
    pipeline: &wgpu::RenderPipeline,
    entries: &[wgpu::BindGroupEntry<'_>],
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("post bind group"),
        layout: &pipeline.get_bind_group_layout(0),
        entries,
    })
}
fn draw(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
    views: &[&wgpu::TextureView],
    load: bool,
    instances: u32,
) {
    let attachments: Vec<_> = views
        .iter()
        .map(|view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if load {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    },
                    store: wgpu::StoreOp::Store,
                },
            })
        })
        .collect();
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("post pass"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..if load { 6 } else { 3 }, 0..instances);
}

// Matches WGSL Frame exactly; effects.z requests shader-side sRGB encoding.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FrameUniform {
    pub inv_vp: [f32; 16],
    pub vp: [f32; 16],
    pub camera_time: [f32; 4],
    pub counts: [u32; 4],
    pub effects: [u32; 4],
}

pub struct Post {
    lighting: wgpu::RenderPipeline,
    particles: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    horizontal: wgpu::RenderPipeline,
    vertical: wgpu::RenderPipeline,
    finish: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    materials: wgpu::Buffer,
    material_count: u32,
    sampler: wgpu::Sampler,
    hdr: Image,
    seed: Image,
    bloom: [[Image; 2]; 3],
    bloom_enabled: bool,
    atmosphere_enabled: bool,
    encode_srgb: bool,
}
impl Post {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/post.wgsl").into()),
        });
        let hdr_format = wgpu::TextureFormat::Rgba16Float;
        Self {
            lighting: pipeline(device, &shader, "lighting", hdr_format, true, false),
            particles: pipeline(device, &shader, "particle_fs", hdr_format, true, true),
            down: pipeline(device, &shader, "downsample", hdr_format, false, false),
            horizontal: pipeline(device, &shader, "blur_h", hdr_format, false, false),
            vertical: pipeline(device, &shader, "blur_v", hdr_format, false, false),
            finish: pipeline(device, &shader, "finish", format, false, false),
            uniform: buffer(
                device,
                &[0; std::mem::size_of::<FrameUniform>()],
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            // One full zeroed MaterialGpu keeps the binding valid before set_materials.
            materials: buffer(
                device,
                &[0; std::mem::size_of::<MaterialGpu>()],
                wgpu::BufferUsages::STORAGE,
            ),
            material_count: 0,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("post linear clamp"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            bloom_enabled: true,
            atmosphere_enabled: true,
            // sRGB attachments encode on store; plain UNORM surfaces do not.
            encode_srgb: !format.is_srgb(),
            hdr: image(device, [1280, 720]),
            seed: image(device, [1280, 720]),
            bloom: [[640, 360], [320, 180], [160, 90]]
                .map(|s| [image(device, s), image(device, s)]),
        }
    }
    /// Upload the static material table once; an empty table uses a safe dummy with count zero.
    pub fn set_materials(&mut self, device: &wgpu::Device, materials: &[MaterialGpu]) {
        self.materials = buffer(
            device,
            bytemuck::cast_slice(materials),
            wgpu::BufferUsages::STORAGE,
        );
        self.material_count = materials.len() as u32;
    }

    /// Toggle bloom and atmosphere (fog, light shafts, rain and steam).
    /// Both are enabled by default; this does not change scene lighting or exposure.
    pub fn set_effects(&mut self, bloom: bool, atmosphere: bool) {
        self.bloom_enabled = bloom;
        self.atmosphere_enabled = atmosphere;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        gbuffer: [&wgpu::TextureView; 4],
        depth: &wgpu::TextureView,
        reflection: &wgpu::TextureView,
        ao: &wgpu::TextureView,
        frame: &FrameInput,
        dynamic: &DynamicFrame,
        debug: DebugView,
    ) {
        let debug_id = match debug {
            DebugView::Final => 0,
            DebugView::Albedo => 1,
            DebugView::Normal => 2,
            DebugView::Roughness => 3,
            DebugView::Depth => 4,
            DebugView::Motion => 5,
            DebugView::Reflection => 6,
            DebugView::AmbientOcclusion => 7,
            DebugView::Shadow => 8,
            DebugView::HistoryRejection => 9,
            DebugView::Bvh => 10,
        };
        let uniform = FrameUniform {
            inv_vp: frame.inv_view_proj.to_cols_array(),
            vp: frame.view_proj.to_cols_array(),
            camera_time: [
                frame.camera_world.x,
                frame.camera_world.y,
                frame.camera_world.z,
                frame.time_s,
            ],
            counts: [dynamic.lights.len().min(64) as u32, debug_id, 1280, 720],
            effects: [
                u32::from(self.bloom_enabled),
                u32::from(self.atmosphere_enabled),
                u32::from(self.encode_srgb),
                self.material_count,
            ],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        let lights = buffer(
            device,
            bytemuck::cast_slice(&dynamic.lights[..dynamic.lights.len().min(64)]),
            wgpu::BufferUsages::STORAGE,
        );
        let group_lit = group(
            device,
            &self.lighting,
            &[
                texture(0, gbuffer[0]),
                texture(1, gbuffer[1]),
                texture(2, gbuffer[2]),
                texture(3, gbuffer[3]),
                texture(4, depth),
                texture(5, reflection),
                texture(6, ao),
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: lights.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 18,
                    resource: self.materials.as_entire_binding(),
                },
            ],
        );
        draw(
            encoder,
            &self.lighting,
            &group_lit,
            &[&self.hdr.view, &self.seed.view],
            false,
            1,
        );
        if self.atmosphere_enabled && debug == DebugView::Final && !dynamic.particles.is_empty() {
            let data = buffer(
                device,
                bytemuck::cast_slice(&dynamic.particles),
                wgpu::BufferUsages::STORAGE,
            );
            let bg = group(
                device,
                &self.particles,
                &[
                    texture(4, depth),
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: self.uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 17,
                        resource: data.as_entire_binding(),
                    },
                ],
            );
            draw(
                encoder,
                &self.particles,
                &bg,
                &[&self.hdr.view, &self.seed.view],
                true,
                dynamic.particles.len() as u32,
            );
        }
        // Finish samples bloom only in Final with bloom enabled; stale levels are unused.
        if self.bloom_enabled && debug == DebugView::Final {
            for level in 0..3 {
                let source = if level == 0 {
                    &self.seed.view
                } else {
                    &self.bloom[level - 1][0].view
                };
                for (pipeline, input, output) in [
                    (&self.down, source, &self.bloom[level][0].view),
                    (
                        &self.horizontal,
                        &self.bloom[level][0].view,
                        &self.bloom[level][1].view,
                    ),
                    (
                        &self.vertical,
                        &self.bloom[level][1].view,
                        &self.bloom[level][0].view,
                    ),
                ] {
                    let bg = group(
                        device,
                        pipeline,
                        &[
                            wgpu::BindGroupEntry {
                                binding: 10,
                                resource: wgpu::BindingResource::Sampler(&self.sampler),
                            },
                            texture(11, input),
                        ],
                    );
                    draw(encoder, pipeline, &bg, &[output], false, 1);
                }
            }
        }
        let final_group = group(
            device,
            &self.finish,
            &[
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                texture(12, &self.hdr.view),
                texture(13, &self.bloom[0][0].view),
                texture(14, &self.bloom[1][0].view),
                texture(15, &self.bloom[2][0].view),
            ],
        );
        draw(encoder, &self.finish, &final_group, &[output], false, 1);
    }
}
