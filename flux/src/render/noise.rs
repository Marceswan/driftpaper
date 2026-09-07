use crate::{grid, rng, settings};

use std::borrow::Cow;
use std::sync::Arc;
use wgpu::util::DeviceExt;

pub struct NoiseGenerator {
    elapsed_time: f32, // TODO: reset
    linear_sampler: wgpu::Sampler,

    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    scaling_ratio: grid::ScalingRatio,

    uniforms: NoiseUniforms,

    channel_settings: Vec<settings::Noise>,
    channels: Vec<NoiseChannel>,

    uniform_buffer: wgpu::Buffer,
    channel_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    push_constants_buffer: wgpu::Buffer,
    inject_noise_bind_group: wgpu::BindGroup,

    generate_noise_pipeline: wgpu::ComputePipeline,
    inject_noise_pipeline: wgpu::ComputePipeline,
}

impl NoiseGenerator {
    pub fn resize(&mut self, device: &wgpu::Device, size: u32, scaling_ratio: grid::ScalingRatio) {
        let (width, height) = (
            size * scaling_ratio.rounded_x(),
            size * scaling_ratio.rounded_y(),
        );
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        self.scaling_ratio = scaling_ratio;
        if self.texture.size() == size {
            return;
        }
        let (texture, texture_view) = create_texture(device, &size);

        self.scaling_ratio = scaling_ratio;
        self.texture = texture;
        self.texture_view = texture_view;
        self.rebuild_bindings(device);
    }

    fn rebuild_bindings(&mut self, device: &wgpu::Device) {
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:noise"),
            layout: &self.generate_noise_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.channel_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.texture_view),
                },
            ],
        });
        self.inject_noise_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:inject_noise"),
            layout: &self.inject_noise_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.push_constants_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
            ],
        });
    }

    pub fn update(&mut self, device: &wgpu::Device, new_settings: &settings::Settings) {
        self.uniforms.multiplier = new_settings.noise_multiplier;
        if self.channel_settings == new_settings.noise_channels {
            return;
        }
        self.channels.truncate(new_settings.noise_channels.len());
        for settings in new_settings.noise_channels.iter().skip(self.channels.len()) {
            self.channels
                .push(NoiseChannel::new(self.scaling_ratio, settings));
        }
        // A runtime-sized storage array requires at least one element.
        if self.channels.is_empty() {
            self.channels.push(bytemuck::Zeroable::zeroed());
        }
        let required_size = std::mem::size_of_val(self.channels.as_slice()) as u64;
        if self.channel_buffer.size() != required_size {
            self.channel_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("storage:noise_channels"),
                size: required_size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.rebuild_bindings(device);
        }
        self.channel_settings
            .clone_from(&new_settings.noise_channels);
    }

    pub fn update_buffers(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        timestep: f32,
    ) {
        self.elapsed_time += timestep;
        self.channels
            .iter_mut()
            .zip(&self.channel_settings)
            .for_each(|(channel, settings)| {
                channel.tick(settings, self.elapsed_time);
            });
        // Queue writes all run before submission. Encode copies so each simulation
        // substep observes its own noise state rather than the last substep's state.
        let mut data = Vec::with_capacity(32 + self.channels.len() * 32);
        data.extend_from_slice(bytemuck::cast_slice(&[0.0_f32, 0.0, 0.0, timestep]));
        data.extend_from_slice(bytemuck::bytes_of(&self.uniforms));
        data.extend_from_slice(bytemuck::cast_slice(&self.channels));
        let upload = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("upload:noise_step"),
            contents: &data,
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        encoder.copy_buffer_to_buffer(&upload, 0, &self.push_constants_buffer, 0, 16);
        encoder.copy_buffer_to_buffer(&upload, 16, &self.uniform_buffer, 0, 16);
        encoder.copy_buffer_to_buffer(&upload, 32, &self.channel_buffer, 0, data.len() as u64 - 32);
    }

    pub fn generate<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let workgroup = (
            self.texture.size().width.div_ceil(16),
            self.texture.size().height.div_ceil(16),
            1,
        );
        cpass.set_pipeline(&self.generate_noise_pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn inject_noise_into<'cpass>(
        &'cpass self,
        cpass: &mut wgpu::ComputePass<'cpass>,
        target_texture_bind_group: &'cpass wgpu::BindGroup,
        target_texture_size: wgpu::Extent3d,
    ) {
        let workgroup = (
            target_texture_size.width.div_ceil(16),
            target_texture_size.height.div_ceil(16),
            1,
        );
        cpass.set_pipeline(&self.inject_noise_pipeline);
        cpass.set_bind_group(0, &self.inject_noise_bind_group, &[]);
        cpass.set_bind_group(1, target_texture_bind_group, &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn get_noise_texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }
}

