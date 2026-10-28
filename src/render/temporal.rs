//! Temporal accumulation at ray resolution. Recreate on resolution changes.
use wgpu::util::DeviceExt;

struct Image {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}
impl Image {
    fn new(device: &wgpu::Device, size: [u32; 2]) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("temporal history"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            _texture: texture,
            view,
        }
    }
}
struct History {
    reflection: Image,
    auxiliary: Image,
    metadata: Image,
}
impl History {
    fn new(device: &wgpu::Device, size: [u32; 2]) -> Self {
        Self {
            reflection: Image::new(device, size),
            auxiliary: Image::new(device, size),
            metadata: Image::new(device, size),
        }
    }
}
pub struct Temporal {
    size: [u32; 2],
    history: [History; 2],
    current: usize,
    valid: bool,
    pipeline: wgpu::ComputePipeline,
    filter_pipeline: wgpu::ComputePipeline,
    filtered_reflection: Image,
    filtered_auxiliary: Image,
    uniform: wgpu::Buffer,
}
impl Temporal {
    pub fn new(device: &wgpu::Device, size: [u32; 2]) -> Self {
        let size = [size[0].max(1), size[1].max(1)];
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("temporal"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/temporal.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("temporal"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let filter_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("temporal spatial filter"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/temporal_filter.wgsl").into(),
            ),
        });
        let filter_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("temporal spatial filter"),
            layout: None,
            module: &filter_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("temporal params"),
            contents: &[0; 144],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self {
            size,
            history: [History::new(device, size), History::new(device, size)],
            current: 0,
            valid: false,
            pipeline,
            filter_pipeline,
            filtered_reflection: Image::new(device, size),
            filtered_auxiliary: Image::new(device, size),
            uniform,
        }
    }
    pub fn reset(&mut self) {
        self.valid = false;
    }
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        raw_reflection: &wgpu::TextureView,
        raw_ao: &wgpu::TextureView,
        motion: &wgpu::TextureView,
        normal: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        frame: &crate::contract::FrameInput,
    ) {
        let mut data = [0u8; 144];
        data[0..64].copy_from_slice(bytemuck::bytes_of(&frame.inv_view_proj.to_cols_array()));
        data[64..128].copy_from_slice(bytemuck::bytes_of(&frame.view_proj.to_cols_array()));
        data[128..132].copy_from_slice(&(u32::from(self.valid && !frame.camera_cut)).to_ne_bytes());
        queue.write_buffer(&self.uniform, 0, &data);
        let next = 1 - self.current;
        let old = &self.history[self.current];
        let out = &self.history[next];
        let views = [
            raw_reflection,
            raw_ao,
            motion,
            normal,
            depth,
            &old.reflection.view,
            &old.auxiliary.view,
            &old.metadata.view,
            &out.reflection.view,
            &out.auxiliary.view,
            &out.metadata.view,
        ];
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: self.uniform.as_entire_binding(),
        }];
        entries.extend(views.iter().enumerate().map(|(i, v)| wgpu::BindGroupEntry {
            binding: i as u32 + 1,
            resource: wgpu::BindingResource::TextureView(v),
        }));
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("temporal"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("temporal"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(self.size[0].div_ceil(8), self.size[1].div_ceil(8), 1);
        }
        let filter_views = [
            &out.reflection.view,
            &out.auxiliary.view,
            &out.metadata.view,
            normal,
            &self.filtered_reflection.view,
            &self.filtered_auxiliary.view,
        ];
        let filter_entries: Vec<_> = filter_views
            .iter()
            .enumerate()
            .map(|(i, v)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu::BindingResource::TextureView(v),
            })
            .collect();
        let filter_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("temporal spatial filter"),
            layout: &self.filter_pipeline.get_bind_group_layout(0),
            entries: &filter_entries,
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("temporal spatial filter"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.filter_pipeline);
            pass.set_bind_group(0, &filter_group, &[]);
            pass.dispatch_workgroups(self.size[0].div_ceil(8), self.size[1].div_ceil(8), 1);
        }
        self.current = next;
        self.valid = true;
    }
    pub fn reflection_view(&self) -> &wgpu::TextureView {
        &self.filtered_reflection.view
    }
    pub fn ao_view(&self) -> &wgpu::TextureView {
        &self.filtered_auxiliary.view
    }
}
