use crate::SharedResources;
use crate::{grid, render, rng, settings};
use settings::Settings;

use std::sync::Arc;
use std::sync::Mutex;

// The time at which the animation timer will reset to zero.
const MAX_ELAPSED_TIME: f32 = 1000.0;
const MAX_FRAME_TIME: f32 = 1.0 / 10.0;

pub struct Flux {
    resources: Arc<SharedResources>,
    swapchain_format: wgpu::TextureFormat,
    settings: Arc<Settings>,
    logical_size: wgpu::Extent3d,
    physical_size: wgpu::Extent3d,

    grid: grid::Grid,
    fluid: render::fluid::Context,
    pub lines: render::lines::Context,
    noise_generator: render::noise::NoiseGenerator,
    debug_texture: render::texture::Context,
    artwork: Option<render::artwork::Context>,
    color_texture: Option<wgpu::TextureView>,

    pub color_image: Arc<Mutex<Option<image::RgbaImage>>>,

    // A timestamp in milliseconds. Either host or video time.
    last_timestamp: f64,

    // A local animation timer in seconds that resets at MAX_ELAPSED_TIME.
    elapsed_time: f32,

    fluid_frame_time: f32,
}

impl Flux {
    /// Get the current grid spacing (used to detect density changes)
    pub fn grid_spacing(&self) -> u32 {
        self.settings.grid_spacing
    }

    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, settings: &Arc<Settings>) {
        if *self.settings == **settings {
            return;
        }
        if settings.grid_spacing == 0
            || settings.fluid_size == 0
            || !settings.fluid_timestep.is_finite()
            || settings.fluid_timestep <= 0.0
            || !settings.animation_speed.is_finite()
            || !(0.25..=2.0).contains(&settings.animation_speed)
        {
            log::warn!("Ignoring invalid simulation dimensions, timestep or animation speed");
            return;
        }
        let resize_grid = self.settings.grid_spacing != settings.grid_spacing;
        if self.settings.color_mode != settings.color_mode {
            self.color_texture = None;
            if let Some(artwork) = &mut self.artwork {
                artwork.clear_color_image();
            }
        }
        self.settings = Arc::clone(settings);
        if resize_grid {
            self.resize(
                device,
                queue,
                self.logical_size.width,
                self.logical_size.height,
                self.physical_size.width,
                self.physical_size.height,
            );
        } else {
            self.lines
                .update(device, queue, self.logical_size, &self.grid, &self.settings);
            self.resize_simulation(device, queue);
        }
        self.noise_generator
            .update(device, &self.settings.noise_profile());
    }

    fn resize_simulation(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let old_size = self.fluid.get_fluid_size();
        self.fluid
            .update(device, queue, self.grid.scaling_ratio, &self.settings);
        self.noise_generator.resize(
            device,
            2 * self.settings.fluid_size,
            self.grid.scaling_ratio,
        );
        if old_size != self.fluid.get_fluid_size() {
            self.debug_texture = render::texture::Context::new_with_resources(
                device,
                self.swapchain_format,
                &[
                    ("fluid", self.fluid.get_velocity_texture_view()),
                    ("noise", self.noise_generator.get_noise_texture_view()),
                    ("pressure", self.fluid.get_pressure_texture_view()),
                    ("divergence", self.fluid.get_divergence_texture_view()),
                ],
                &self.resources,
            );
        }
    }

    pub fn sample_colors_from_image(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &image::RgbaImage,
    ) {
        let texture_view = render::color::load_color_texture(device, queue, image);
        self.sample_colors_from_texture_view(device, queue, texture_view);
    }

    pub fn sample_colors_from_texture_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_view: wgpu::TextureView,
    ) {
        self.color_texture = Some(texture_view.clone());
        if let Some(artwork) = &mut self.artwork {
            artwork.set_color_image(device, &texture_view);
        }
        self.lines
            .update_color_bindings(device, queue, Some(texture_view), None);
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        swapchain_format: wgpu::TextureFormat,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
        settings: &Arc<Settings>,
    ) -> Result<Flux, String> {
        Self::new_with_resources(
            device,
            queue,
            swapchain_format,
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            settings,
            &Arc::new(SharedResources::new(device)),
        )
    }

    pub fn new_with_resources(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        swapchain_format: wgpu::TextureFormat,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
        settings: &Arc<Settings>,
        resources: &Arc<SharedResources>,
    ) -> Result<Flux, String> {
        if !resources.belongs_to(device) {
            return Err("SharedResources belongs to a different GPU device".into());
        }
        if logical_width == 0
            || logical_height == 0
            || physical_width == 0
            || physical_height == 0
            || settings.grid_spacing == 0
            || settings.fluid_size == 0
            || !settings.fluid_timestep.is_finite()
            || settings.fluid_timestep <= 0.0
            || !settings.animation_speed.is_finite()
            || !(0.25..=2.0).contains(&settings.animation_speed)
        {
            return Err(
                "Dimensions, grid spacing, fluid size and timestep must be positive and finite; animation speed must be 0.25–2.0"
                    .into(),
            );
        }

        log::info!("✨ Initialising Flux");

        rng::init_from_seed(&settings.seed);

        let logical_size = wgpu::Extent3d {
            width: logical_width,
            height: logical_height,
            depth_or_array_layers: 1,
        };
        let physical_size = wgpu::Extent3d {
            width: physical_width,
            height: physical_height,
            depth_or_array_layers: 1,
        };

        log::info!("📐 Logical size: {}x{}", logical_width, logical_height);
        log::info!("📏 Physical size: {}x{}", physical_width, physical_height);

        let grid = grid::Grid::new(logical_width, logical_height, settings.grid_spacing);

        let fluid = render::fluid::Context::new_with_resources(
            device,
            queue,
            grid.scaling_ratio,
            settings,
            resources,
        );

        let lines = render::lines::Context::new_with_resources(
            device,
            queue,
            swapchain_format,
            logical_size,
            &grid,
            settings,
            resources,
        );

        let noise_settings = Arc::new(settings.noise_profile());
        let mut noise_generator_builder = render::noise::NoiseGeneratorBuilder::new(
            2 * settings.fluid_size,
            grid.scaling_ratio,
            &noise_settings,
        );
        noise_settings.noise_channels.iter().for_each(|channel| {
            noise_generator_builder.add_channel(channel);
        });
        let noise_generator =
            noise_generator_builder.build_with_resources(device, queue, resources);

        let debug_texture = render::texture::Context::new_with_resources(
            device,
            swapchain_format,
            &[
                ("fluid", fluid.get_velocity_texture_view()),
                ("noise", noise_generator.get_noise_texture_view()),
                ("pressure", fluid.get_pressure_texture_view()),
                ("divergence", fluid.get_divergence_texture_view()),
            ],
            resources,
        );

        Ok(Flux {
            resources: Arc::clone(resources),
            swapchain_format,
            settings: Arc::clone(settings),
            logical_size,
            physical_size,

            fluid,
            grid,
            lines,
            noise_generator,
            debug_texture,
            artwork: None,
            color_texture: None,
            color_image: Arc::new(Mutex::new(None)),

            last_timestamp: 0.0,
            elapsed_time: 0.0,

            fluid_frame_time: 0.0,
        })
    }

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
    ) {
        if logical_width == 0 || logical_height == 0 || physical_width == 0 || physical_height == 0
        {
            return;
        }
        let columns = (logical_width / self.settings.grid_spacing).max(1) + 1;
        let rows = ((logical_height as f32 / logical_width as f32) * (columns - 1) as f32)
            .floor()
            .max(1.0) as u32
            + 1;
        if self.logical_size.width == logical_width
            && self.logical_size.height == logical_height
            && self.grid.columns == columns
            && self.grid.rows == rows
        {
            self.physical_size.width = physical_width;
            self.physical_size.height = physical_height;
            self.lines
                .update(device, queue, self.logical_size, &self.grid, &self.settings);
            self.resize_simulation(device, queue);
            return;
        }
        let grid = grid::Grid::new(logical_width, logical_height, self.settings.grid_spacing);

        // TODO: fetch line state from GPU and resample for new grid
        let logical_size = wgpu::Extent3d {
            width: logical_width,
            height: logical_height,
            depth_or_array_layers: 1,
        };
        let physical_size = wgpu::Extent3d {
            width: physical_width,
            height: physical_height,
            depth_or_array_layers: 1,
        };

        self.lines
            .resize(device, queue, logical_size, &grid, &self.settings);

        self.grid = grid;
        self.logical_size = logical_size;
        self.physical_size = physical_size;

        self.resize_simulation(device, queue);
    }

    pub fn animate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        screen_viewport: Option<render::ScreenViewport>,
        timestamp: f64,
    ) {
        self.compute(device, queue, encoder, timestamp);
        self.render(device, queue, encoder, view, screen_viewport);
    }

    pub fn compute(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        timestamp: f64,
    ) {
        // The delta time in seconds
        let timestep = f32::min(
            MAX_FRAME_TIME,
            (0.001 * (timestamp - self.last_timestamp) as f32).max(0.0),
        );

        let timestep = timestep * self.settings.animation_speed;
        self.last_timestamp = timestamp;
        self.elapsed_time += timestep;
        self.fluid_frame_time += timestep;

        // Reset animation timers to avoid precision issues
        let timer_overflow = self.elapsed_time - MAX_ELAPSED_TIME;
        if timer_overflow >= 0.0 {
            self.elapsed_time = timer_overflow;
        }

        let surfaces = self.settings.animation != settings::Animation::Drift;
        if surfaces {
            if self.artwork.is_none() {
                let mut artwork = render::artwork::Context::new(
                    device,
                    self.swapchain_format,
                    self.noise_generator.get_noise_texture_view(),
                    &self.resources,
                );
                if let Some(view) = &self.color_texture {
                    artwork.set_color_image(device, view);
                }
                self.artwork = Some(artwork);
            }
            let artwork = self.artwork.as_mut().unwrap();
            artwork.sync(
                device,
                self.noise_generator.get_noise_texture_view(),
                self.fluid.get_fluid_size(),
                &self.settings,
            );
            artwork.update_uniforms(
                queue,
                &self.settings,
                self.grid.aspect_ratio,
                Default::default(),
            );
        } else {
            self.artwork = None;
        }
        let needs_fluid = matches!(
            self.settings.animation,
            settings::Animation::Drift | settings::Animation::Ink
        ) || self.settings.mode != settings::Mode::Normal;
        while self.fluid_frame_time >= self.settings.fluid_timestep {
            self.noise_generator
                .update_buffers(device, encoder, self.settings.fluid_timestep);

            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("flux::compute"),
                timestamp_writes: None,
            });

            self.noise_generator.generate(&mut cpass);

            if needs_fluid {
                self.fluid.advect_forward(queue, &mut cpass);
                self.fluid.advect_reverse(queue, &mut cpass);
                self.fluid.adjust_advection(&mut cpass);
                self.fluid.diffuse(&mut cpass);

                let velocity_bind_group = self.fluid.get_write_velocity_bind_group();
                self.noise_generator.inject_noise_into(
                    &mut cpass,
                    velocity_bind_group,
                    self.fluid.get_fluid_size(),
                );

                self.fluid.calculate_divergence(&mut cpass);
                self.fluid.solve_pressure(queue, &mut cpass);
                self.fluid.subtract_gradient(&mut cpass);
            }
            if self.settings.animation == settings::Animation::Ink {
                self.artwork
                    .as_mut()
                    .unwrap()
                    .step(&mut cpass, self.fluid.get_read_velocity_bind_group());
            }

            self.fluid_frame_time -= self.settings.fluid_timestep;
        }

        if !surfaces {
            self.lines.tick_line_uniforms(
                device,
                queue,
                timestep.min(MAX_FRAME_TIME),
                self.elapsed_time,
            );

            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("flux::place_lines"),
                timestamp_writes: None,
            });

            self.lines
                .place_lines(&mut cpass, self.fluid.get_read_velocity_bind_group());
        }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        screen_viewport: Option<render::ScreenViewport>,
    ) {
        encoder.push_debug_group("render lines");

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("flux::render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            use settings::Mode::*;
            match &self.settings.mode {
                Normal => {
                    let view_transform = screen_viewport
                        .map(|ref sv| {
                            render::ViewTransform::from_screen_viewport(&self.physical_size, sv)
                        })
                        .unwrap_or_default();
                    if let Some(artwork) = &self.artwork {
                        artwork.update_uniforms(
                            queue,
                            &self.settings,
                            self.grid.aspect_ratio,
                            view_transform,
                        );
                        artwork.draw(&mut rpass);
                    } else {
                        self.lines.set_view_transform(queue, view_transform);
                        self.lines.draw_lines(&mut rpass);
                        self.lines.draw_endpoints(&mut rpass);
                    }
                }
                DebugNoise => {
                    self.debug_texture.draw_texture(device, &mut rpass, "noise");
                }
                DebugFluid => {
                    self.debug_texture.draw_texture(device, &mut rpass, "fluid");
                }
                DebugPressure => {
                    self.debug_texture
                        .draw_texture(device, &mut rpass, "pressure");
                }
                DebugDivergence => {
                    self.debug_texture
                        .draw_texture(device, &mut rpass, "divergence");
                }
            };
        }

        encoder.pop_debug_group();
    }
}