pub struct NoiseGeneratorBuilder {
    settings: Arc<settings::Settings>,
    size: u32,
    scaling_ratio: grid::ScalingRatio,
    channels: Vec<settings::Noise>,
}

impl NoiseGeneratorBuilder {
    // TODO: just provide the final size, no scaling ratio
    pub fn new(
        size: u32,
        scaling_ratio: grid::ScalingRatio,
        settings: &Arc<settings::Settings>,
    ) -> Self {
        NoiseGeneratorBuilder {
            settings: Arc::clone(settings),
            size,
            scaling_ratio,
            channels: Vec::new(),
        }
    }

    pub fn add_channel(&mut self, channel: &settings::Noise) -> &Self {
        self.channels.push(channel.clone());

        self
    }

    pub fn build(self, device: &wgpu::Device, queue: &wgpu::Queue) -> NoiseGenerator {
        self.build_with_resources(
            device,
            queue,
            &Arc::new(crate::SharedResources::new(device)),
        )
    }

    pub(crate) fn build_with_resources(
        self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        resources: &Arc<crate::SharedResources>,
    ) -> NoiseGenerator {
        log::info!("🎛 Generating noise");

        let uniforms = NoiseUniforms::new(&self.settings);
        let mut channels = self
            .channels
            .iter()
            .map(|channel| NoiseChannel::new(self.scaling_ratio, channel))
            .collect::<Vec<_>>();
        if channels.is_empty() {
            channels.push(bytemuck::Zeroable::zeroed());
        }

        let (width, height) = (
            self.size * self.scaling_ratio.rounded_x(),
            self.size * self.scaling_ratio.rounded_y(),
        );

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let (texture, texture_view) = create_texture(device, &size);

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler:linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform:noise"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let channel_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("storage:noise_channels"),
            contents: bytemuck::cast_slice(&channels),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind_group_layout:noise"),
            entries: &[
                // noiseTexture
                // wgpu::BindGroupLayoutEntry {
                //     binding: 0,
                //     visibility: wgpu::ShaderStages::COMPUTE,
                //     ty: wgpu::BindingType::Texture {
                //         sample_type: wgpu::TextureSampleType::Float { filterable: true },
                //         view_dimension: wgpu::TextureViewDimension::D2,
                //         multisampled: false,
                //     },
                //     count: None,
                // },
                // // sampler
                // wgpu::BindGroupLayoutEntry {
                //     binding: 2,
                //     visibility: wgpu::ShaderStages::COMPUTE,
                //     ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                //     count: None,
                // },
                // uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // channels
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // outTexture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:noise"),
            layout: &bind_group_layout,
            entries: &[
                // wgpu::BindGroupEntry {
                //     binding: 2,
                //     resource: wgpu::BindingResource::Sampler(&linear_sampler),
                // },
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &channel_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout:generate_noise"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let generate_noise_shader = resources.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:generate_noise"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/generate_noise.comp.wgsl"
            ))),
        });

        let generate_noise_pipeline =
            resources.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("pipeline:generate_noise"),
                layout: Some(&pipeline_layout),
                module: &generate_noise_shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        let push_constants_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("push_constants:noise"),
            contents: bytemuck::cast_slice(&[0.0, 0.0, 0.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let inject_noise_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Inject noise bind group layout"),
                entries: &[
                    // push_constants
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // noise_texure
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let inject_noise_bind_group_layout_2 =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Inject noise bind group layout 2"),
                entries: &[
                    // velocity_texture
                    // wgpu::BindGroupLayoutEntry {
                    //     binding: 0,
                    //     visibility: wgpu::ShaderStages::COMPUTE,
                    //     ty: wgpu::BindingType::StorageTexture {
                    //         access: wgpu::StorageTextureAccess::ReadOnly,
                    //         format: wgpu::TextureFormat::Rg32Float,
                    //         view_dimension: wgpu::TextureViewDimension::D2,
                    //     },
                    //     count: None,
                    // },
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // out_velocity_texture
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

        let inject_noise_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Inject noise bind group"),
            layout: &inject_noise_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &push_constants_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
            ],
        });

        let inject_noise_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Inject noise layout"),
                bind_group_layouts: &[
                    &inject_noise_bind_group_layout,
                    &inject_noise_bind_group_layout_2,
                ],
                push_constant_ranges: &[],
            });

        let inject_noise_shader = resources.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Inject noise shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/inject_noise.comp.wgsl"
            ))),
        });

        let inject_noise_pipeline =
            resources.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Inject noise"),
                layout: Some(&inject_noise_pipeline_layout),
                module: &inject_noise_shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        NoiseGenerator {
            linear_sampler,
            elapsed_time: 0.0,

            uniforms,
            channel_settings: self.channels,
            channels,

            uniform_buffer,
            channel_buffer,
            scaling_ratio: self.scaling_ratio,
            texture,
            texture_view,
            bind_group,
            inject_noise_bind_group,
            push_constants_buffer,

            generate_noise_pipeline,
            inject_noise_pipeline,
        }
    }
}

