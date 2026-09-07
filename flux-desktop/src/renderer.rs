use crate::*;
use flux::{Flux, Settings, SharedResources};
use power::UserEvent;
use std::time::{Duration, Instant};
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    keyboard::{KeyCode, PhysicalKey},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Gpu {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    resources: Arc<SharedResources>,
}

struct DisplayRenderer {
    // Surface owns an Arc to the window; no lifetime transmute is needed.
    surface: wgpu::Surface<'static>,
    window: Arc<Window>,
    gpu: Arc<Gpu>,
    config: wgpu::SurfaceConfiguration,
    flux: Flux,
    display: DisplayInfo,
    size: PhysicalSize<u32>,
    occluded: bool,
}

#[derive(Debug, PartialEq)]
enum SurfaceAction {
    Skip,
    Reconfigure,
    Recreate,
    Exit,
}
fn surface_action(error: &wgpu::SurfaceError) -> SurfaceAction {
    match error {
        wgpu::SurfaceError::Timeout => SurfaceAction::Skip,
        wgpu::SurfaceError::Outdated => SurfaceAction::Reconfigure,
        wgpu::SurfaceError::Lost => SurfaceAction::Recreate,
        wgpu::SurfaceError::OutOfMemory => SurfaceAction::Exit,
        wgpu::SurfaceError::Other => SurfaceAction::Recreate,
    }
}
fn valid_size(size: PhysicalSize<u32>) -> bool {
    size.width > 0 && size.height > 0
}

impl DisplayRenderer {
    fn resize(&mut self, size: PhysicalSize<u32>) {
        let previous = self.size;
        self.size = size;
        if !valid_size(size) {
            return;
        }
        let logical: LogicalSize<u32> = size.to_logical(self.window.scale_factor());
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.gpu.device, &self.config);
        // Flux itself handles unchanged grid sizes without recreating resources.
        self.flux.resize(
            &self.gpu.device,
            &self.gpu.queue,
            logical.width.max(1),
            logical.height.max(1),
            size.width,
            size.height,
        );
        if !valid_size(previous) {
            self.window.request_redraw();
        }
    }

    fn render(&mut self, instance: &wgpu::Instance, timestamp: f64) -> Result<()> {
        if !valid_size(self.size) {
            return Ok(());
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(error) => {
                match surface_action(&error) {
                    SurfaceAction::Skip => log::debug!("Skipping frame: {error}"),
                    SurfaceAction::Reconfigure => {
                        self.surface.configure(&self.gpu.device, &self.config)
                    }
                    SurfaceAction::Recreate => {
                        self.surface = instance.create_surface(self.window.clone())?;
                        self.surface.configure(&self.gpu.device, &self.config);
                    }
                    SurfaceAction::Exit => return Err(error.into()),
                }
                return Ok(());
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("drift:frame"),
            });
        self.flux.animate(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &view,
            None,
            timestamp,
        );
        self.gpu.queue.submit(Some(encoder.finish()));
        self.window.pre_present_notify();
        frame.present();
        Ok(())
    }
}

fn current_settings() -> Arc<Settings> {
    let mut settings = Settings {
        color_mode: scheme_to_color_mode(CURRENT_COLOR_SCHEME.load(Ordering::SeqCst)),
        grid_spacing: density_to_grid_spacing(CURRENT_DENSITY.load(Ordering::SeqCst)),
        noise_multiplier: noise_strength_to_multiplier(
            CURRENT_NOISE_STRENGTH.load(Ordering::SeqCst),
        ),
        line_length: line_length_to_value(CURRENT_LINE_LENGTH.load(Ordering::SeqCst)),
        line_width: line_width_to_value(CURRENT_LINE_WIDTH.load(Ordering::SeqCst)),
        view_scale: view_scale_to_value(CURRENT_VIEW_SCALE.load(Ordering::SeqCst)),
        brightness_multiplier: brightness_to_multiplier(CURRENT_BRIGHTNESS.load(Ordering::SeqCst)),
        ..Settings::default()
    };
    if CURRENT_COLOR_SCHEME.load(Ordering::SeqCst) == 4 {
        if let Some(wheel) = *custom_color_wheel()
            .lock()
            .unwrap_or_else(|err| err.into_inner())
        {
            settings.color_mode = flux::settings::ColorMode::Custom(wheel);
        }
    }
    Arc::new(settings)
}

