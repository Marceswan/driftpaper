// Hide the Windows console in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod displays;
mod palette;
mod platform;
mod power;
mod preferences;
mod renderer;

use clap::Parser;
use displays::DisplayInfo;
use palette::*;
use platform::*;
use power::wake;
use preferences::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use winit::window::{Window, WindowBuilder, WindowLevel};

// Global flag to signal quit from menu bar
static SHOULD_QUIT: AtomicBool = AtomicBool::new(false);

// Global settings for menu control
static CURRENT_COLOR_SCHEME: AtomicU32 = AtomicU32::new(0); // 0=Original, 1=Plasma, 2=Poolside, 3=SpaceGrey
static CURRENT_DENSITY: AtomicU32 = AtomicU32::new(1); // 0=Sparse, 1=Normal, 2=Dense
static CURRENT_NOISE_STRENGTH: AtomicU32 = AtomicU32::new(1); // 0=Low, 1=Medium, 2=High, 3=Max
static CURRENT_LINE_LENGTH: AtomicU32 = AtomicU32::new(1); // 0=Short, 1=Medium, 2=Long, 3=Extra Long
static CURRENT_LINE_WIDTH: AtomicU32 = AtomicU32::new(1); // 0=Thin, 1=Medium, 2=Thick
static CURRENT_VIEW_SCALE: AtomicU32 = AtomicU32::new(1); // 0=Compact, 1=Normal, 2=Wide
static CURRENT_BRIGHTNESS: AtomicU32 = AtomicU32::new(1); // 0=Dim, 1=Normal, 2=Bright, 3=Vivid
static SETTINGS_CHANGED: AtomicBool = AtomicBool::new(false);

// Global flag to signal screen configuration changed (resolution, refresh rate, display added/removed)
static SCREEN_CONFIG_CHANGED: AtomicBool = AtomicBool::new(false);

// Global storage for custom color wheel extracted from an image
// Written by menu handler thread, read by render/event loop thread
fn custom_color_wheel() -> &'static Mutex<Option<[f32; 24]>> {
    static INSTANCE: OnceLock<Mutex<Option<[f32; 24]>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(None))
}

static CURRENT_FPS: AtomicU32 = AtomicU32::new(30);

#[derive(Parser, Debug, Clone)]
#[command(
    name = "drift",
    about = "Drift - A live wallpaper inspired by macOS Drift"
)]
struct Args {
    /// Run in normal window mode instead of as wallpaper
    #[arg(long)]
    windowed: bool,

    /// Target FPS override (1-240); otherwise use the saved preference (default: 30)
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=240))]
    fps: Option<u32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();
    let mut builder = winit::event_loop::EventLoopBuilder::<power::UserEvent>::with_user_event();
    power::configure_event_loop(&mut builder);
    let event_loop = builder.build()?;
    power::install_proxy(event_loop.create_proxy());
    let prefs = load_preferences();
    CURRENT_FPS.store(args.fps.unwrap_or(prefs.fps), Ordering::SeqCst);
    CURRENT_COLOR_SCHEME.store(prefs.color_scheme, Ordering::SeqCst);
    CURRENT_DENSITY.store(prefs.density, Ordering::SeqCst);
    CURRENT_NOISE_STRENGTH.store(prefs.noise_strength, Ordering::SeqCst);
    CURRENT_LINE_LENGTH.store(prefs.line_length, Ordering::SeqCst);
    CURRENT_LINE_WIDTH.store(prefs.line_width, Ordering::SeqCst);
    CURRENT_VIEW_SCALE.store(prefs.view_scale, Ordering::SeqCst);
    CURRENT_BRIGHTNESS.store(prefs.brightness, Ordering::SeqCst);
    *custom_color_wheel()
        .lock()
        .unwrap_or_else(|err| err.into_inner()) = prefs.custom_color_wheel;
    #[cfg(target_os = "windows")]
    let _tray_icon = if !args.windowed {
        setup_menu_bar()
    } else {
        None
    };
    #[cfg(not(target_os = "windows"))]
    if !args.windowed {
        setup_menu_bar();
    }
    setup_screen_change_observer();
    power::observe();
    renderer::run(event_loop, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_fps_before_opening_windows() {
        for value in ["0", "241", "-1"] {
            assert!(Args::try_parse_from(["drift", "--fps", value]).is_err());
        }
        assert_eq!(
            Args::try_parse_from(["drift", "--fps", "60"]).unwrap().fps,
            Some(60)
        );
        assert_eq!(Args::try_parse_from(["drift"]).unwrap().fps, None);
    }
}