// #[derive(Debug)]
// pub enum Problem {
//     ReadSettings(String),
//     ReadImage(std::io::Error),
//     DecodeColorTexture(image::ImageError),
//     Render(render::Problem),
// }
//
// impl fmt::Display for Problem {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Problem::ReadSettings(msg) => write!(f, "{}", msg),
//             Problem::ReadImage(msg) => write!(f, "{}", msg),
//             Problem::DecodeColorTexture(msg) => write!(f, "Failed to decode image: {}", msg),
//             Problem::Render(render_msg) => write!(f, "{}", render_msg),
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_animation_modes_evolve_recolor_and_resize() {
        let Some((device, queue)) = crate::test_support::gpu() else {
            return;
        };
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let resources = Arc::new(SharedResources::new(&device));
        let mut settings = Arc::new(Settings {
            seed: Some("synthetic-preview".into()),
            fluid_size: 64,
            color_mode: settings::ColorMode::Preset(settings::ColorPreset::Poolside),
            ..Default::default()
        });
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut flux = Flux::new_with_resources(
            &device, &queue, format, 1280, 800, 640, 400, &settings, &resources,
        )
        .unwrap();
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("animation verification"),
            size: wgpu::Extent3d {
                width: 640,
                height: 400,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let mut time = 0.0;
        let mut previous_mode = Vec::new();
        for animation in settings::Animation::ALL.into_iter().skip(1) {
            Arc::make_mut(&mut settings).animation = animation;
            Arc::make_mut(&mut settings).color_mode =
                settings::ColorMode::Preset(match animation {
                    settings::Animation::Aurora => settings::ColorPreset::Aurora,
                    settings::Animation::Caustics => settings::ColorPreset::DeepOcean,
                    settings::Animation::Metal => settings::ColorPreset::Moonlight,
                    _ => settings::ColorPreset::Poolside,
                });
            flux.update(&device, &queue, &settings);
            let warmup = if std::env::var_os("DRIFTPAPER_PREVIEW_DIR").is_some() {
                300
            } else {
                90
            };
            for _ in 0..warmup {
                time += 1000.0 / 30.0;
                let mut encoder = device.create_command_encoder(&Default::default());
                flux.animate(&device, &queue, &mut encoder, &view, None, time);
                queue.submit([encoder.finish()]);
            }
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            if let Some(error) = pollster::block_on(device.pop_error_scope()) {
                panic!("{animation:?}: {error}");
            }
            device.push_error_scope(wgpu::ErrorFilter::Validation);
            let first = crate::test_support::read_texture(&device, &queue, &target, 4);
            assert!(
                first
                    .chunks_exact(4)
                    .filter(|p| p[..3].iter().any(|&c| c > 12))
                    .count()
                    > 2000,
                "{animation:?} should draw visible artwork"
            );
            assert!(first != previous_mode, "modes must have distinct output");
            if let Ok(directory) = std::env::var("DRIFTPAPER_PREVIEW_DIR") {
                std::fs::create_dir_all(&directory).unwrap();
                image::RgbaImage::from_raw(640, 400, first.clone())
                    .unwrap()
                    .save(std::path::Path::new(&directory).join(format!("{animation:?}.png")))
                    .unwrap();
            }
            for _ in 0..30 {
                time += 1000.0 / 30.0;
                let mut encoder = device.create_command_encoder(&Default::default());
                flux.animate(&device, &queue, &mut encoder, &view, None, time);
                queue.submit([encoder.finish()]);
            }
            let later = crate::test_support::read_texture(&device, &queue, &target, 4);
            assert!(first != later, "{animation:?} must animate");
            // Recolor with no time advance to prove palette is independent of motion.
            Arc::make_mut(&mut settings).color_mode = settings::ColorMode::Custom([0.2; 24]);
            flux.update(&device, &queue, &settings);
            let mut encoder = device.create_command_encoder(&Default::default());
            flux.animate(&device, &queue, &mut encoder, &view, None, time);
            queue.submit([encoder.finish()]);
            let recolored = crate::test_support::read_texture(&device, &queue, &target, 4);
            assert!(
                later != recolored,
                "{animation:?} must respond to palette changes"
            );
            let mut previous_palette = recolored;
            for preset in [
                settings::ColorPreset::Aurora,
                settings::ColorPreset::Ember,
                settings::ColorPreset::DeepOcean,
                settings::ColorPreset::RoseQuartz,
                settings::ColorPreset::Moonlight,
            ] {
                Arc::make_mut(&mut settings).color_mode = settings::ColorMode::Preset(preset);
                flux.update(&device, &queue, &settings);
                let mut encoder = device.create_command_encoder(&Default::default());
                flux.animate(&device, &queue, &mut encoder, &view, None, time);
                queue.submit([encoder.finish()]);
                let pixels = crate::test_support::read_texture(&device, &queue, &target, 4);
                assert!(
                    pixels != previous_palette,
                    "{animation:?} must display {preset:?}"
                );
                previous_palette = pixels;
            }
            Arc::make_mut(&mut settings).color_mode =
                settings::ColorMode::Preset(settings::ColorPreset::Poolside);
            flux.update(&device, &queue, &settings);
            let counts = resources.program_counts();
            for (width, height) in [(800, 1280), (3200, 800), (1280, 800)] {
                flux.resize(&device, &queue, width, height, 640, 400);
                time += 50.0;
                let mut encoder = device.create_command_encoder(&Default::default());
                flux.animate(&device, &queue, &mut encoder, &view, None, time);
                queue.submit([encoder.finish()]);
            }
            assert_eq!(
                counts,
                resources.program_counts(),
                "resize must reuse programs"
            );
            previous_mode = first;
        }
        Arc::make_mut(&mut settings).animation = settings::Animation::Drift;
        flux.update(&device, &queue, &settings);
        let mut encoder = device.create_command_encoder(&Default::default());
        flux.animate(&device, &queue, &mut encoder, &view, None, time + 50.0);
        queue.submit([encoder.finish()]);
        assert!(flux.artwork.is_none(), "Drift releases the extra renderer");
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert!(pollster::block_on(device.pop_error_scope()).is_none());
    }

    #[test]
    fn gpu_resize_palette_and_shared_programs() {
        let Some((device, queue)) = crate::test_support::gpu() else {
            return;
        };
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let resources = Arc::new(SharedResources::new(&device));
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut settings = Arc::new(Settings {
            seed: Some("resize-regression".into()),
            fluid_size: 16,
            grid_spacing: 32,
            color_mode: settings::ColorMode::Custom(settings::COLOR_SCHEME_PLASMA),
            ..Default::default()
        });
        let mut flux = Flux::new_with_resources(
            &device, &queue, format, 512, 512, 64, 64, &settings, &resources,
        )
        .unwrap();
        let counts = resources.program_counts();
        let mut other = Flux::new_with_resources(
            &device, &queue, format, 512, 512, 64, 64, &settings, &resources,
        )
        .unwrap();
        assert_eq!(
            counts,
            resources.program_counts(),
            "second simulation must reuse compiled programs"
        );
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("regression render target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let mut timestamp = 0.0;
        for (width, height, fluid_size, channel_count) in [
            (8192, 512, 16, 3),
            (512, 8192, 16, 1),
            (512, 8192, 32, 4),
            (512, 512, 16, 0),
        ] {
            Arc::make_mut(&mut settings).fluid_size = fluid_size;
            Arc::make_mut(&mut settings).noise_channels = vec![
                settings::Noise {
                    scale: 2.8,
                    multiplier: 1.0,
                    offset_increment: 0.01
                };
                channel_count
            ];
            flux.update(&device, &queue, &settings);
            flux.resize(&device, &queue, width, height, 64, 64);
            flux.lines.assert_color_mode(1);
            flux.fluid.assert_resource_sizes();
            flux.noise_generator
                .assert_resource_sizes(flux.fluid.get_fluid_size());
            assert_eq!(
                counts,
                resources.program_counts(),
                "resize must reuse compiled programs"
            );
            for mode in [
                settings::Mode::Normal,
                settings::Mode::DebugNoise,
                settings::Mode::DebugFluid,
                settings::Mode::DebugPressure,
                settings::Mode::DebugDivergence,
            ] {
                Arc::make_mut(&mut settings).mode = mode;
                flux.update(&device, &queue, &settings);
                timestamp += 50.0;
                let mut encoder = device.create_command_encoder(&Default::default());
                flux.animate(&device, &queue, &mut encoder, &view, None, timestamp);
                other.animate(&device, &queue, &mut encoder, &view, None, timestamp);
                queue.submit([encoder.finish()]);
            }
            flux.noise_generator
                .assert_generated(&device, &queue, channel_count != 0);
        }
        Arc::make_mut(&mut settings).grid_spacing = 64;
        flux.update(&device, &queue, &settings);
        assert_eq!(flux.grid.columns, 9, "density changes must resize the grid");
        let prior_size = flux.logical_size;
        flux.resize(&device, &queue, 0, 0, 0, 0);
        assert_eq!(
            flux.logical_size, prior_size,
            "zero-sized windows suspend resizing"
        );
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            panic!("GPU validation: {error}");
        }
    }

    #[test]
    fn gpu_batched_substeps_match_separate_submissions() {
        let Some((device, queue)) = crate::test_support::gpu() else {
            return;
        };
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let resources = Arc::new(SharedResources::new(&device));
        let settings = Arc::new(Settings {
            seed: Some("substep-regression".into()),
            fluid_size: 16,
            grid_spacing: 64,
            // Exact binary time increments keep accumulation identical.
            fluid_timestep: 1.0 / 64.0,
            ..Default::default()
        });
        let mut batched = Flux::new_with_resources(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            256,
            256,
            64,
            64,
            &settings,
            &resources,
        )
        .unwrap();
        let mut separate = Flux::new_with_resources(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            256,
            256,
            64,
            64,
            &settings,
            &resources,
        )
        .unwrap();
        let mut encoder = device.create_command_encoder(&Default::default());
        batched.compute(&device, &queue, &mut encoder, 46.875);
        queue.submit([encoder.finish()]);
        for timestamp in [15.625, 31.25, 46.875] {
            let mut encoder = device.create_command_encoder(&Default::default());
            separate.compute(&device, &queue, &mut encoder, timestamp);
            queue.submit([encoder.finish()]);
        }
        let batched = batched.fluid.velocity_bytes(&device, &queue);
        let separate = separate.fluid.velocity_bytes(&device, &queue);
        let values = |bytes: Vec<u8>| {
            bytes
                .chunks_exact(4)
                .map(|v| f32::from_ne_bytes(v.try_into().unwrap()))
                .collect::<Vec<_>>()
        };
        let batched = values(batched);
        let separate = values(separate);
        assert!(
            batched.iter().any(|value| value.abs() > 1e-6),
            "fluid must contain noise"
        );
        for (a, b) in batched.iter().zip(&separate) {
            assert!(
                (a - b).abs() < 1e-6,
                "batched simulation diverged: {a} vs {b}"
            );
        }
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            panic!("GPU validation: {error}");
        }
    }

    #[test]
    fn custom_palette_round_trips_and_selects_palette_shader() {
        let mode = settings::ColorMode::Custom(settings::COLOR_SCHEME_POOLSIDE);
        assert_eq!(u32::from(mode.clone()), 1);
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(
            serde_json::from_str::<settings::ColorMode>(&json).unwrap(),
            mode
        );
    }
}