async fn create_renderer(
    target: &EventLoopWindowTarget<UserEvent>,
    instance: &wgpu::Instance,
    gpus: &mut Vec<Arc<Gpu>>,
    display: DisplayInfo,
    wallpaper: bool,
    settings: &Arc<Settings>,
) -> Result<DisplayRenderer> {
    let mut builder = WindowBuilder::new()
        .with_title("DriftPaper")
        .with_visible(false)
        .with_decorations(!wallpaper)
        .with_resizable(!wallpaper)
        .with_active(!wallpaper);
    if wallpaper {
        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowBuilderExtMacOS;
            builder = builder.with_accepts_first_mouse(false);
        }
        #[cfg(target_os = "macos")]
        {
            builder = builder.with_inner_size(LogicalSize::new(display.width, display.height));
        }
        #[cfg(not(target_os = "macos"))]
        {
            builder = builder
                .with_inner_size(PhysicalSize::new(display.pixels_wide, display.pixels_high));
        }
        builder = builder.with_window_level(WindowLevel::AlwaysOnBottom);
        // macOS positions are Cocoa coordinates and are set by setup_wallpaper_window.
        #[cfg(not(target_os = "macos"))]
        {
            builder = builder.with_position(winit::dpi::PhysicalPosition::new(
                display.origin_x as i32,
                display.origin_y as i32,
            ));
        }
    } else {
        builder = builder.with_inner_size(LogicalSize::new(1280, 800));
    }
    let window = Arc::new(builder.build(target)?);
    if wallpaper {
        setup_wallpaper_window(&window, &display);
    }
    let surface = instance.create_surface(window.clone())?;
    let gpu = if let Some(gpu) = gpus
        .iter()
        .find(|gpu| gpu.adapter.is_surface_supported(&surface))
    {
        gpu.clone()
    } else {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await?;
        let mut limits = wgpu::Limits::default().using_resolution(adapter.limits());
        limits.max_push_constant_size = 8;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("drift:shared-device"),
                required_features: wgpu::Features::PUSH_CONSTANTS
                    | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                    | wgpu::Features::FLOAT32_FILTERABLE,
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await?;
        let resources = Arc::new(SharedResources::new(&device));
        let gpu = Arc::new(Gpu {
            adapter,
            device,
            queue,
            resources,
        });
        log::info!("Created shared GPU: {}", gpu.adapter.get_info().name);
        gpus.push(gpu.clone());
        gpu
    };
    let caps = surface.get_capabilities(&gpu.adapter);
    let size = window.inner_size();
    let format = preferred_format(&caps).ok_or("Display has no supported surface formats")?;
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: *caps
            .alpha_modes
            .first()
            .ok_or("Display has no alpha modes")?,
        view_formats: vec![],
    };
    if valid_size(size) {
        surface.configure(&gpu.device, &config);
    }
    let logical: LogicalSize<u32> = size.to_logical(window.scale_factor());
    let flux = Flux::new_with_resources(
        &gpu.device,
        &gpu.queue,
        format,
        logical.width.max(1),
        logical.height.max(1),
        size.width.max(1),
        size.height.max(1),
        settings,
        &gpu.resources,
    )?;
    window.set_visible(true);
    if wallpaper {
        setup_wallpaper_window(&window, &display);
    }
    Ok(DisplayRenderer {
        surface,
        window,
        gpu,
        config,
        flux,
        display,
        size,
        occluded: false,
    })
}

fn reconcile(
    target: &EventLoopWindowTarget<UserEvent>,
    instance: &wgpu::Instance,
    renderers: &mut Vec<DisplayRenderer>,
    gpus: &mut Vec<Arc<Gpu>>,
    settings: &Arc<Settings>,
) {
    let displays = get_all_displays();
    let current: Vec<_> = renderers.iter().map(|r| r.display.clone()).collect();
    let removed = displays::removed_displays(&current, &displays);
    renderers.retain(|renderer| !removed.contains(&renderer.display.id.as_str()));
    for display in displays {
        if let Some(renderer) = renderers
            .iter_mut()
            .find(|renderer| renderer.display.id == display.id)
        {
            if renderer.display != display {
                setup_wallpaper_window(&renderer.window, &display);
                renderer.display = display;
                renderer.resize(renderer.window.inner_size());
            }
        } else {
            match pollster::block_on(create_renderer(
                target, instance, gpus, display, true, settings,
            )) {
                Ok(renderer) => renderers.push(renderer),
                Err(err) => log::error!("Cannot initialize display: {err}"),
            }
        }
    }
    gpus.retain(|gpu| Arc::strong_count(gpu) > 1);
}