fn create_texture(
    device: &wgpu::Device,
    size: &wgpu::Extent3d,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("texture:noise"),
        size: *size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // TODO: try RG16Float
        format: wgpu::TextureFormat::Rg32Float,
        view_formats: &[],
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | if cfg!(test) {
                wgpu::TextureUsages::COPY_SRC
            } else {
                wgpu::TextureUsages::empty()
            },
    });

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("view:noise"),
        ..Default::default()
    });

    (texture, texture_view)
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NoiseChannel {
    scale: [f32; 2],   // 0
    offset_1: f32,     // 8
    offset_2: f32,     // 12
    blend_factor: f32, //16
    multiplier: f32,   // 20
    _padding: [u32; 2], // 24
                       // roundUp(8, 24) = 24 -> 32 for uniform
}

impl NoiseChannel {
    const BLEND_THRESHOLD: f32 = 1000.0;

    pub fn new(scaling_ratio: grid::ScalingRatio, channel_settings: &settings::Noise) -> Self {
        Self {
            scale: [
                channel_settings.scale * scaling_ratio.x(),
                channel_settings.scale * scaling_ratio.y(),
            ],
            offset_1: Self::BLEND_THRESHOLD * rng::gen::<f32>(),
            offset_2: 0.0,
            blend_factor: 0.0,
            multiplier: channel_settings.multiplier,
            _padding: [0; 2],
        }
    }

    pub fn tick(&mut self, channel_settings: &settings::Noise, elapsed_time: f32) {
        let scale = channel_settings.scale
            * (1.0 + 0.15 * (0.01 * elapsed_time * std::f32::consts::TAU).sin());
        self.scale = [scale, scale];
        self.multiplier = channel_settings.multiplier;
        self.offset_1 += channel_settings.offset_increment;

        if self.offset_1 > Self::BLEND_THRESHOLD {
            self.blend_factor += channel_settings.offset_increment;
            self.offset_2 += channel_settings.offset_increment;
        }

        // Reset blending
        if self.blend_factor > 1.0 {
            self.offset_1 = self.offset_2;
            self.offset_2 = 0.0;
            self.blend_factor = 0.0;
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NoiseUniforms {
    multiplier: f32, // 0
    _padding: [u32; 3],
}

impl NoiseUniforms {
    fn new(settings: &settings::Settings) -> Self {
        Self {
            multiplier: settings.noise_multiplier,
            _padding: [0, 0, 0],
        }
    }
}

#[cfg(test)]
impl NoiseGenerator {
    pub(crate) fn assert_generated(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        expect_noise: bool,
    ) {
        let data = crate::test_support::read_texture(device, queue, &self.texture, 8);
        let nonzero = data
            .chunks_exact(4)
            .any(|bytes| f32::from_ne_bytes(bytes.try_into().unwrap()).abs() > 1e-6);
        assert_eq!(
            nonzero, expect_noise,
            "noise generation must target the current resized texture"
        );
    }

    pub(crate) fn assert_resource_sizes(&self, fluid_size: wgpu::Extent3d) {
        assert_eq!(self.texture.size().width, fluid_size.width * 2);
        assert_eq!(self.texture.size().height, fluid_size.height * 2);
        assert_eq!(self.channel_buffer.size(), self.channels.len() as u64 * 32);
    }
}
