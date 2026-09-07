//! Continuous surfaces and a bounded, fixed-step ink simulation.
use super::ViewTransform;
use crate::{
    settings::{Animation, Settings, COLOR_SCHEME_POOLSIDE},
    SharedResources,
};
use std::borrow::Cow;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    palette: [[f32; 4]; 6],
    controls: [f32; 4],
    options: [u32; 4],
    viewport: [f32; 4],
}

pub struct Context {
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bindings: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    ink_pipeline: wgpu::ComputePipeline,
    // Each group samples one texture and writes the other. Non-ink modes use 1x1.
    dye: [wgpu::Texture; 2],
    dye_bindings: [wgpu::BindGroup; 2],
    draw_bindings: [wgpu::BindGroup; 2],
    index: usize,
    noise_view: wgpu::TextureView,
    color_view: wgpu::TextureView,
    has_color_image: bool,
}

fn texture_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

impl Context {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        noise_view: &wgpu::TextureView,
        resources: &SharedResources,
    ) -> Self {
        let shader = resources.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:artwork"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/artwork.wgsl"
            ))),
        });
        let common_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("artwork:common"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(
                    2,
                    wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                ),
                texture_entry(
                    3,
                    wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                ),
            ],
        });
        let read_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("artwork:read-dye"),
            entries: &[texture_entry(0, wgpu::ShaderStages::FRAGMENT)],
        });
        let ink_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("artwork:ink-step"),
            entries: &[
                texture_entry(0, wgpu::ShaderStages::COMPUTE),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        // Matches the existing fluid read bind group, including its visibility.
        let velocity_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("artwork:velocity"),
            entries: &[
                texture_entry(0, wgpu::ShaderStages::COMPUTE),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rg32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let render_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("artwork:render-layout"),
            bind_group_layouts: &[&common_layout, &read_layout],
            push_constant_ranges: &[],
        });
        let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("artwork:compute-layout"),
            bind_group_layouts: &[&common_layout, &ink_layout, &velocity_layout],
            push_constant_ranges: &[],
        });
        let pipeline = resources.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline:artwork"),
            layout: Some(&render_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(format.into())],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let ink_pipeline = resources.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("pipeline:ink"),
            layout: Some(&compute_layout),
            module: &shader,
            entry_point: Some("ink"),
            compilation_options: Default::default(),
            cache: None,
        });
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("artwork:uniforms"),
            contents: bytemuck::bytes_of(&Uniforms {
                palette: [[0.0; 4]; 6],
                controls: [1.0; 4],
                options: [0; 4],
                viewport: [0.0, 0.0, 1.0, 1.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("artwork:linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let dye = Self::make_dye(device, 1, 1);
        let color_view = dye[0].create_view(&Default::default());
        let bindings = Self::make_bindings(
            device,
            &pipeline,
            &uniform_buffer,
            &sampler,
            noise_view,
            &color_view,
        );
        let (dye_bindings, draw_bindings) =
            Self::make_dye_bindings(device, &pipeline, &ink_pipeline, &dye);
        Self {
            uniform_buffer,
            sampler,
            bindings,
            pipeline,
            ink_pipeline,
            dye,
            dye_bindings,
            draw_bindings,
            index: 0,
            noise_view: noise_view.clone(),
            color_view,
            has_color_image: false,
        }
    }

    fn make_bindings(
        device: &wgpu::Device,
        pipeline: &wgpu::RenderPipeline,
        buffer: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        noise: &wgpu::TextureView,
        color: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("artwork:bindings"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(noise),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(color),
                },
            ],
        })
    }

    fn make_dye(device: &wgpu::Device, width: u32, height: u32) -> [wgpu::Texture; 2] {
        std::array::from_fn(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("artwork:dye"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                view_formats: &[],
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            })
        })
    }

    fn make_dye_bindings(
        device: &wgpu::Device,
        pipeline: &wgpu::RenderPipeline,
        ink_pipeline: &wgpu::ComputePipeline,
        dye: &[wgpu::Texture; 2],
    ) -> ([wgpu::BindGroup; 2], [wgpu::BindGroup; 2]) {
        let views = dye.each_ref().map(|t| t.create_view(&Default::default()));
        let compute = std::array::from_fn(|index| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("artwork:dye-step"),
                layout: &ink_pipeline.get_bind_group_layout(1),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&views[index]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&views[1 - index]),
                    },
                ],
            })
        });
        let render = std::array::from_fn(|index| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("artwork:dye-draw"),
                layout: &pipeline.get_bind_group_layout(1),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&views[index]),
                }],
            })
        });
        (compute, render)
    }

    pub fn set_color_image(&mut self, device: &wgpu::Device, view: &wgpu::TextureView) {
        self.color_view = view.clone();
        self.has_color_image = true;
        self.rebind(device);
    }

    pub fn clear_color_image(&mut self) {
        self.has_color_image = false;
    }

    fn rebind(&mut self, device: &wgpu::Device) {
        self.bindings = Self::make_bindings(
            device,
            &self.pipeline,
            &self.uniform_buffer,
            &self.sampler,
            &self.noise_view,
            &self.color_view,
        );
    }

    pub fn sync(
        &mut self,
        device: &wgpu::Device,
        noise: &wgpu::TextureView,
        fluid_size: wgpu::Extent3d,
        settings: &Settings,
    ) {
        if self.noise_view != *noise {
            self.noise_view = noise.clone();
            self.rebind(device);
        }
        let (width, height) = if settings.animation == Animation::Ink {
            // Preserve aspect while bounding the longest side to 1024 texels.
            let scale = (1024.0 / (fluid_size.width.max(fluid_size.height) * 4) as f32).min(1.0);
            (
                ((fluid_size.width * 4) as f32 * scale).round().max(1.0) as u32,
                ((fluid_size.height * 4) as f32 * scale).round().max(1.0) as u32,
            )
        } else {
            (1, 1)
        };
        if self.dye[0].width() != width || self.dye[0].height() != height {
            self.dye = Self::make_dye(device, width, height);
            (self.dye_bindings, self.draw_bindings) =
                Self::make_dye_bindings(device, &self.pipeline, &self.ink_pipeline, &self.dye);
            self.index = 0;
        }
    }

    pub fn update_uniforms(
        &self,
        queue: &wgpu::Queue,
        settings: &Settings,
        aspect: f32,
        view: ViewTransform,
    ) {
        let wheel = settings
            .color_mode
            .to_color_wheel()
            .unwrap_or(COLOR_SCHEME_POOLSIDE);
        let mut palette = [[0.0; 4]; 6];
        for (entry, rgba) in palette.iter_mut().zip(wheel.chunks_exact(4)) {
            entry.copy_from_slice(rgba);
        }
        let color_source = if self.has_color_image {
            2
        } else if settings.color_mode.to_color_wheel().is_some() {
            1
        } else {
            0
        };
        let uniforms = Uniforms {
            palette,
            controls: [
                aspect,
                settings.view_scale.max(0.1),
                settings.brightness_multiplier,
                settings.fluid_timestep,
            ],
            options: [
                settings.animation as u32,
                color_source,
                settings.grid_spacing,
                0,
            ],
            viewport: [
                (1.0 - (1.0 + view.offset[0]) / view.scale[0]) * 0.5,
                (1.0 - (1.0 - view.offset[1]) / view.scale[1]) * 0.5,
                1.0 / view.scale[0],
                1.0 / view.scale[1],
            ],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn step(&mut self, cpass: &mut wgpu::ComputePass<'_>, velocity: &wgpu::BindGroup) {
        cpass.set_pipeline(&self.ink_pipeline);
        cpass.set_bind_group(0, &self.bindings, &[]);
        cpass.set_bind_group(1, &self.dye_bindings[self.index], &[]);
        cpass.set_bind_group(2, velocity, &[]);
        cpass.dispatch_workgroups(
            self.dye[0].width().div_ceil(16),
            self.dye[0].height().div_ceil(16),
            1,
        );
        self.index = 1 - self.index;
    }

    pub fn draw(&self, rpass: &mut wgpu::RenderPass<'_>) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bindings, &[]);
        rpass.set_bind_group(1, &self.draw_bindings[self.index], &[]);
        rpass.draw(0..3, 0..1);
    }
}