pub(crate) fn run(event_loop: EventLoop<UserEvent>, args: Args) -> Result<()> {
    let instance = wgpu::Instance::default();
    // Keep notification delivery alive even when every monitor is disconnected.
    #[cfg(target_os = "windows")]
    let power_window = {
        use winit::platform::windows::WindowBuilderExtWindows;
        let window = WindowBuilder::new()
            .with_title("DriftPaper notifications")
            .with_visible(false)
            .with_active(false)
            .with_skip_taskbar(true)
            .build(&event_loop)?;
        power::observe_window(&window);
        window
    };
    let wallpaper = !args.windowed;
    let mut settings = current_settings();
    let mut gpus = Vec::new();
    let mut renderers = Vec::new();
    if wallpaper {
        reconcile(&event_loop, &instance, &mut renderers, &mut gpus, &settings);
    } else {
        let display = DisplayInfo {
            id: "preview".into(),
            origin_x: 0.0,
            origin_y: 0.0,
            width: 1280.0,
            height: 800.0,
            pixels_wide: 1280,
            pixels_high: 800,
        };
        renderers.push(pollster::block_on(create_renderer(
            &event_loop,
            &instance,
            &mut gpus,
            display,
            false,
            &settings,
        ))?);
    }
    let mut clock = FrameClock::new(Instant::now());
    let mut suspended = false;
    let mut reconcile_at = Instant::now() + Duration::from_secs(2);
    let (image_tx, image_rx) = std::sync::mpsc::channel();
    event_loop.run(move |event, target| {
        #[cfg(target_os = "windows")]
        let _observer_lifetime = &power_window;
        if SHOULD_QUIT.load(Ordering::SeqCst) { target.exit(); return; }
        platform::sync_menu_state();
        if SETTINGS_CHANGED.swap(false, Ordering::SeqCst) {
            settings = current_settings();
            for renderer in &mut renderers {
                renderer.flux.update(&renderer.gpu.device, &renderer.gpu.queue, &settings);
            }
        }
        while let Ok(image) = image_rx.try_recv() {
            for renderer in &mut renderers { renderer.flux.sample_colors_from_image(&renderer.gpu.device, &renderer.gpu.queue, &image); }
        }
        let now = Instant::now();
        let changed = SCREEN_CONFIG_CHANGED.swap(false, Ordering::SeqCst);
        if wallpaper && (changed || now >= reconcile_at) {
            reconcile(target, &instance, &mut renderers, &mut gpus, &settings);
            reconcile_at = now + Duration::from_secs(2);
        }
        match event {
            Event::Suspended => suspended = true,
            Event::Resumed => { suspended = false; SCREEN_CONFIG_CHANGED.store(true, Ordering::SeqCst); }
            Event::WindowEvent { window_id, event } => {
                if let Some(renderer) = renderers.iter_mut().find(|r| r.window.id() == window_id) {
                    match event {
                        WindowEvent::CloseRequested => target.exit(),
                        WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(KeyCode::Escape | KeyCode::KeyQ), state: ElementState::Released, .. }, .. } if !wallpaper => target.exit(),
                        WindowEvent::Resized(size) => renderer.resize(size),
                        WindowEvent::ScaleFactorChanged { .. } => renderer.resize(renderer.window.inner_size()),
                        WindowEvent::Occluded(value) => renderer.occluded = value,
                        WindowEvent::DroppedFile(path) if !wallpaper => {
                            let tx = image_tx.clone();
                            std::thread::spawn(move || {
                                match image::open(path).map(|image| image.to_rgba8()) {
                                    Ok(image) => { let _ = tx.send(image); power::wake(); }
                                    Err(err) => log::error!("Cannot decode dropped image: {err}"),
                                }
                            });
                        }
                        WindowEvent::RedrawRequested if !suspended && !power::paused() && (wallpaper || !renderer.occluded) => {
                            if let Err(err) = renderer.render(&instance, clock.timestamp_ms()) {
                                log::error!("Rendering stopped: {err}"); target.exit();
                            }
                        }
                        _ => (),
                    }
                }
            }
            Event::AboutToWait => {
                let active = !suspended && !power::paused() && renderers.iter().any(|r| valid_size(r.size) && (wallpaper || !r.occluded));
                let fps = CURRENT_FPS.load(Ordering::SeqCst).clamp(1, 240);
                if clock.tick(now, fps, active) {
                    for renderer in &renderers {
                        if valid_size(renderer.size) && (wallpaper || !renderer.occluded) { renderer.window.request_redraw(); }
                    }
                }
                // Low-frequency display reconciliation also retries failed initialization.
                // Sleep/lock uses true Wait; native events wake the loop again.
                target.set_control_flow(if suspended || power::paused() { ControlFlow::Wait }
                    else if active { ControlFlow::WaitUntil(if wallpaper { clock.deadline(fps).min(reconcile_at) } else { clock.deadline(fps) }) }
                    else if wallpaper { ControlFlow::WaitUntil(reconcile_at) }
                    else { ControlFlow::Wait });
            }
            _ => (),
        }
    })?;
    Ok(())
}

struct FrameClock {
    last: Instant,
    elapsed: Duration,
    active: bool,
}
impl FrameClock {
    fn new(now: Instant) -> Self {
        Self {
            last: now,
            elapsed: Duration::ZERO,
            active: false,
        }
    }
    fn tick(&mut self, now: Instant, fps: u32, active: bool) -> bool {
        if !active || !self.active {
            self.last = now;
            self.active = active;
            return active;
        }
        let delta = now.duration_since(self.last);
        if delta < frame_duration(fps) {
            return false;
        }
        self.elapsed += delta.min(Duration::from_millis(100));
        self.last = now;
        true
    }
    fn deadline(&self, fps: u32) -> Instant {
        self.last + frame_duration(fps)
    }
    fn timestamp_ms(&self) -> f64 {
        self.elapsed.as_secs_f64() * 1000.0
    }
}
fn frame_duration(fps: u32) -> Duration {
    Duration::from_secs_f64(1.0 / fps.clamp(1, 240) as f64)
}
fn preferred_format(caps: &wgpu::SurfaceCapabilities) -> Option<wgpu::TextureFormat> {
    [
        wgpu::TextureFormat::Rgb10a2Unorm,
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ]
    .into_iter()
    .find(|format| caps.formats.contains(format))
    .or_else(|| caps.formats.first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recoverable_surface_errors_do_not_exit() {
        assert_eq!(
            surface_action(&wgpu::SurfaceError::Timeout),
            SurfaceAction::Skip
        );
        assert_eq!(
            surface_action(&wgpu::SurfaceError::Outdated),
            SurfaceAction::Reconfigure
        );
        assert_eq!(
            surface_action(&wgpu::SurfaceError::Lost),
            SurfaceAction::Recreate
        );
        assert_eq!(
            surface_action(&wgpu::SurfaceError::OutOfMemory),
            SurfaceAction::Exit
        );
    }
    #[test]
    fn zero_sized_windows_suspend_rendering() {
        assert!(!valid_size(PhysicalSize::new(0, 1080)));
        assert!(!valid_size(PhysicalSize::new(1920, 0)));
        assert!(valid_size(PhysicalSize::new(1920, 1080)));
    }
    #[test]
    fn frame_clock_respects_fps_and_excludes_sleep_time() {
        let start = Instant::now();
        let mut clock = FrameClock::new(start);
        assert!(clock.tick(start, 30, true));
        assert!(!clock.tick(start + Duration::from_millis(16), 30, true));
        assert!(clock.tick(start + Duration::from_millis(34), 30, true));
        clock.tick(start + Duration::from_secs(1), 30, false);
        clock.tick(start + Duration::from_secs(3600), 30, true);
        assert_eq!(clock.elapsed, Duration::from_millis(34));
    }
}
