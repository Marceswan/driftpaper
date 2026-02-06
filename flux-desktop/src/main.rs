// Disable the console window that pops up when you launch the .exe
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use image::RgbaImage;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};

use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowBuilder, WindowLevel},
};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowBuilderExtMacOS;

#[cfg(target_os = "windows")]
use winit::platform::windows::WindowBuilderExtWindows;

use flux::{Flux, Settings};

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

/// Persistent user preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct UserPreferences {
    color_scheme: u32,
    density: u32,
    noise_strength: u32,
    line_length: u32,
    line_width: u32,
    view_scale: u32,
    brightness: u32,
    fps: u32,
    #[serde(default)]
    run_on_login: bool,
    #[serde(default)]
    custom_color_wheel: Option<[f32; 24]>,
    #[serde(default)]
    custom_image_path: Option<String>,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            color_scheme: 0,
            density: 1,
            noise_strength: 1, // Medium
            line_length: 1,    // Medium
            line_width: 1,     // Medium
            view_scale: 1,     // Normal
            brightness: 1,     // Normal
            fps: 30,
            run_on_login: false,
            custom_color_wheel: None,
            custom_image_path: None,
        }
    }
}

fn get_preferences_path() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        std::path::PathBuf::from(format!("{}/.config/driftpaper/preferences.json", home))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(format!("{}\\DriftPaper\\preferences.json", appdata))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        std::path::PathBuf::from(format!("{}/.config/driftpaper/preferences.json", home))
    }
}

fn load_preferences() -> UserPreferences {
    let path = get_preferences_path();
    if let Ok(contents) = std::fs::read_to_string(&path) {
        serde_json::from_str(&contents).unwrap_or_default()
    } else {
        UserPreferences::default()
    }
}

fn save_preferences(prefs: &UserPreferences) {
    let path = get_preferences_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(prefs) {
        let _ = std::fs::write(path, json);
    }
}

/// Convert density setting to grid_spacing value
/// Larger values = fewer lines = less memory usage
fn density_to_grid_spacing(density: u32) -> u32 {
    match density {
        0 => 25, // Sparse - fewer stems, lowest memory
        1 => 15, // Normal - balanced
        2 => 10, // Dense - more stems
        _ => 15,
    }
}

/// Get color preset from scheme index
fn scheme_to_color_mode(scheme: u32) -> flux::settings::ColorMode {
    use flux::settings::{ColorMode, ColorPreset};
    match scheme {
        0 => ColorMode::Preset(ColorPreset::Original),
        1 => ColorMode::Preset(ColorPreset::Plasma),
        2 => ColorMode::Preset(ColorPreset::Poolside),
        3 => ColorMode::Preset(ColorPreset::SpaceGrey),
        // 4 = Custom Image - use Original as placeholder; actual custom wheel is injected separately
        4 => ColorMode::Preset(ColorPreset::Original),
        _ => ColorMode::Preset(ColorPreset::Original),
    }
}

/// Convert HSL values to RGB floats (0.0-1.0)
fn hsl_to_rgb_f32(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s == 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue_to_rgb = |p: f32, q: f32, mut t: f32| -> f32 {
        if t < 0.0 { t += 1.0; }
        if t > 1.0 { t -= 1.0; }
        if t < 1.0 / 6.0 { return p + (q - p) * 6.0 * t; }
        if t < 1.0 / 2.0 { return q; }
        if t < 2.0 / 3.0 { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
        p
    };
    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (r, g, b)
}

/// Extract 6 dominant colors from an image and return as [f32; 24] color wheel
fn extract_colors_from_image(path: &Path) -> Result<[f32; 24], String> {
    let img = image::open(path).map_err(|e| format!("Failed to open image: {}", e))?;

    // Downscale to max 200x200 for fast processing
    let thumb = img.thumbnail(200, 200);
    let rgb = thumb.to_rgb8();

    // Bin pixels into 12 hue buckets (30 degrees each)
    struct HueBucket {
        h_sum: f64,
        s_sum: f64,
        l_sum: f64,
        count: u64,
    }
    let mut buckets: Vec<HueBucket> = (0..12)
        .map(|_| HueBucket { h_sum: 0.0, s_sum: 0.0, l_sum: 0.0, count: 0 })
        .collect();

    for pixel in rgb.pixels() {
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let l = (max + min) / 2.0;

        // Filter very dark, very light, and near-grey pixels
        if l < 0.08 || l > 0.92 || delta < 0.02 {
            continue;
        }

        let s = if l < 0.5 {
            delta / (max + min)
        } else {
            delta / (2.0 - max - min)
        };

        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };
        let h = if h < 0.0 { h + 360.0 } else { h };

        let bucket_idx = ((h / 30.0) as usize).min(11);
        buckets[bucket_idx].h_sum += h as f64;
        buckets[bucket_idx].s_sum += s as f64;
        buckets[bucket_idx].l_sum += l as f64;
        buckets[bucket_idx].count += 1;
    }

    // Collect non-empty buckets with averages
    let mut candidates: Vec<(f32, f32, f32, u64)> = buckets.iter()
        .filter(|b| b.count > 0)
        .map(|b| {
            let n = b.count as f64;
            ((b.h_sum / n) as f32, (b.s_sum / n) as f32, (b.l_sum / n) as f32, b.count)
        })
        .collect();

    // Sort by count descending, take top 6
    candidates.sort_by(|a, b| b.3.cmp(&a.3));

    // Handle monochrome edge case: if fewer than 6 buckets, spread lightness
    if candidates.len() < 6 {
        if candidates.is_empty() {
            // Completely monochrome or featureless - generate a neutral spread
            candidates = (0..6)
                .map(|i| (0.0, 0.0, 0.2 + (i as f32) * 0.12))
                .map(|c| (c.0, c.1, c.2, 1))
                .collect();
        } else {
            // Duplicate and vary lightness
            let base = candidates.clone();
            while candidates.len() < 6 {
                let src = &base[candidates.len() % base.len()];
                let offset = (candidates.len() as f32) * 0.08;
                let new_l = (src.2 + offset).min(0.85);
                candidates.push((src.0, src.1, new_l, 1));
            }
        }
    }
    candidates.truncate(6);

    // Sort by hue for smooth shader interpolation
    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Convert HSL -> RGB, pack into [f32; 24]
    let mut wheel = [0.0f32; 24];
    for (i, (h, s, l, _)) in candidates.iter().enumerate() {
        let (r, g, b) = hsl_to_rgb_f32(*h / 360.0, *s, *l);
        wheel[i * 4] = r;
        wheel[i * 4 + 1] = g;
        wheel[i * 4 + 2] = b;
        wheel[i * 4 + 3] = 1.0;
    }

    log::info!("Extracted {} colors from image: {:?}", candidates.len(), path);
    Ok(wheel)
}

/// Convert noise strength setting to noise_multiplier value
fn noise_strength_to_multiplier(strength: u32) -> f32 {
    match strength {
        0 => 0.15,  // Low
        1 => 0.45,  // Medium (default)
        2 => 0.75,  // High
        3 => 1.0,   // Max
        _ => 0.45,
    }
}

/// Convert line length setting to line_length value
fn line_length_to_value(length: u32) -> f32 {
    match length {
        0 => 63.0,    // Short
        1 => 142.0,   // Medium
        2 => 220.0,   // Long
        3 => 315.0,   // Extra Long
        _ => 142.0,
    }
}

/// Convert line width setting to line_width value
fn line_width_to_value(width: u32) -> f32 {
    match width {
        0 => 4.0,   // Thin
        1 => 9.0,   // Medium (default)
        2 => 16.0,  // Thick
        _ => 9.0,
    }
}

/// Convert view scale setting to view_scale value
fn view_scale_to_value(scale: u32) -> f32 {
    match scale {
        0 => 1.0,   // Compact
        1 => 1.6,   // Normal (default)
        2 => 2.2,   // Wide
        _ => 1.6,
    }
}

/// Convert brightness setting to multiplier value
fn brightness_to_multiplier(brightness: u32) -> f32 {
    match brightness {
        0 => 0.5,   // Dim
        1 => 1.0,   // Normal (default)
        2 => 2.0,   // Bright
        3 => 3.5,   // Vivid
        _ => 1.0,
    }
}

#[derive(Parser, Debug, Clone)]
#[command(name = "drift", about = "Drift - A live wallpaper inspired by macOS Drift")]
struct Args {
    /// Run in normal window mode instead of as wallpaper
    #[arg(long)]
    windowed: bool,

    /// Target frames per second (lower = less CPU/GPU, default: 60)
    #[arg(long, default_value = "60")]
    fps: u32,
}

struct App {
    runtime: tokio::runtime::Runtime,
    tx: mpsc::Sender<Msg>,
    rx: mpsc::Receiver<Msg>,

    flux: Flux,
    #[allow(dead_code)]
    settings: Arc<Settings>,

    color_image: Arc<Mutex<Option<RgbaImage>>>,
}

enum Msg {
    DecodedImage,
}

impl App {
    fn handle_pending_messages(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::DecodedImage => {
                    if let Some(image) = &*self.color_image.lock().unwrap() {
                        self.flux.sample_colors_from_image(device, queue, image);
                    }
                }
            }
        }
    }

    pub fn decode_image(&self, encoded_bytes: Vec<u8>) {
        let tx = self.tx.clone();
        let color_image = Arc::clone(&self.color_image);
        self.runtime.spawn(async move {
            match flux::render::color::Context::decode_color_texture(&encoded_bytes) {
                Ok(image) => {
                    {
                        let mut boop = color_image.lock().unwrap();
                        *boop = Some(image);
                    }
                    if tx.send(Msg::DecodedImage).await.is_err() {
                        log::error!("Failed to send decoded image message");
                    }
                }
                Err(err) => log::error!("{}", err),
            }
        });
        log::debug!("Spawned image decoding task");
    }
}

/// Display info for wallpaper mode
#[derive(Debug, Clone)]
struct DisplayInfo {
    origin_x: f64,
    origin_y: f64,
    width: f64,
    height: f64,
    // Physical pixel dimensions (for wgpu surface)
    pixels_wide: u32,
    pixels_high: u32,
}

#[cfg(target_os = "macos")]
fn setup_wallpaper_window(window: &Window, display: &DisplayInfo) {
    use cocoa::appkit::{NSWindow, NSWindowCollectionBehavior, NSView};
    use cocoa::base::{id, nil, NO, YES};
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{msg_send, sel, sel_impl, class};
    use objc::runtime::{Object, Sel, Class, Method};
    use objc::declare::ClassDecl;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // Custom hitTest: that always returns nil - makes view transparent to clicks
    extern "C" fn hit_test_nil(_this: &Object, _cmd: Sel, _point: cocoa::foundation::NSPoint) -> id {
        std::ptr::null_mut()
    }

    // Install the swizzled hitTest method on the view's class
    unsafe fn swizzle_hit_test(view: id) {
        let view_class: *const Class = msg_send![view, class];
        if view_class.is_null() {
            return;
        }

        // Add our custom hitTest: method that returns nil
        let hit_test_sel = sel!(hitTest:);
        let imp: objc::runtime::Imp = std::mem::transmute(
            hit_test_nil as extern "C" fn(&Object, Sel, cocoa::foundation::NSPoint) -> id
        );

        // Try to add the method first
        let method_added: bool = objc::runtime::class_addMethod(
            view_class as *mut Class,
            hit_test_sel,
            imp,
            b"@@:{NSPoint=dd}\0".as_ptr() as *const i8,
        ) as i8 != 0;

        if method_added {
            log::info!("Successfully added custom hitTest: to view class");
        } else {
            // Method already exists, replace the implementation
            let method = objc::runtime::class_getInstanceMethod(view_class as *const Class, hit_test_sel);
            if !method.is_null() {
                objc::runtime::method_setImplementation(method as *mut objc::runtime::Method, imp);
                log::info!("Replaced existing hitTest: implementation");
            }
        }
    }

    let handle = window.window_handle().unwrap();
    if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
        let ns_view: id = appkit_handle.ns_view.as_ptr() as id;

        unsafe {
            let ns_window: id = msg_send![ns_view, window];

            // FIRST: Set borderless style mask (before setting frame)
            let _: () = msg_send![ns_window, setStyleMask: 0u64];

            // Window appearance
            let _: () = msg_send![ns_window, setHasShadow: NO];
            let _: () = msg_send![ns_window, setOpaque: NO];
            let _: () = msg_send![ns_window, setBackgroundColor: cocoa::appkit::NSColor::clearColor(std::ptr::null_mut())];

            // Desktop window level - same as wallpaper
            // kCGDesktopWindowLevelKey = -2147483623
            let _: () = msg_send![ns_window, setLevel: -2147483623i64];

            // Try making the window non-activating - this can help with click-through
            let _: () = msg_send![ns_window, setHidesOnDeactivate: NO];
            let _: () = msg_send![ns_window, setReleasedWhenClosed: NO];

            // Prevent the window from ever becoming key or main
            // Use a slightly transparent alpha to trigger click-through behavior
            let _: () = msg_send![ns_window, setAlphaValue: 0.99f64];

            // Appear on all spaces
            let behavior = NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle;
            ns_window.setCollectionBehavior_(behavior);

            // Click-through - ensure all mouse events pass to desktop
            // This must be set for the window to allow clicks through to Finder/desktop
            let _: () = msg_send![ns_window, setIgnoresMouseEvents: YES];
            let _: () = msg_send![ns_window, setAcceptsMouseMovedEvents: NO];
            let _: () = msg_send![ns_window, setExcludedFromWindowsMenu: YES];

            // Verify ignoresMouseEvents was set
            let ignores: cocoa::base::BOOL = msg_send![ns_window, ignoresMouseEvents];
            log::info!("Window ignoresMouseEvents: {}", ignores != NO);

            // Also set mouse event ignoring on the content view and ns_view
            // This ensures the entire view hierarchy passes events through
            let content_view: id = msg_send![ns_window, contentView];
            if content_view != std::ptr::null_mut() {
                // NSView doesn't have setIgnoresMouseEvents, but we can disable hit testing
                // by making the view not accept first responder
                let _: () = msg_send![content_view, setAcceptsTouchEvents: NO];
            }

            // Make the ns_view (Metal view) also not accept touch/mouse
            let _: () = msg_send![ns_view, setAcceptsTouchEvents: NO];

            // Swizzle hitTest: to always return nil - makes view completely transparent to clicks
            swizzle_hit_test(ns_view);

            // Also swizzle the content view if different
            let content_view_for_swizzle: id = msg_send![ns_window, contentView];
            if content_view_for_swizzle != ns_view && content_view_for_swizzle != std::ptr::null_mut() {
                swizzle_hit_test(content_view_for_swizzle);
            }

            // Resign key/main window status if somehow acquired
            let _: () = msg_send![ns_window, resignKeyWindow];
            let _: () = msg_send![ns_window, resignMainWindow];

            // Send window to back of its level - important for proper z-order with Finder desktop
            let _: () = msg_send![ns_window, orderBack: std::ptr::null::<objc::runtime::Object>()];

            // Set exact frame for this display
            let frame_rect = NSRect::new(
                NSPoint::new(display.origin_x, display.origin_y),
                NSSize::new(display.width, display.height),
            );
            let _: () = msg_send![ns_window, setFrame: frame_rect display: YES];

            // Ensure the content view fills the entire window
            let content_view: id = msg_send![ns_window, contentView];
            if content_view != std::ptr::null_mut() {
                // Set autoresizing mask to fill the window
                let autoresizing_mask: u64 = 0x12; // NSViewWidthSizable | NSViewHeightSizable
                let _: () = msg_send![content_view, setAutoresizingMask: autoresizing_mask];

                // Set the content view frame to match window bounds
                let bounds: NSRect = msg_send![ns_window, frame];
                let content_rect = NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(bounds.size.width, bounds.size.height),
                );
                let _: () = msg_send![content_view, setFrame: content_rect];

                // Also ensure the Metal/wgpu layer fills the view
                let layer: id = msg_send![ns_view, layer];
                if layer != std::ptr::null_mut() {
                    let _: () = msg_send![layer, setFrame: content_rect];
                }

                // Force layout
                let _: () = msg_send![content_view, setNeedsLayout: YES];
                let _: () = msg_send![ns_view, setNeedsLayout: YES];
            }

            // Verify the frame was set correctly
            let actual_frame: NSRect = msg_send![ns_window, frame];
            let view_frame: NSRect = msg_send![ns_view, frame];
            let view_bounds: NSRect = msg_send![ns_view, bounds];
            let content_view: id = msg_send![ns_window, contentView];
            let content_frame: NSRect = msg_send![content_view, frame];
            let superview: id = msg_send![ns_view, superview];

            log::info!(
                "Wallpaper window debug: window_frame=({}, {}, {}x{})",
                actual_frame.origin.x, actual_frame.origin.y,
                actual_frame.size.width, actual_frame.size.height
            );
            log::info!(
                "  content_view frame=({}, {}, {}x{})",
                content_frame.origin.x, content_frame.origin.y,
                content_frame.size.width, content_frame.size.height
            );
            log::info!(
                "  ns_view frame=({}, {}, {}x{}), bounds=({}, {}, {}x{})",
                view_frame.origin.x, view_frame.origin.y,
                view_frame.size.width, view_frame.size.height,
                view_bounds.origin.x, view_bounds.origin.y,
                view_bounds.size.width, view_bounds.size.height
            );

            // Check if ns_view is the content view or a subview
            let is_content_view = ns_view == content_view;
            log::info!("  ns_view is content_view: {}, has superview: {}",
                is_content_view, superview != std::ptr::null_mut());

            // If ns_view is not the content view, resize it to fill
            if !is_content_view && superview != std::ptr::null_mut() {
                let superview_bounds: NSRect = msg_send![superview, bounds];
                log::info!("  superview bounds: {}x{}", superview_bounds.size.width, superview_bounds.size.height);

                // Set ns_view to fill its superview
                let fill_frame = NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(superview_bounds.size.width, superview_bounds.size.height),
                );
                let _: () = msg_send![ns_view, setFrame: fill_frame];

                // Verify
                let new_view_frame: NSRect = msg_send![ns_view, frame];
                log::info!("  ns_view resized to: {}x{}", new_view_frame.size.width, new_view_frame.size.height);
            }
        }
    }

    log::info!(
        "Wallpaper window configured: origin=({}, {}), size={}x{}",
        display.origin_x, display.origin_y, display.width, display.height
    );
}

#[cfg(target_os = "macos")]
fn get_all_displays() -> Vec<DisplayInfo> {
    use cocoa::appkit::NSScreen;
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSArray;
    use objc::{msg_send, sel, sel_impl};
    use cocoa::foundation::NSRect;

    let mut displays = Vec::new();

    unsafe {
        // Use NSScreen instead of CGDisplay for accurate coordinates
        let screens: id = NSScreen::screens(nil);
        let count: u64 = msg_send![screens, count];

        for i in 0..count {
            let screen: id = msg_send![screens, objectAtIndex: i];
            let frame: NSRect = msg_send![screen, frame];
            let visible_frame: NSRect = msg_send![screen, visibleFrame];

            // Get backing scale factor for Retina detection
            let scale: f64 = msg_send![screen, backingScaleFactor];

            // Calculate physical pixels
            let pixels_wide = (frame.size.width * scale) as u32;
            let pixels_high = (frame.size.height * scale) as u32;

            log::info!(
                "NSScreen {}: frame=({}, {}, {}x{}), visible=({}, {}, {}x{}), scale={}, pixels={}x{}",
                i,
                frame.origin.x, frame.origin.y,
                frame.size.width, frame.size.height,
                visible_frame.origin.x, visible_frame.origin.y,
                visible_frame.size.width, visible_frame.size.height,
                scale,
                pixels_wide, pixels_high
            );

            displays.push(DisplayInfo {
                origin_x: frame.origin.x,
                origin_y: frame.origin.y,
                width: frame.size.width,
                height: frame.size.height,
                pixels_wide,
                pixels_high,
            });
        }
    }

    // Fallback if no screens found
    if displays.is_empty() {
        use core_graphics::display::CGDisplay;
        let display = CGDisplay::main();
        let bounds = display.bounds();
        displays.push(DisplayInfo {
            origin_x: bounds.origin.x,
            origin_y: bounds.origin.y,
            width: bounds.size.width,
            height: bounds.size.height,
            pixels_wide: display.pixels_wide() as u32,
            pixels_high: display.pixels_high() as u32,
        });
    }

    displays
}

// ==================== Windows Implementation ====================

#[cfg(target_os = "windows")]
mod windows_wallpaper {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE, FALSE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowA, FindWindowExA, GetWindowLongPtrA, GetWindowRect,
        SendMessageTimeoutA, SetParent, SetWindowLongPtrA, SetWindowPos, ShowWindow,
        SetLayeredWindowAttributes, GWL_EXSTYLE, GWL_STYLE, LWA_ALPHA,
        SWP_NOACTIVATE, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE,
        SMTO_NORMAL, SW_SHOWNA, WS_CHILD, WS_DISABLED, WS_POPUP, WS_EX_LAYERED,
    };
    use std::ptr::null;

    // Store WorkerW if found
    static mut WORKERW: HWND = 0 as HWND;

    unsafe extern "system" fn enum_windows_proc(hwnd: HWND, _lparam: LPARAM) -> BOOL {
        // Check if this window contains SHELLDLL_DefView (the desktop icons container)
        let shell_view = FindWindowExA(hwnd, 0 as HWND, b"SHELLDLL_DefView\0".as_ptr(), null());
        if shell_view != 0 as HWND {
            log::info!("Found SHELLDLL_DefView inside window {:?}", hwnd);

            // Now find the WorkerW window that's a sibling AFTER this one
            let worker = FindWindowExA(0 as HWND, hwnd, b"WorkerW\0".as_ptr(), null());
            if worker != 0 as HWND {
                WORKERW = worker;
                log::info!("Found WorkerW {:?} (sibling after SHELLDLL_DefView parent)", worker);
                return FALSE; // Stop enumeration
            }
        }
        TRUE // Continue enumeration
    }

    pub unsafe fn setup_as_wallpaper(hwnd: HWND, _x: i32, _y: i32, width: i32, height: i32) {
        // Find Progman window
        let progman = FindWindowA(b"Progman\0".as_ptr(), null());
        if progman == 0 as HWND {
            log::error!("Failed to find Progman window");
            return;
        }
        log::info!("Found Progman window: {:?}", progman);

        // Send message to spawn/prepare WorkerW
        let mut result: usize = 0;
        SendMessageTimeoutA(progman, 0x052C, 0xD, 0x1, SMTO_NORMAL, 1000, &mut result as *mut usize);
        log::info!("SendMessage 0x052C result: {}", result);

        // Small delay to let Windows process the message
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Find SHELLDLL_DefView and WorkerW
        let shell_view = FindWindowExA(progman, 0 as HWND, b"SHELLDLL_DefView\0".as_ptr(), null());
        WORKERW = 0 as HWND;
        EnumWindows(Some(enum_windows_proc), 0);

        // Get Progman dimensions
        let mut progman_rect: windows_sys::Win32::Foundation::RECT = std::mem::zeroed();
        GetWindowRect(progman, &mut progman_rect);
        let progman_width = progman_rect.right - progman_rect.left;
        let progman_height = progman_rect.bottom - progman_rect.top;
        log::info!("Progman size: {}x{}", progman_width, progman_height);

        if shell_view != 0 as HWND {
            // Windows 11 style: SHELLDLL_DefView is in Progman
            // We parent to Progman and position below shell_view
            log::info!("Using Windows 11 style injection (SHELLDLL_DefView in Progman)");
            log::info!("shell_view: {:?}, workerw: {:?}", shell_view, WORKERW);

            // Step 1: Modify window style - remove popup/disabled, add child
            let style = GetWindowLongPtrA(hwnd, GWL_STYLE);
            let new_style = (style & !(WS_POPUP as isize) & !(WS_DISABLED as isize)) | WS_CHILD as isize;
            SetWindowLongPtrA(hwnd, GWL_STYLE, new_style);
            log::info!("Style: {:X} -> {:X}", style, new_style);

            // Step 2: Parent to Progman
            let old_parent = SetParent(hwnd, progman);
            log::info!("SetParent to Progman, old parent: {:?}", old_parent);

            // Step 3: Make layered and set alpha
            let ex_style = GetWindowLongPtrA(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrA(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED as isize);
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);

            // Step 4: Position BELOW shell_view (this is the key!)
            // Using shell_view as hWndInsertAfter places our window directly below it in z-order
            let pos_result = SetWindowPos(
                hwnd,
                shell_view,  // Insert after (below) shell_view
                0,
                0,
                progman_width,
                progman_height,
                SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
            log::info!("SetWindowPos below shell_view result: {}", pos_result);

            // Step 5: If we have WorkerW, reorder it above our window
            if WORKERW != 0 as HWND {
                SetWindowPos(
                    WORKERW,
                    hwnd,  // Insert after (below) our window
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
                log::info!("Reordered WorkerW above our window");
            }

            // Step 6: Show without activating
            ShowWindow(hwnd, SW_SHOWNA);
            log::info!("Window shown as wallpaper");

        } else if WORKERW != 0 as HWND {
            // Legacy Windows 10 style: Use WorkerW as parent
            log::info!("Using legacy WorkerW style injection");

            let style = GetWindowLongPtrA(hwnd, GWL_STYLE);
            let new_style = (style & !(WS_POPUP as isize) & !(WS_DISABLED as isize)) | WS_CHILD as isize;
            SetWindowLongPtrA(hwnd, GWL_STYLE, new_style);

            SetParent(hwnd, WORKERW);

            let ex_style = GetWindowLongPtrA(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrA(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED as isize);
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);

            SetWindowPos(hwnd, 0 as HWND, 0, 0, width, height, SWP_NOACTIVATE | SWP_FRAMECHANGED);
            ShowWindow(hwnd, SW_SHOWNA);
            log::info!("Window shown as wallpaper (WorkerW parent)");

        } else {
            log::error!("Could not find shell_view or WorkerW - wallpaper injection failed");
        }
    }
}

#[cfg(target_os = "windows")]
fn setup_wallpaper_window(window: &Window, display: &DisplayInfo) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
            let hwnd = win32_handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            unsafe {
                windows_wallpaper::setup_as_wallpaper(
                    hwnd,
                    display.origin_x as i32,
                    display.origin_y as i32,
                    display.pixels_wide as i32,
                    display.pixels_high as i32,
                );
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn get_all_displays() -> Vec<DisplayInfo> {
    use windows_sys::Win32::Foundation::{BOOL, LPARAM, RECT, TRUE};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoA, HDC, HMONITOR, MONITORINFO,
    };

    static mut DISPLAYS: Vec<DisplayInfo> = Vec::new();

    unsafe extern "system" fn monitor_enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _lprect: *mut RECT,
        _lparam: LPARAM,
    ) -> BOOL {
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;

        if GetMonitorInfoA(hmonitor, &mut info) != 0 {
            let rect = info.rcMonitor;
            let width = (rect.right - rect.left) as f64;
            let height = (rect.bottom - rect.top) as f64;

            DISPLAYS.push(DisplayInfo {
                origin_x: rect.left as f64,
                origin_y: rect.top as f64,
                width,
                height,
                pixels_wide: width as u32,
                pixels_high: height as u32,
            });
        }
        TRUE
    }

    unsafe {
        DISPLAYS.clear();
        EnumDisplayMonitors(0 as HDC, std::ptr::null(), Some(monitor_enum_proc), 0);

        if DISPLAYS.is_empty() {
            // Fallback to default display
            vec![DisplayInfo {
                origin_x: 0.0,
                origin_y: 0.0,
                width: 1920.0,
                height: 1080.0,
                pixels_wide: 1920,
                pixels_high: 1080,
            }]
        } else {
            DISPLAYS.clone()
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn setup_wallpaper_window(_window: &Window, _display: &DisplayInfo) {
    log::warn!("Wallpaper mode is only supported on macOS and Windows");
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn get_all_displays() -> Vec<DisplayInfo> {
    vec![DisplayInfo {
        origin_x: 0.0,
        origin_y: 0.0,
        width: 1920.0,
        height: 1080.0,
        pixels_wide: 1920,
        pixels_high: 1080,
    }]
}

/// Check if launch at login is enabled (LaunchAgent exists)
#[cfg(target_os = "macos")]
fn is_launch_at_login_enabled() -> bool {
    let home = std::env::var("HOME").unwrap_or_default();
    let plist_path = format!("{}/Library/LaunchAgents/me.sandydoo.driftpaper.plist", home);
    std::path::Path::new(&plist_path).exists()
}

/// Enable launch at login by creating a LaunchAgent
#[cfg(target_os = "macos")]
fn enable_launch_at_login() {
    let home = std::env::var("HOME").unwrap_or_default();
    let launch_agents_dir = format!("{}/Library/LaunchAgents", home);
    let plist_path = format!("{}/me.sandydoo.driftpaper.plist", launch_agents_dir);

    // Get the path to the current executable
    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "/Applications/DriftPaper.app/Contents/MacOS/DriftPaper".to_string());

    let plist_content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>me.sandydoo.driftpaper</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--fps</string>
        <string>30</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#, exe_path);

    // Create LaunchAgents directory if it doesn't exist
    let _ = std::fs::create_dir_all(&launch_agents_dir);

    match std::fs::write(&plist_path, plist_content) {
        Ok(_) => log::info!("Launch at login enabled: {}", plist_path),
        Err(e) => log::error!("Failed to enable launch at login: {}", e),
    }
}

/// Disable launch at login by removing the LaunchAgent
#[cfg(target_os = "macos")]
fn disable_launch_at_login() {
    let home = std::env::var("HOME").unwrap_or_default();
    let plist_path = format!("{}/Library/LaunchAgents/me.sandydoo.driftpaper.plist", home);

    match std::fs::remove_file(&plist_path) {
        Ok(_) => log::info!("Launch at login disabled"),
        Err(e) => log::error!("Failed to disable launch at login: {}", e),
    }
}

/// Setup macOS screen configuration change observer
/// This monitors for display resolution changes, display add/remove, etc.
#[cfg(target_os = "macos")]
fn setup_screen_change_observer() {
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSString, NSAutoreleasePool};
    use objc::{class, msg_send, sel, sel_impl};
    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};

    extern "C" fn screen_did_change(_this: &Object, _cmd: Sel, _notification: id) {
        log::info!("Screen configuration changed - will reinitialize displays");
        SCREEN_CONFIG_CHANGED.store(true, Ordering::SeqCst);
    }

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        // Create observer class if it doesn't exist
        let class_name = "ScreenChangeObserver";
        let observer: id;

        if let Some(existing_class) = objc::runtime::Class::get(class_name) {
            observer = msg_send![existing_class, new];
        } else {
            let superclass = class!(NSObject);
            let mut decl = ClassDecl::new(class_name, superclass).unwrap();
            decl.add_method(
                sel!(screenDidChange:),
                screen_did_change as extern "C" fn(&Object, Sel, id),
            );
            let observer_class = decl.register();
            observer = msg_send![observer_class, new];
        }

        // Register for screen change notifications
        let notification_center: id = msg_send![class!(NSNotificationCenter), defaultCenter];
        let notification_name = NSString::alloc(nil).init_str("NSApplicationDidChangeScreenParametersNotification");

        let _: () = msg_send![notification_center, addObserver:observer
            selector:sel!(screenDidChange:)
            name:notification_name
            object:nil];

        // Retain observer to prevent deallocation
        let _: () = msg_send![observer, retain];

        log::info!("Screen change observer registered");
    }
}

#[cfg(not(target_os = "macos"))]
fn setup_screen_change_observer() {
    log::warn!("Screen change observer is only supported on macOS");
}

/// Setup macOS menu bar item for wallpaper control
#[cfg(target_os = "macos")]
fn setup_menu_bar() {
    use cocoa::appkit::{
        NSMenu, NSMenuItem, NSStatusBar, NSVariableStatusItemLength,
    };
    use cocoa::base::{id, nil, selector, YES, NO};
    use cocoa::foundation::{NSAutoreleasePool, NSString};
    use objc::{class, msg_send, sel, sel_impl};
    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel, BOOL};

    // Action handlers
    extern "C" fn quit_action(_this: &Object, _cmd: Sel, _sender: id) {
        log::info!("Quit requested from menu bar");
        SHOULD_QUIT.store(true, Ordering::SeqCst);
    }

    extern "C" fn toggle_login_action(_this: &Object, _cmd: Sel, sender: id) {
        // Toggle the login setting
        let was_enabled = is_launch_at_login_enabled();
        if was_enabled {
            disable_launch_at_login();
        } else {
            enable_launch_at_login();
        }
        // Update the menu item checkmark
        unsafe {
            let new_state: i64 = if was_enabled { 0 } else { 1 }; // NSOffState = 0, NSOnState = 1
            let _: () = msg_send![sender, setState: new_state];
        }
    }

    extern "C" fn set_color_original(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_color_original action triggered");
        set_color_scheme(0, sender);
    }

    extern "C" fn set_color_plasma(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_color_plasma action triggered");
        set_color_scheme(1, sender);
    }

    extern "C" fn set_color_poolside(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_color_poolside action triggered");
        set_color_scheme(2, sender);
    }

    extern "C" fn set_color_spacegrey(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_color_spacegrey action triggered");
        set_color_scheme(3, sender);
    }

    extern "C" fn set_color_custom_image(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_color_custom_image action triggered");
        // Open file dialog on a separate thread to avoid blocking the menu
        std::thread::spawn(move || {
            let dialog = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "webp"])
                .set_title("Choose an image for color theme");
            if let Some(path) = dialog.pick_file() {
                match extract_colors_from_image(&path) {
                    Ok(wheel) => {
                        // Store in global mutex
                        if let Ok(mut guard) = custom_color_wheel().lock() {
                            *guard = Some(wheel);
                        }
                        // Save to preferences
                        let mut prefs = load_preferences();
                        prefs.color_scheme = 4;
                        prefs.custom_color_wheel = Some(wheel);
                        prefs.custom_image_path = Some(path.to_string_lossy().to_string());
                        save_preferences(&prefs);
                        CURRENT_COLOR_SCHEME.store(4, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        log::info!("Custom color theme extracted from: {:?}", path);
                    }
                    Err(e) => {
                        log::error!("Failed to extract colors: {}", e);
                    }
                }
            } else {
                log::info!("Custom image file dialog cancelled");
            }
        });
        // Update checkmarks for the color menu (sender is the "Custom Image..." item)
        unsafe {
            let menu: id = msg_send![sender, menu];
            if menu != nil {
                let count: i64 = msg_send![menu, numberOfItems];
                for i in 0..count {
                    let item: id = msg_send![menu, itemAtIndex: i];
                    let tag: i64 = msg_send![item, tag];
                    let state: i64 = if tag == 4 { 1 } else { 0 };
                    let _: () = msg_send![item, setState: state];
                }
            }
        }
    }

    fn set_color_scheme(scheme: u32, sender: id) {
        log::info!("Setting color scheme to: {}", scheme);
        CURRENT_COLOR_SCHEME.store(scheme, Ordering::SeqCst);
        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
        // Save preference
        let mut prefs = load_preferences();
        prefs.color_scheme = scheme;
        save_preferences(&prefs);
        // Update checkmarks - get parent menu and update all items
        unsafe {
            let menu: id = msg_send![sender, menu];
            let count: i64 = msg_send![menu, numberOfItems];
            for i in 0..count {
                let item: id = msg_send![menu, itemAtIndex: i];
                let tag: i64 = msg_send![item, tag];
                let state: i64 = if tag == scheme as i64 { 1 } else { 0 };
                let _: () = msg_send![item, setState: state];
            }
        }
    }

    extern "C" fn set_density_sparse(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_density_sparse action triggered");
        set_density(0, sender);
    }

    extern "C" fn set_density_normal(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_density_normal action triggered");
        set_density(1, sender);
    }

    extern "C" fn set_density_dense(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_density_dense action triggered");
        set_density(2, sender);
    }

    fn set_density(density: u32, sender: id) {
        log::info!("Density changed to: {}", density);
        CURRENT_DENSITY.store(density, Ordering::SeqCst);
        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
        // Save preference
        let mut prefs = load_preferences();
        prefs.density = density;
        save_preferences(&prefs);
        // Update checkmarks
        unsafe {
            let menu: id = msg_send![sender, menu];
            let count: i64 = msg_send![menu, numberOfItems];
            for i in 0..count {
                let item: id = msg_send![menu, itemAtIndex: i];
                let tag: i64 = msg_send![item, tag];
                let state: i64 = if tag == density as i64 { 1 } else { 0 };
                let _: () = msg_send![item, setState: state];
            }
        }
    }

    extern "C" fn set_noise_low(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_noise_low action triggered");
        set_noise_strength(0, sender);
    }

    extern "C" fn set_noise_medium(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_noise_medium action triggered");
        set_noise_strength(1, sender);
    }

    extern "C" fn set_noise_high(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_noise_high action triggered");
        set_noise_strength(2, sender);
    }

    extern "C" fn set_noise_max(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_noise_max action triggered");
        set_noise_strength(3, sender);
    }

    fn set_noise_strength(strength: u32, sender: id) {
        log::info!("Noise strength changed to: {}", strength);
        CURRENT_NOISE_STRENGTH.store(strength, Ordering::SeqCst);
        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
        // Save preference
        let mut prefs = load_preferences();
        prefs.noise_strength = strength;
        save_preferences(&prefs);
        // Update checkmarks
        unsafe {
            let menu: id = msg_send![sender, menu];
            let count: i64 = msg_send![menu, numberOfItems];
            for i in 0..count {
                let item: id = msg_send![menu, itemAtIndex: i];
                let tag: i64 = msg_send![item, tag];
                let state: i64 = if tag == strength as i64 { 1 } else { 0 };
                let _: () = msg_send![item, setState: state];
            }
        }
    }

    // ===== Line Length Handlers =====
    extern "C" fn set_line_short(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_line_short action triggered");
        set_line_length(0, sender);
    }

    extern "C" fn set_line_medium(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_line_medium action triggered");
        set_line_length(1, sender);
    }

    extern "C" fn set_line_long(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_line_long action triggered");
        set_line_length(2, sender);
    }

    extern "C" fn set_line_extra_long(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_line_extra_long action triggered");
        set_line_length(3, sender);
    }

    fn set_line_length(length: u32, sender: id) {
        log::info!("Line length changed to: {}", length);
        CURRENT_LINE_LENGTH.store(length, Ordering::SeqCst);
        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
        let mut prefs = load_preferences();
        prefs.line_length = length;
        save_preferences(&prefs);
        unsafe {
            let menu: id = msg_send![sender, menu];
            let count: i64 = msg_send![menu, numberOfItems];
            for i in 0..count {
                let item: id = msg_send![menu, itemAtIndex: i];
                let tag: i64 = msg_send![item, tag];
                let state: i64 = if tag == length as i64 { 1 } else { 0 };
                let _: () = msg_send![item, setState: state];
            }
        }
    }

    // ===== Line Width Handlers =====
    extern "C" fn set_width_thin(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_width_thin action triggered");
        set_line_width(0, sender);
    }

    extern "C" fn set_width_medium(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_width_medium action triggered");
        set_line_width(1, sender);
    }

    extern "C" fn set_width_thick(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_width_thick action triggered");
        set_line_width(2, sender);
    }

    fn set_line_width(width: u32, sender: id) {
        log::info!("Line width changed to: {}", width);
        CURRENT_LINE_WIDTH.store(width, Ordering::SeqCst);
        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
        let mut prefs = load_preferences();
        prefs.line_width = width;
        save_preferences(&prefs);
        unsafe {
            let menu: id = msg_send![sender, menu];
            let count: i64 = msg_send![menu, numberOfItems];
            for i in 0..count {
                let item: id = msg_send![menu, itemAtIndex: i];
                let tag: i64 = msg_send![item, tag];
                let state: i64 = if tag == width as i64 { 1 } else { 0 };
                let _: () = msg_send![item, setState: state];
            }
        }
    }

    // ===== View Scale Handlers =====
    extern "C" fn set_scale_compact(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_scale_compact action triggered");
        set_view_scale(0, sender);
    }

    extern "C" fn set_scale_normal(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_scale_normal action triggered");
        set_view_scale(1, sender);
    }

    extern "C" fn set_scale_wide(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_scale_wide action triggered");
        set_view_scale(2, sender);
    }

    fn set_view_scale(scale: u32, sender: id) {
        log::info!("View scale changed to: {}", scale);
        CURRENT_VIEW_SCALE.store(scale, Ordering::SeqCst);
        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
        let mut prefs = load_preferences();
        prefs.view_scale = scale;
        save_preferences(&prefs);
        unsafe {
            let menu: id = msg_send![sender, menu];
            let count: i64 = msg_send![menu, numberOfItems];
            for i in 0..count {
                let item: id = msg_send![menu, itemAtIndex: i];
                let tag: i64 = msg_send![item, tag];
                let state: i64 = if tag == scale as i64 { 1 } else { 0 };
                let _: () = msg_send![item, setState: state];
            }
        }
    }

    // ===== Brightness Handlers =====
    extern "C" fn set_brightness_dim(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_brightness_dim action triggered");
        set_brightness(0, sender);
    }

    extern "C" fn set_brightness_normal(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_brightness_normal action triggered");
        set_brightness(1, sender);
    }

    extern "C" fn set_brightness_bright(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_brightness_bright action triggered");
        set_brightness(2, sender);
    }

    extern "C" fn set_brightness_vivid(_this: &Object, _cmd: Sel, sender: id) {
        log::info!("set_brightness_vivid action triggered");
        set_brightness(3, sender);
    }

    fn set_brightness(brightness: u32, sender: id) {
        log::info!("Brightness changed to: {}", brightness);
        CURRENT_BRIGHTNESS.store(brightness, Ordering::SeqCst);
        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
        let mut prefs = load_preferences();
        prefs.brightness = brightness;
        save_preferences(&prefs);
        unsafe {
            let menu: id = msg_send![sender, menu];
            let count: i64 = msg_send![menu, numberOfItems];
            for i in 0..count {
                let item: id = msg_send![menu, itemAtIndex: i];
                let tag: i64 = msg_send![item, tag];
                let state: i64 = if tag == brightness as i64 { 1 } else { 0 };
                let _: () = msg_send![item, setState: state];
            }
        }
    }

    // Delegate method to update menu when opened
    extern "C" fn menu_will_open(_this: &Object, _cmd: Sel, menu: id) {
        // Update login item state when menu opens
        unsafe {
            let login_item: id = msg_send![menu, itemWithTag: 100i64];
            if login_item != nil {
                let state: i64 = if is_launch_at_login_enabled() { 1 } else { 0 };
                let _: () = msg_send![login_item, setState: state];
            }
        }
    }

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        // Ensure NSApplication is initialized for LSUIElement apps
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, setActivationPolicy: 1i64]; // NSApplicationActivationPolicyAccessory

        // Load saved preferences
        let prefs = load_preferences();
        // If custom image scheme is selected but no cached wheel, fall back to Original
        let effective_scheme = if prefs.color_scheme == 4 && prefs.custom_color_wheel.is_none() {
            0
        } else {
            prefs.color_scheme
        };
        CURRENT_COLOR_SCHEME.store(effective_scheme, Ordering::SeqCst);
        CURRENT_DENSITY.store(prefs.density, Ordering::SeqCst);
        CURRENT_NOISE_STRENGTH.store(prefs.noise_strength, Ordering::SeqCst);
        CURRENT_LINE_LENGTH.store(prefs.line_length, Ordering::SeqCst);
        CURRENT_LINE_WIDTH.store(prefs.line_width, Ordering::SeqCst);
        CURRENT_VIEW_SCALE.store(prefs.view_scale, Ordering::SeqCst);
        CURRENT_BRIGHTNESS.store(prefs.brightness, Ordering::SeqCst);

        // Load cached custom color wheel if available
        if prefs.color_scheme == 4 {
            if let Some(wheel) = prefs.custom_color_wheel {
                if let Ok(mut guard) = custom_color_wheel().lock() {
                    *guard = Some(wheel);
                }
                log::info!("Loaded cached custom color wheel from preferences");
            }
        }

        // Register our action handler class (also as menu delegate)
        // Use a unique class name to avoid conflicts if app restarts
        let class_name = "DriftMenuHandler";
        let handler: id;

        // Check if class already exists
        if let Some(existing_class) = objc::runtime::Class::get(class_name) {
            // Class exists, create instance from it
            handler = msg_send![existing_class, new];
            log::info!("Using existing menu handler class");
        } else {
            // Create new class
            let superclass = class!(NSObject);
            let mut decl = ClassDecl::new(class_name, superclass).unwrap();
            decl.add_method(sel!(quitAction:), quit_action as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(toggleLoginAction:), toggle_login_action as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setColorOriginal:), set_color_original as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setColorPlasma:), set_color_plasma as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setColorPoolside:), set_color_poolside as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setColorSpacegrey:), set_color_spacegrey as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setColorCustomImage:), set_color_custom_image as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setDensitySparse:), set_density_sparse as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setDensityNormal:), set_density_normal as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setDensityDense:), set_density_dense as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setNoiseLow:), set_noise_low as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setNoiseMedium:), set_noise_medium as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setNoiseHigh:), set_noise_high as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setNoiseMax:), set_noise_max as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setLineShort:), set_line_short as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setLineMedium:), set_line_medium as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setLineLong:), set_line_long as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setLineExtraLong:), set_line_extra_long as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setWidthThin:), set_width_thin as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setWidthMedium:), set_width_medium as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setWidthThick:), set_width_thick as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setScaleCompact:), set_scale_compact as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setScaleNormal:), set_scale_normal as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setScaleWide:), set_scale_wide as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setBrightnessDim:), set_brightness_dim as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setBrightnessNormal:), set_brightness_normal as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setBrightnessBright:), set_brightness_bright as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(setBrightnessVivid:), set_brightness_vivid as extern "C" fn(&Object, Sel, id));
            decl.add_method(sel!(menuWillOpen:), menu_will_open as extern "C" fn(&Object, Sel, id));
            let handler_class = decl.register();
            handler = msg_send![handler_class, new];
            log::info!("Registered new menu handler class");
        }

        // Create the status bar item with fixed width
        let status_bar: id = NSStatusBar::systemStatusBar(nil);
        let status_item: id = status_bar.statusItemWithLength_(50.0); // Fixed width

        // Retain immediately to prevent deallocation
        let _: () = msg_send![status_item, retain];

        // Set the title on the status item button
        let button: id = msg_send![status_item, button];
        if button != nil {
            let title = NSString::alloc(nil).init_str("Drift");
            let _: () = msg_send![button, setTitle: title];
            log::info!("Status bar button title set to 'Drift'");
        }

        // Create the main menu
        let menu = NSMenu::new(nil).autorelease();
        let _: () = msg_send![menu, setDelegate: handler];
        let _: () = msg_send![menu, setAutoenablesItems: NO]; // Prevent auto-disabling of items

        // ===== Color Scheme Submenu =====
        let color_title = NSString::alloc(nil).init_str("Color Scheme");
        let color_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            color_title,
            selector(""),
            NSString::alloc(nil).init_str(""),
        );

        let color_menu = NSMenu::new(nil).autorelease();
        let _: () = msg_send![color_menu, setAutoenablesItems: NO]; // Prevent auto-disabling
        let color_names = ["Original", "Plasma", "Poolside", "Space Grey"];
        let color_selectors = [
            sel!(setColorOriginal:),
            sel!(setColorPlasma:),
            sel!(setColorPoolside:),
            sel!(setColorSpacegrey:),
        ];

        for (i, (name, action)) in color_names.iter().zip(color_selectors.iter()).enumerate() {
            let item_title = NSString::alloc(nil).init_str(name);
            let item: id = msg_send![class!(NSMenuItem), alloc];
            let item: id = msg_send![item, initWithTitle:item_title action:*action keyEquivalent:NSString::alloc(nil).init_str("")];
            let _: () = msg_send![item, setTarget: handler];
            let _: () = msg_send![item, setTag: i as i64];
            let _: () = msg_send![item, setEnabled: YES]; // Ensure item is enabled
            // Set initial checkmark based on saved preference
            if i as u32 == prefs.color_scheme {
                let _: () = msg_send![item, setState: 1i64]; // NSOnState
            }

            // Debug: verify item configuration
            let is_enabled: BOOL = msg_send![item, isEnabled];
            let target: id = msg_send![item, target];
            let item_action: Sel = msg_send![item, action];
            log::info!("Color item '{}': enabled={}, has_target={}, action={:?}",
                name, is_enabled != NO, target != nil, item_action);

            color_menu.addItem_(item);
        }

        // Separator before custom image option
        let color_sep: id = msg_send![class!(NSMenuItem), separatorItem];
        color_menu.addItem_(color_sep);

        // "Custom Image..." menu item
        let custom_title = NSString::alloc(nil).init_str("Custom Image...");
        let custom_item: id = msg_send![class!(NSMenuItem), alloc];
        let custom_item: id = msg_send![custom_item, initWithTitle:custom_title action:sel!(setColorCustomImage:) keyEquivalent:NSString::alloc(nil).init_str("")];
        let _: () = msg_send![custom_item, setTarget: handler];
        let _: () = msg_send![custom_item, setTag: 4i64];
        let _: () = msg_send![custom_item, setEnabled: YES];
        if prefs.color_scheme == 4 {
            let _: () = msg_send![custom_item, setState: 1i64]; // NSOnState
        }
        color_menu.addItem_(custom_item);

        let _: () = msg_send![color_item, setSubmenu: color_menu];
        menu.addItem_(color_item);

        // ===== Density Submenu =====
        let density_title = NSString::alloc(nil).init_str("Density");
        let density_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            density_title,
            selector(""),
            NSString::alloc(nil).init_str(""),
        );

        let density_menu = NSMenu::new(nil).autorelease();
        let _: () = msg_send![density_menu, setAutoenablesItems: NO]; // Prevent auto-disabling
        let density_names = ["Sparse", "Normal", "Dense"];
        let density_selectors = [
            sel!(setDensitySparse:),
            sel!(setDensityNormal:),
            sel!(setDensityDense:),
        ];

        for (i, (name, action)) in density_names.iter().zip(density_selectors.iter()).enumerate() {
            let item_title = NSString::alloc(nil).init_str(name);
            let item: id = msg_send![class!(NSMenuItem), alloc];
            let item: id = msg_send![item, initWithTitle:item_title action:*action keyEquivalent:NSString::alloc(nil).init_str("")];
            let _: () = msg_send![item, setTarget: handler];
            let _: () = msg_send![item, setTag: i as i64];
            let _: () = msg_send![item, setEnabled: YES]; // Ensure item is enabled
            // Set initial checkmark based on saved preference
            if i as u32 == prefs.density {
                let _: () = msg_send![item, setState: 1i64]; // NSOnState
            }
            density_menu.addItem_(item);
        }

        let _: () = msg_send![density_item, setSubmenu: density_menu];
        menu.addItem_(density_item);

        // ===== Noise Strength Submenu =====
        let noise_title = NSString::alloc(nil).init_str("Noise Strength");
        let noise_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            noise_title,
            selector(""),
            NSString::alloc(nil).init_str(""),
        );

        let noise_menu = NSMenu::new(nil).autorelease();
        let _: () = msg_send![noise_menu, setAutoenablesItems: NO]; // Prevent auto-disabling
        let noise_names = ["Low", "Medium", "High", "Max"];
        let noise_selectors = [
            sel!(setNoiseLow:),
            sel!(setNoiseMedium:),
            sel!(setNoiseHigh:),
            sel!(setNoiseMax:),
        ];

        for (i, (name, action)) in noise_names.iter().zip(noise_selectors.iter()).enumerate() {
            let item_title = NSString::alloc(nil).init_str(name);
            let item: id = msg_send![class!(NSMenuItem), alloc];
            let item: id = msg_send![item, initWithTitle:item_title action:*action keyEquivalent:NSString::alloc(nil).init_str("")];
            let _: () = msg_send![item, setTarget: handler];
            let _: () = msg_send![item, setTag: i as i64];
            let _: () = msg_send![item, setEnabled: YES]; // Ensure item is enabled
            // Set initial checkmark based on saved preference
            if i as u32 == prefs.noise_strength {
                let _: () = msg_send![item, setState: 1i64]; // NSOnState
            }
            noise_menu.addItem_(item);
        }

        let _: () = msg_send![noise_item, setSubmenu: noise_menu];
        menu.addItem_(noise_item);

        // ===== Line Length Submenu =====
        let line_length_title = NSString::alloc(nil).init_str("Line Length");
        let line_length_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            line_length_title,
            selector(""),
            NSString::alloc(nil).init_str(""),
        );

        let line_length_menu = NSMenu::new(nil).autorelease();
        let _: () = msg_send![line_length_menu, setAutoenablesItems: NO];
        let line_length_names = ["Short", "Medium", "Long", "Extra Long"];
        let line_length_selectors = [
            sel!(setLineShort:),
            sel!(setLineMedium:),
            sel!(setLineLong:),
            sel!(setLineExtraLong:),
        ];

        for (i, (name, action)) in line_length_names.iter().zip(line_length_selectors.iter()).enumerate() {
            let item_title = NSString::alloc(nil).init_str(name);
            let item: id = msg_send![class!(NSMenuItem), alloc];
            let item: id = msg_send![item, initWithTitle:item_title action:*action keyEquivalent:NSString::alloc(nil).init_str("")];
            let _: () = msg_send![item, setTarget: handler];
            let _: () = msg_send![item, setTag: i as i64];
            let _: () = msg_send![item, setEnabled: YES];
            if i as u32 == prefs.line_length {
                let _: () = msg_send![item, setState: 1i64];
            }
            line_length_menu.addItem_(item);
        }

        let _: () = msg_send![line_length_item, setSubmenu: line_length_menu];
        menu.addItem_(line_length_item);

        // ===== Line Width Submenu =====
        let line_width_title = NSString::alloc(nil).init_str("Line Width");
        let line_width_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            line_width_title,
            selector(""),
            NSString::alloc(nil).init_str(""),
        );

        let line_width_menu = NSMenu::new(nil).autorelease();
        let _: () = msg_send![line_width_menu, setAutoenablesItems: NO];
        let line_width_names = ["Thin", "Medium", "Thick"];
        let line_width_selectors = [
            sel!(setWidthThin:),
            sel!(setWidthMedium:),
            sel!(setWidthThick:),
        ];

        for (i, (name, action)) in line_width_names.iter().zip(line_width_selectors.iter()).enumerate() {
            let item_title = NSString::alloc(nil).init_str(name);
            let item: id = msg_send![class!(NSMenuItem), alloc];
            let item: id = msg_send![item, initWithTitle:item_title action:*action keyEquivalent:NSString::alloc(nil).init_str("")];
            let _: () = msg_send![item, setTarget: handler];
            let _: () = msg_send![item, setTag: i as i64];
            let _: () = msg_send![item, setEnabled: YES];
            if i as u32 == prefs.line_width {
                let _: () = msg_send![item, setState: 1i64];
            }
            line_width_menu.addItem_(item);
        }

        let _: () = msg_send![line_width_item, setSubmenu: line_width_menu];
        menu.addItem_(line_width_item);

        // ===== View Scale Submenu =====
        let view_scale_title = NSString::alloc(nil).init_str("View Scale");
        let view_scale_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            view_scale_title,
            selector(""),
            NSString::alloc(nil).init_str(""),
        );

        let view_scale_menu = NSMenu::new(nil).autorelease();
        let _: () = msg_send![view_scale_menu, setAutoenablesItems: NO];
        let view_scale_names = ["Compact", "Normal", "Wide"];
        let view_scale_selectors = [
            sel!(setScaleCompact:),
            sel!(setScaleNormal:),
            sel!(setScaleWide:),
        ];

        for (i, (name, action)) in view_scale_names.iter().zip(view_scale_selectors.iter()).enumerate() {
            let item_title = NSString::alloc(nil).init_str(name);
            let item: id = msg_send![class!(NSMenuItem), alloc];
            let item: id = msg_send![item, initWithTitle:item_title action:*action keyEquivalent:NSString::alloc(nil).init_str("")];
            let _: () = msg_send![item, setTarget: handler];
            let _: () = msg_send![item, setTag: i as i64];
            let _: () = msg_send![item, setEnabled: YES];
            if i as u32 == prefs.view_scale {
                let _: () = msg_send![item, setState: 1i64];
            }
            view_scale_menu.addItem_(item);
        }

        let _: () = msg_send![view_scale_item, setSubmenu: view_scale_menu];
        menu.addItem_(view_scale_item);

        // ===== Brightness Submenu =====
        let brightness_title = NSString::alloc(nil).init_str("Brightness");
        let brightness_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            brightness_title,
            selector(""),
            NSString::alloc(nil).init_str(""),
        );

        let brightness_menu = NSMenu::new(nil).autorelease();
        let _: () = msg_send![brightness_menu, setAutoenablesItems: NO];

        let brightness_names = ["Dim", "Normal", "Bright", "Vivid"];
        let brightness_selectors = [
            sel!(setBrightnessDim:),
            sel!(setBrightnessNormal:),
            sel!(setBrightnessBright:),
            sel!(setBrightnessVivid:),
        ];

        for (i, (name, action)) in brightness_names.iter().zip(brightness_selectors.iter()).enumerate() {
            let item_title = NSString::alloc(nil).init_str(name);
            let item: id = msg_send![class!(NSMenuItem), alloc];
            let item: id = msg_send![item, initWithTitle:item_title action:*action keyEquivalent:NSString::alloc(nil).init_str("")];
            let _: () = msg_send![item, setTarget: handler];
            let _: () = msg_send![item, setTag: i as i64];
            let _: () = msg_send![item, setEnabled: YES];
            if i as u32 == prefs.brightness {
                let _: () = msg_send![item, setState: 1i64];
            }
            brightness_menu.addItem_(item);
        }

        let _: () = msg_send![brightness_item, setSubmenu: brightness_menu];
        menu.addItem_(brightness_item);

        // ===== Separator =====
        let separator1: id = msg_send![class!(NSMenuItem), separatorItem];
        menu.addItem_(separator1);

        // ===== Launch at Login =====
        let login_title = NSString::alloc(nil).init_str("Launch at Login");
        let login_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            login_title,
            selector("toggleLoginAction:"),
            NSString::alloc(nil).init_str(""),
        );
        let _: () = msg_send![login_item, setTarget: handler];
        let _: () = msg_send![login_item, setTag: 100i64]; // Tag for identifying in delegate
        // Set initial state
        if is_launch_at_login_enabled() {
            let _: () = msg_send![login_item, setState: 1i64]; // NSOnState
        }
        menu.addItem_(login_item);

        // ===== Separator =====
        let separator2: id = msg_send![class!(NSMenuItem), separatorItem];
        menu.addItem_(separator2);

        // ===== Quit =====
        let quit_title = NSString::alloc(nil).init_str("Quit DriftPaper");
        let quit_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            quit_title,
            selector("quitAction:"),
            NSString::alloc(nil).init_str("q"),
        );
        let _: () = msg_send![quit_item, setTarget: handler];
        menu.addItem_(quit_item);

        // Attach menu to status item
        let _: () = msg_send![status_item, setMenu: menu];

        // Explicitly set visible and ensure button is enabled
        let _: () = msg_send![status_item, setVisible: YES];
        if button != nil {
            let _: () = msg_send![button, setEnabled: YES];
            // Force button to display
            let _: () = msg_send![button, setNeedsDisplay: YES];
        }

        // Retain the status item, handler, and menus to prevent deallocation
        let _: () = msg_send![status_item, retain];
        let _: () = msg_send![handler, retain];
        let _: () = msg_send![menu, retain];
        let _: () = msg_send![color_menu, retain];
        let _: () = msg_send![density_menu, retain];
        let _: () = msg_send![noise_menu, retain];
        let _: () = msg_send![line_length_menu, retain];
        let _: () = msg_send![line_width_menu, retain];
        let _: () = msg_send![view_scale_menu, retain];
        let _: () = msg_send![brightness_menu, retain];

        // Store in static to prevent deallocation
        static mut STATUS_ITEM: *mut Object = std::ptr::null_mut();
        STATUS_ITEM = status_item as *mut Object;

        log::info!(
            "Menu bar item created (launch at login: {}, color: {}, density: {}, noise: {}, line_length: {}, line_width: {}, view_scale: {})",
            is_launch_at_login_enabled(),
            prefs.color_scheme,
            prefs.density,
            prefs.noise_strength,
            prefs.line_length,
            prefs.line_width,
            prefs.view_scale
        );
    }
}

// ==================== Windows Startup Registry ====================

#[cfg(target_os = "windows")]
fn is_run_on_login_enabled() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run") {
        key.get_value::<String, _>("DriftPaper").is_ok()
    } else {
        false
    }
}

#[cfg(target_os = "windows")]
fn set_run_on_login(enable: bool) {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Run", KEY_SET_VALUE | KEY_QUERY_VALUE) {
        if enable {
            // Get the current executable path
            if let Ok(exe_path) = std::env::current_exe() {
                let path_str = exe_path.to_string_lossy().to_string();
                if let Err(e) = key.set_value("DriftPaper", &path_str) {
                    log::error!("Failed to set run on login: {}", e);
                } else {
                    log::info!("Run on login enabled: {}", path_str);
                }
            }
        } else {
            if let Err(e) = key.delete_value("DriftPaper") {
                log::warn!("Failed to remove run on login (may not exist): {}", e);
            } else {
                log::info!("Run on login disabled");
            }
        }
    } else {
        log::error!("Failed to open registry key for run on login");
    }
}

// ==================== Windows System Tray ====================

#[cfg(target_os = "windows")]
fn setup_menu_bar() -> Option<tray_icon::TrayIcon> {
    use tray_icon::{TrayIconBuilder, Icon};
    use muda::{Menu, MenuItem, Submenu, PredefinedMenuItem, CheckMenuItem};

    let prefs = load_preferences();

    // Load cached custom color wheel if available, or fall back
    let effective_scheme = if prefs.color_scheme == 4 && prefs.custom_color_wheel.is_none() {
        0
    } else {
        prefs.color_scheme
    };
    if prefs.color_scheme == 4 {
        if let Some(wheel) = prefs.custom_color_wheel {
            if let Ok(mut guard) = custom_color_wheel().lock() {
                *guard = Some(wheel);
            }
            log::info!("Loaded cached custom color wheel from preferences (Windows)");
        }
    }

    // Load preferences into atomics
    CURRENT_COLOR_SCHEME.store(effective_scheme, Ordering::SeqCst);
    CURRENT_DENSITY.store(prefs.density, Ordering::SeqCst);
    CURRENT_NOISE_STRENGTH.store(prefs.noise_strength, Ordering::SeqCst);
    CURRENT_LINE_LENGTH.store(prefs.line_length, Ordering::SeqCst);
    CURRENT_LINE_WIDTH.store(prefs.line_width, Ordering::SeqCst);
    CURRENT_VIEW_SCALE.store(prefs.view_scale, Ordering::SeqCst);
    CURRENT_BRIGHTNESS.store(prefs.brightness, Ordering::SeqCst);

    // Create menu
    let menu = Menu::new();

    // Color Scheme submenu
    let color_submenu = Submenu::new("Color Scheme", true);
    let color_original = CheckMenuItem::new("Original", true, prefs.color_scheme == 0, None);
    let color_plasma = CheckMenuItem::new("Plasma", true, prefs.color_scheme == 1, None);
    let color_poolside = CheckMenuItem::new("Poolside", true, prefs.color_scheme == 2, None);
    let color_spacegrey = CheckMenuItem::new("Space Grey", true, prefs.color_scheme == 3, None);
    let color_custom = CheckMenuItem::new("Custom Image...", true, prefs.color_scheme == 4, None);
    let _ = color_submenu.append(&color_original);
    let _ = color_submenu.append(&color_plasma);
    let _ = color_submenu.append(&color_poolside);
    let _ = color_submenu.append(&color_spacegrey);
    let _ = color_submenu.append(&muda::PredefinedMenuItem::separator());
    let _ = color_submenu.append(&color_custom);
    let _ = menu.append(&color_submenu);

    // Density submenu
    let density_submenu = Submenu::new("Density", true);
    let density_sparse = CheckMenuItem::new("Sparse", true, prefs.density == 0, None);
    let density_normal = CheckMenuItem::new("Normal", true, prefs.density == 1, None);
    let density_dense = CheckMenuItem::new("Dense", true, prefs.density == 2, None);
    let _ = density_submenu.append(&density_sparse);
    let _ = density_submenu.append(&density_normal);
    let _ = density_submenu.append(&density_dense);
    let _ = menu.append(&density_submenu);

    // Noise Strength submenu
    let noise_submenu = Submenu::new("Noise Strength", true);
    let noise_low = CheckMenuItem::new("Low", true, prefs.noise_strength == 0, None);
    let noise_medium = CheckMenuItem::new("Medium", true, prefs.noise_strength == 1, None);
    let noise_high = CheckMenuItem::new("High", true, prefs.noise_strength == 2, None);
    let noise_max = CheckMenuItem::new("Max", true, prefs.noise_strength == 3, None);
    let _ = noise_submenu.append(&noise_low);
    let _ = noise_submenu.append(&noise_medium);
    let _ = noise_submenu.append(&noise_high);
    let _ = noise_submenu.append(&noise_max);
    let _ = menu.append(&noise_submenu);

    // Line Length submenu
    let length_submenu = Submenu::new("Line Length", true);
    let length_short = CheckMenuItem::new("Short", true, prefs.line_length == 0, None);
    let length_medium = CheckMenuItem::new("Medium", true, prefs.line_length == 1, None);
    let length_long = CheckMenuItem::new("Long", true, prefs.line_length == 2, None);
    let length_extra = CheckMenuItem::new("Extra Long", true, prefs.line_length == 3, None);
    let _ = length_submenu.append(&length_short);
    let _ = length_submenu.append(&length_medium);
    let _ = length_submenu.append(&length_long);
    let _ = length_submenu.append(&length_extra);
    let _ = menu.append(&length_submenu);

    // Line Width submenu
    let width_submenu = Submenu::new("Line Width", true);
    let width_thin = CheckMenuItem::new("Thin", true, prefs.line_width == 0, None);
    let width_medium = CheckMenuItem::new("Medium", true, prefs.line_width == 1, None);
    let width_thick = CheckMenuItem::new("Thick", true, prefs.line_width == 2, None);
    let _ = width_submenu.append(&width_thin);
    let _ = width_submenu.append(&width_medium);
    let _ = width_submenu.append(&width_thick);
    let _ = menu.append(&width_submenu);

    // View Scale submenu
    let scale_submenu = Submenu::new("View Scale", true);
    let scale_compact = CheckMenuItem::new("Compact", true, prefs.view_scale == 0, None);
    let scale_normal = CheckMenuItem::new("Normal", true, prefs.view_scale == 1, None);
    let scale_wide = CheckMenuItem::new("Wide", true, prefs.view_scale == 2, None);
    let _ = scale_submenu.append(&scale_compact);
    let _ = scale_submenu.append(&scale_normal);
    let _ = scale_submenu.append(&scale_wide);
    let _ = menu.append(&scale_submenu);

    // Brightness submenu
    let brightness_submenu = Submenu::new("Brightness", true);
    let brightness_dim = CheckMenuItem::new("Dim", true, prefs.brightness == 0, None);
    let brightness_normal = CheckMenuItem::new("Normal", true, prefs.brightness == 1, None);
    let brightness_bright = CheckMenuItem::new("Bright", true, prefs.brightness == 2, None);
    let brightness_vivid = CheckMenuItem::new("Vivid", true, prefs.brightness == 3, None);
    let _ = brightness_submenu.append(&brightness_dim);
    let _ = brightness_submenu.append(&brightness_normal);
    let _ = brightness_submenu.append(&brightness_bright);
    let _ = brightness_submenu.append(&brightness_vivid);
    let _ = menu.append(&brightness_submenu);

    let _ = menu.append(&PredefinedMenuItem::separator());

    // Run on Login item
    let run_on_login_enabled = is_run_on_login_enabled();
    let run_on_login_item = CheckMenuItem::new("Run on Login", true, run_on_login_enabled, None);
    let _ = menu.append(&run_on_login_item);

    let _ = menu.append(&PredefinedMenuItem::separator());

    // Quit item
    let quit_item = MenuItem::new("Quit DriftPaper", true, None);
    let quit_id = quit_item.id().clone();
    let _ = menu.append(&quit_item);

    // Try to load the app icon from embedded bytes, fall back to simple icon
    let icon = {
        // Embed the 32x32 PNG icon at compile time
        let icon_bytes = include_bytes!("../Assets.xcassets/AppIcon.appiconset/32.png");
        match image::load_from_memory(icon_bytes) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();
                Icon::from_rgba(rgba.into_raw(), width, height).unwrap_or_else(|_| {
                    let icon_data = vec![255u8; 32 * 32 * 4];
                    Icon::from_rgba(icon_data, 32, 32).expect("Failed to create fallback icon")
                })
            }
            Err(_) => {
                let icon_data = vec![255u8; 32 * 32 * 4];
                Icon::from_rgba(icon_data, 32, 32).expect("Failed to create fallback icon")
            }
        }
    };

    // Build tray icon - must be kept alive for the duration of the program
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("DriftPaper - Right-click for settings")
        .with_icon(icon)
        .build()
        .expect("Failed to create tray icon");

    log::info!("Windows system tray created");

    // Extract string IDs before spawning thread (MenuId contains Rc which is not Send)
    let color_ids: Vec<String> = [&color_original, &color_plasma, &color_poolside, &color_spacegrey, &color_custom]
        .iter().map(|item| item.id().0.clone()).collect();
    let density_ids: Vec<String> = [&density_sparse, &density_normal, &density_dense]
        .iter().map(|item| item.id().0.clone()).collect();
    let noise_ids: Vec<String> = [&noise_low, &noise_medium, &noise_high, &noise_max]
        .iter().map(|item| item.id().0.clone()).collect();
    let length_ids: Vec<String> = [&length_short, &length_medium, &length_long, &length_extra]
        .iter().map(|item| item.id().0.clone()).collect();
    let width_ids: Vec<String> = [&width_thin, &width_medium, &width_thick]
        .iter().map(|item| item.id().0.clone()).collect();
    let scale_ids: Vec<String> = [&scale_compact, &scale_normal, &scale_wide]
        .iter().map(|item| item.id().0.clone()).collect();
    let brightness_ids: Vec<String> = [&brightness_dim, &brightness_normal, &brightness_bright, &brightness_vivid]
        .iter().map(|item| item.id().0.clone()).collect();
    let run_on_login_id_str = run_on_login_item.id().0.clone();
    let quit_id_str = quit_id.0.clone();

    // Spawn thread to handle menu events
    std::thread::spawn(move || {
        use muda::MenuEvent;
        let menu_channel = MenuEvent::receiver();

        loop {
            if let Ok(event) = menu_channel.recv() {
                let id_str = &event.id.0;

                // Check color scheme
                for (i, color_id) in color_ids.iter().enumerate() {
                    if id_str == color_id {
                        if i == 4 {
                            // Custom Image - open file dialog
                            let dialog = rfd::FileDialog::new()
                                .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "webp"])
                                .set_title("Choose an image for color theme");
                            if let Some(path) = dialog.pick_file() {
                                match extract_colors_from_image(&path) {
                                    Ok(wheel) => {
                                        if let Ok(mut guard) = custom_color_wheel().lock() {
                                            *guard = Some(wheel);
                                        }
                                        let mut prefs = load_preferences();
                                        prefs.color_scheme = 4;
                                        prefs.custom_color_wheel = Some(wheel);
                                        prefs.custom_image_path = Some(path.to_string_lossy().to_string());
                                        save_preferences(&prefs);
                                        CURRENT_COLOR_SCHEME.store(4, Ordering::SeqCst);
                                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                                        log::info!("Custom color theme extracted from: {:?}", path);
                                    }
                                    Err(e) => {
                                        log::error!("Failed to extract colors: {}", e);
                                        continue;
                                    }
                                }
                            } else {
                                log::info!("Custom image file dialog cancelled");
                                continue;
                            }
                        } else {
                            CURRENT_COLOR_SCHEME.store(i as u32, Ordering::SeqCst);
                            SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                            let mut prefs = load_preferences();
                            prefs.color_scheme = i as u32;
                            save_preferences(&prefs);
                        }
                        log::info!("Color scheme changed to {}", i);
                    }
                }

                // Check density
                for (i, density_id) in density_ids.iter().enumerate() {
                    if id_str == density_id {
                        CURRENT_DENSITY.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        let mut prefs = load_preferences();
                        prefs.density = i as u32;
                        save_preferences(&prefs);
                        log::info!("Density changed to {}", i);
                    }
                }

                // Check noise
                for (i, noise_id) in noise_ids.iter().enumerate() {
                    if id_str == noise_id {
                        CURRENT_NOISE_STRENGTH.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        let mut prefs = load_preferences();
                        prefs.noise_strength = i as u32;
                        save_preferences(&prefs);
                        log::info!("Noise strength changed to {}", i);
                    }
                }

                // Check line length
                for (i, length_id) in length_ids.iter().enumerate() {
                    if id_str == length_id {
                        CURRENT_LINE_LENGTH.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        let mut prefs = load_preferences();
                        prefs.line_length = i as u32;
                        save_preferences(&prefs);
                        log::info!("Line length changed to {}", i);
                    }
                }

                // Check line width
                for (i, width_id) in width_ids.iter().enumerate() {
                    if id_str == width_id {
                        CURRENT_LINE_WIDTH.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        let mut prefs = load_preferences();
                        prefs.line_width = i as u32;
                        save_preferences(&prefs);
                        log::info!("Line width changed to {}", i);
                    }
                }

                // Check view scale
                for (i, scale_id) in scale_ids.iter().enumerate() {
                    if id_str == scale_id {
                        CURRENT_VIEW_SCALE.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        let mut prefs = load_preferences();
                        prefs.view_scale = i as u32;
                        save_preferences(&prefs);
                        log::info!("View scale changed to {}", i);
                    }
                }

                // Check brightness
                for (i, brightness_id) in brightness_ids.iter().enumerate() {
                    if id_str == brightness_id {
                        CURRENT_BRIGHTNESS.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        let mut prefs = load_preferences();
                        prefs.brightness = i as u32;
                        save_preferences(&prefs);
                        log::info!("Brightness changed to {}", i);
                    }
                }

                // Check run on login toggle
                if id_str == &run_on_login_id_str {
                    // Toggle the current state
                    let currently_enabled = is_run_on_login_enabled();
                    set_run_on_login(!currently_enabled);
                    let mut prefs = load_preferences();
                    prefs.run_on_login = !currently_enabled;
                    save_preferences(&prefs);
                    log::info!("Run on login toggled to {}", !currently_enabled);
                }

                // Check quit
                if id_str == &quit_id_str {
                    log::info!("Quit requested from tray");
                    SHOULD_QUIT.store(true, Ordering::SeqCst);
                    break;
                }
            }
        }
    });

    Some(tray_icon)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn setup_menu_bar() {
    log::warn!("System tray is only supported on macOS and Windows");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let event_loop = EventLoop::new().unwrap();

    // Wallpaper mode is the default; use --windowed for normal window
    if !args.windowed {
        // Setup menu bar for wallpaper control (must be on main thread before event loop)
        // On Windows, we need to keep the tray icon alive by storing the returned value
        #[cfg(target_os = "windows")]
        let _tray_icon = setup_menu_bar();
        #[cfg(not(target_os = "windows"))]
        setup_menu_bar();

        // Setup screen configuration change monitoring
        setup_screen_change_observer();

        // Get all displays and create one window per display
        let displays = get_all_displays();

        if displays.is_empty() {
            log::error!("No displays found");
            return Ok(());
        }

        log::info!("Creating {} wallpaper windows (one per display)", displays.len());

        // Create windows for each display
        let mut windows = Vec::new();
        for (i, display) in displays.iter().enumerate() {
            let logical_size = winit::dpi::LogicalSize::new(display.width, display.height);

            #[cfg(target_os = "macos")]
            let window = WindowBuilder::new()
                .with_title(&format!("DriftPaper {}", i))
                .with_decorations(false)
                .with_resizable(false)
                .with_inner_size(logical_size)
                .with_position(winit::dpi::LogicalPosition::new(display.origin_x, display.origin_y))
                .with_window_level(WindowLevel::AlwaysOnBottom)
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true)
                .build(&event_loop)
                .unwrap();

            #[cfg(not(target_os = "macos"))]
            let window = WindowBuilder::new()
                .with_title(&format!("DriftPaper {}", i))
                .with_decorations(false)
                .with_resizable(false)
                .with_inner_size(logical_size)
                .with_position(winit::dpi::LogicalPosition::new(display.origin_x, display.origin_y))
                .with_window_level(WindowLevel::AlwaysOnBottom)
                .build(&event_loop)
                .unwrap();

            windows.push((window, display.clone()));
        }

        pollster::block_on(run_wallpaper_multi(runtime, event_loop, windows, args))
    } else {
        let logical_size = winit::dpi::LogicalSize::new(1280, 800);

        #[cfg(target_os = "macos")]
        let window = WindowBuilder::new()
            .with_title("Drift")
            .with_decorations(true)
            .with_resizable(true)
            .with_inner_size(logical_size)
            .with_title_hidden(true)
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .build(&event_loop)
            .unwrap();

        #[cfg(not(target_os = "macos"))]
        let window = WindowBuilder::new()
            .with_title("Drift")
            .with_decorations(true)
            .with_resizable(true)
            .with_inner_size(logical_size)
            .build(&event_loop)
            .unwrap();

        pollster::block_on(run_normal(runtime, event_loop, window, args))
    }
}

/// State for a single display's renderer
struct DisplayRenderer {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    flux: Flux,
    display_info: DisplayInfo,
}

/// Multi-display wallpaper mode - creates one window per display for reliable rendering
async fn run_wallpaper_multi(
    _runtime: tokio::runtime::Runtime,
    event_loop: EventLoop<()>,
    windows: Vec<(winit::window::Window, DisplayInfo)>,
    args: Args,
) -> Result<(), Box<dyn std::error::Error>> {
    let wgpu_instance = wgpu::Instance::default();

    // Load user preferences and apply to settings
    let prefs = load_preferences();
    let mut settings = Settings::default();
    settings.color_mode = scheme_to_color_mode(prefs.color_scheme);
    settings.grid_spacing = density_to_grid_spacing(prefs.density);
    settings.noise_multiplier = noise_strength_to_multiplier(prefs.noise_strength);
    settings.line_length = line_length_to_value(prefs.line_length);
    settings.line_width = line_width_to_value(prefs.line_width);
    settings.view_scale = view_scale_to_value(prefs.view_scale);
    settings.brightness_multiplier = brightness_to_multiplier(prefs.brightness);
    let settings = Arc::new(settings);

    log::info!(
        "Applied settings from preferences: color={}, density={}, noise={}, line_length={}, line_width={}, view_scale={}, brightness={}",
        prefs.color_scheme,
        prefs.density,
        prefs.noise_strength,
        prefs.line_length,
        prefs.line_width,
        prefs.view_scale,
        prefs.brightness
    );

    // Initialize each display
    let mut renderers: Vec<DisplayRenderer> = Vec::new();

    for (window, display) in windows {
        // Setup wallpaper window properties
        setup_wallpaper_window(&window, &display);

        let window = Arc::new(window);

        // SAFETY: The window lives for the duration of the program
        let surface = unsafe {
            let surface = wgpu_instance.create_surface(Arc::clone(&window)).unwrap();
            std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(surface)
        };

        let adapter = wgpu_instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let adapter_info = adapter.get_info();
        log::info!(
            "Display adapter: {} ({:?}, {:?})",
            adapter_info.name,
            adapter_info.backend,
            adapter_info.device_type
        );

        let mut limits = wgpu::Limits::default().using_resolution(adapter.limits());
        limits.max_push_constant_size = 8;
        let features = wgpu::Features::PUSH_CONSTANTS
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
            | wgpu::Features::FLOAT32_FILTERABLE;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: features,
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .expect("Failed to create device");

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = get_preferred_format(&swapchain_capabilities);
        log::info!(
            "Surface format: {:?}, alpha modes: {:?}",
            swapchain_format,
            swapchain_capabilities.alpha_modes
        );

        // Query actual window size after configuration
        // IMPORTANT: winit's inner_size() returns PHYSICAL pixels (backing store size)
        // CGDisplay's pixels_wide/high returns LOGICAL pixels (points)
        // We must use the window's reported physical size for the surface
        let actual_size = window.inner_size();
        let scale_factor = window.scale_factor();

        // Use window's physical size for surface (NOT CGDisplay which lies about Retina)
        let physical_width = actual_size.width;
        let physical_height = actual_size.height;
        let logical_width = display.width as u32;
        let logical_height = display.height as u32;

        log::info!(
            "Display renderer: {}x{} logical, {}x{} physical (scale: {}, CGDisplay reported: {}x{})",
            logical_width, logical_height, physical_width, physical_height,
            scale_factor, display.pixels_wide, display.pixels_high
        );

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: physical_width.max(1),
            height: physical_height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let mut flux = Flux::new(
            &device,
            &queue,
            swapchain_format,
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            &Arc::clone(&settings),
        )
        .unwrap();

        // Inject cached custom color wheel on startup if scheme is Custom Image
        if prefs.color_scheme == 4 {
            if let Ok(guard) = custom_color_wheel().lock() {
                if let Some(wheel) = *guard {
                    let color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("buffer:custom_color"),
                        size: 4 * (wheel.len() as u64),
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    queue.write_buffer(&color_buffer, 0, bytemuck::cast_slice(&wheel));
                    flux.lines.update_color_bindings(&device, &queue, None, Some(color_buffer));
                    log::info!("Injected cached custom color wheel on startup");
                }
            }
        }

        window.set_visible(true);

        // Re-apply setIgnoresMouseEvents after window is visible
        // This ensures winit hasn't reset it during window setup
        #[cfg(target_os = "macos")]
        {
            use cocoa::base::{id, YES, NO, BOOL};
            use objc::{msg_send, sel, sel_impl};
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};

            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                    let ns_view: id = appkit_handle.ns_view.as_ptr() as id;
                    unsafe {
                        let ns_window: id = msg_send![ns_view, window];
                        // Ensure mouse events still pass through after window is visible
                        let _: () = msg_send![ns_window, setIgnoresMouseEvents: YES];

                        // Send window to back again after it becomes visible
                        let _: () = msg_send![ns_window, orderBack: std::ptr::null::<objc::runtime::Object>()];

                        // Verify the setting
                        let ignores: BOOL = msg_send![ns_window, ignoresMouseEvents];
                        let level: i64 = msg_send![ns_window, level];
                        log::info!(
                            "Post-visible: ignoresMouseEvents={}, windowLevel={}, ordered back",
                            ignores != NO, level
                        );
                    }
                }
            }
        }

        renderers.push(DisplayRenderer {
            window,
            surface,
            device,
            queue,
            config,
            flux,
            display_info: display,
        });
    }

    let start = std::time::Instant::now();
    let target_frame_time = std::time::Duration::from_secs_f64(1.0 / args.fps as f64);
    let mut last_frame = std::time::Instant::now();

    // Collect window IDs for event matching
    let window_ids: Vec<_> = renderers.iter().map(|r| r.window.id()).collect();

    event_loop.run(move |event, elwt| {
        // Check if quit was requested from menu bar
        if SHOULD_QUIT.load(Ordering::SeqCst) {
            log::info!("Exiting due to menu bar quit");
            elwt.exit();
            return;
        }

        // Check if settings changed from menu and apply live updates
        if SETTINGS_CHANGED.swap(false, Ordering::SeqCst) {
            let new_color = CURRENT_COLOR_SCHEME.load(Ordering::SeqCst);
            let new_density = CURRENT_DENSITY.load(Ordering::SeqCst);
            let new_noise = CURRENT_NOISE_STRENGTH.load(Ordering::SeqCst);
            let new_line_length = CURRENT_LINE_LENGTH.load(Ordering::SeqCst);
            let new_line_width = CURRENT_LINE_WIDTH.load(Ordering::SeqCst);
            let new_view_scale = CURRENT_VIEW_SCALE.load(Ordering::SeqCst);
            let new_brightness = CURRENT_BRIGHTNESS.load(Ordering::SeqCst);
            log::info!("Applying live settings update: color={}, density={}, noise={}, line_length={}, line_width={}, view_scale={}, brightness={}",
                new_color, new_density, new_noise, new_line_length, new_line_width, new_view_scale, new_brightness);

            let mut new_settings = Settings::default();
            new_settings.color_mode = scheme_to_color_mode(new_color);
            new_settings.grid_spacing = density_to_grid_spacing(new_density);
            new_settings.noise_multiplier = noise_strength_to_multiplier(new_noise);
            new_settings.line_length = line_length_to_value(new_line_length);
            new_settings.line_width = line_width_to_value(new_line_width);
            new_settings.view_scale = view_scale_to_value(new_view_scale);
            new_settings.brightness_multiplier = brightness_to_multiplier(new_brightness);
            let new_settings = Arc::new(new_settings);

            // Check if we have a custom color wheel to inject
            let custom_wheel = if new_color == 4 {
                custom_color_wheel().lock().ok().and_then(|g| *g)
            } else {
                None
            };

            for renderer in &mut renderers {
                // Check if density changed BEFORE updating (update overwrites settings)
                let density_changed = renderer.flux.grid_spacing() != new_settings.grid_spacing;

                // Update settings - this handles color, noise, line dimensions, brightness
                renderer.flux.update(&renderer.device, &renderer.queue, &new_settings);

                // Only resize if density changed (grid_spacing affects line count)
                // Resize recreates buffers which is expensive, so only do it when necessary
                if density_changed {
                    log::info!("Density changed, resizing renderer");
                    let physical_size = renderer.window.inner_size();
                    let logical_width = renderer.display_info.width as u32;
                    let logical_height = renderer.display_info.height as u32;
                    renderer.flux.resize(
                        &renderer.device,
                        &renderer.queue,
                        logical_width,
                        logical_height,
                        physical_size.width,
                        physical_size.height,
                    );
                }

                // Inject custom color wheel if scheme is Custom Image
                if let Some(wheel) = custom_wheel {
                    let color_buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("buffer:custom_color"),
                        size: 4 * (wheel.len() as u64),
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    renderer.queue.write_buffer(&color_buffer, 0, bytemuck::cast_slice(&wheel));
                    renderer.flux.lines.update_color_bindings(
                        &renderer.device,
                        &renderer.queue,
                        None,
                        Some(color_buffer),
                    );
                    log::info!("Injected custom color wheel into renderer");
                }
            }
        }

        // Check if screen configuration changed (resolution, display add/remove)
        if SCREEN_CONFIG_CHANGED.swap(false, Ordering::SeqCst) {
            let new_displays = get_all_displays();
            log::info!("Screen config changed, got {} displays (had {} renderers)",
                new_displays.len(), renderers.len());

            // For each renderer, try to match it with updated display info and resize
            for (i, renderer) in renderers.iter_mut().enumerate() {
                if let Some(display) = new_displays.get(i) {
                    // Update window position and size
                    #[cfg(target_os = "macos")]
                    {
                        use cocoa::base::id;
                        use cocoa::foundation::{NSPoint, NSRect, NSSize};
                        use objc::{msg_send, sel, sel_impl};
                        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

                        if let Ok(handle) = renderer.window.window_handle() {
                            if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                                let ns_view: id = appkit_handle.ns_view.as_ptr() as id;
                                unsafe {
                                    let ns_window: id = msg_send![ns_view, window];
                                    let frame_rect = NSRect::new(
                                        NSPoint::new(display.origin_x, display.origin_y),
                                        NSSize::new(display.width, display.height),
                                    );
                                    let _: () = msg_send![ns_window, setFrame: frame_rect display: cocoa::base::YES];
                                }
                            }
                        }
                    }

                    // Get the new physical size from the window
                    let new_physical_size = renderer.window.inner_size();
                    let _scale = renderer.window.scale_factor();
                    let logical_width = display.width as u32;
                    let logical_height = display.height as u32;

                    log::info!("Display {}: updating to {}x{} logical, {}x{} physical",
                        i, logical_width, logical_height,
                        new_physical_size.width, new_physical_size.height);

                    // Reconfigure surface
                    renderer.config.width = new_physical_size.width.max(1);
                    renderer.config.height = new_physical_size.height.max(1);
                    renderer.surface.configure(&renderer.device, &renderer.config);

                    // Resize flux renderer
                    renderer.flux.resize(
                        &renderer.device,
                        &renderer.queue,
                        logical_width,
                        logical_height,
                        new_physical_size.width,
                        new_physical_size.height,
                    );

                    // Update stored display info
                    renderer.display_info = display.clone();
                }
            }

            // If number of displays changed significantly, log a warning
            if new_displays.len() != renderers.len() {
                log::warn!(
                    "Number of displays changed ({} -> {}). Restart app for full reconfiguration.",
                    renderers.len(), new_displays.len()
                );
            }
        }

        elwt.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            last_frame + target_frame_time,
        ));

        match event {
            Event::AboutToWait => {
                let now = std::time::Instant::now();
                if now.duration_since(last_frame) >= target_frame_time {
                    // Request redraw on all windows
                    for renderer in &renderers {
                        renderer.window.request_redraw();
                    }
                    last_frame = now;
                }
            }
            Event::WindowEvent { event, window_id } => {
                // Find which renderer this event belongs to
                if let Some(renderer_idx) = window_ids.iter().position(|&id| id == window_id) {
                    match event {
                        WindowEvent::CloseRequested => elwt.exit(),
                        WindowEvent::KeyboardInput {
                            event: KeyEvent {
                                physical_key: PhysicalKey::Code(KeyCode::KeyQ),
                                state: ElementState::Released,
                                ..
                            },
                            ..
                        } => elwt.exit(),
                        WindowEvent::Resized(new_size) => {
                            let renderer = &mut renderers[renderer_idx];
                            renderer.config.width = new_size.width.max(1);
                            renderer.config.height = new_size.height.max(1);
                            renderer.surface.configure(&renderer.device, &renderer.config);

                            let logical = new_size.to_logical(renderer.window.scale_factor());
                            renderer.flux.resize(
                                &renderer.device,
                                &renderer.queue,
                                logical.width,
                                logical.height,
                                new_size.width,
                                new_size.height,
                            );
                        }
                        WindowEvent::RedrawRequested => {
                            let renderer = &mut renderers[renderer_idx];
                            let frame = renderer
                                .surface
                                .get_current_texture()
                                .expect("Failed to acquire next swap chain texture");
                            let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                            let mut encoder = renderer.device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor {
                                    label: Some("flux:render"),
                                },
                            );

                            // Use same time for all displays to keep them in sync
                            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                            renderer.flux.animate(
                                &renderer.device,
                                &renderer.queue,
                                &mut encoder,
                                &view,
                                None,
                                elapsed,
                            );

                            renderer.queue.submit(Some(encoder.finish()));
                            renderer.window.pre_present_notify();
                            frame.present();
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
    }).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

#[allow(dead_code)]
async fn run_wallpaper(
    runtime: tokio::runtime::Runtime,
    event_loop: EventLoop<()>,
    window: winit::window::Window,
    args: Args,
    display: DisplayInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    setup_wallpaper_window(&window, &display);

    let window = Arc::new(window);
    let wgpu_instance = wgpu::Instance::default();
    let window_surface = wgpu_instance.create_surface(window.clone()).unwrap();

    let adapter = wgpu_instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: Some(&window_surface),
        })
        .await
        .expect("Failed to find an appropriate adapter");

    let mut limits = wgpu::Limits::default().using_resolution(adapter.limits());
    limits.max_push_constant_size = 8;
    let features = wgpu::Features::PUSH_CONSTANTS
        | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
        | wgpu::Features::FLOAT32_FILTERABLE;

    let (device, command_queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: features,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
        })
        .await
        .expect("Failed to create device");

    let swapchain_capabilities = window_surface.get_capabilities(&adapter);
    let swapchain_format = get_preferred_format(&swapchain_capabilities);

    // Use display dimensions directly rather than relying on inner_size()
    // This ensures we use the correct size even when NSWindow frame differs from winit's view
    let scale_factor = window.scale_factor();
    let physical_width = (display.width * scale_factor) as u32;
    let physical_height = (display.height * scale_factor) as u32;
    let logical_width = display.width as u32;
    let logical_height = display.height as u32;

    log::info!("Display dimensions: {}x{} (logical), {}x{} (physical)",
               logical_width, logical_height, physical_width, physical_height);
    log::info!("Window scale_factor: {}", scale_factor);

    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: physical_width.max(1),
        height: physical_height.max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: swapchain_capabilities.alpha_modes[0],
        view_formats: vec![],
    };

    window_surface.configure(&device, &config);

    let settings = Arc::new(Settings::default());
    let flux = Flux::new(
        &device,
        &command_queue,
        swapchain_format,
        logical_width,
        logical_height,
        physical_width,
        physical_height,
        &Arc::clone(&settings),
    )
    .unwrap();

    window.set_visible(true);

    let (tx, rx) = mpsc::channel(32);
    let mut app = App {
        runtime,
        tx,
        rx,
        flux,
        settings,
        color_image: Arc::new(Mutex::new(None)),
    };

    let start = std::time::Instant::now();
    let target_frame_time = std::time::Duration::from_secs_f64(1.0 / args.fps as f64);
    let mut last_frame = std::time::Instant::now();

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            last_frame + target_frame_time,
        ));

        app.handle_pending_messages(&device, &command_queue);

        match event {
            Event::AboutToWait => {
                let now = std::time::Instant::now();
                if now.duration_since(last_frame) >= target_frame_time {
                    window.request_redraw();
                    last_frame = now;
                }
            }
            Event::WindowEvent { event, window_id } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::KeyboardInput {
                    event: KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyQ),
                        state: ElementState::Released,
                        ..
                    },
                    ..
                } => elwt.exit(),
                WindowEvent::Resized(new_size) => {
                    config.width = new_size.width.max(1);
                    config.height = new_size.height.max(1);
                    window_surface.configure(&device, &config);

                    let logical = new_size.to_logical(window.scale_factor());
                    app.flux.resize(&device, &command_queue, logical.width, logical.height, new_size.width, new_size.height);
                }
                WindowEvent::RedrawRequested => {
                    let frame = window_surface
                        .get_current_texture()
                        .expect("Failed to acquire next swap chain texture");
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("flux:render"),
                    });

                    app.flux.animate(&device, &command_queue, &mut encoder, &view, None, start.elapsed().as_secs_f64() * 1000.0);

                    command_queue.submit(Some(encoder.finish()));
                    window.pre_present_notify();
                    frame.present();
                }
                _ => (),
            },
            _ => (),
        }
    }).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

async fn run_normal(
    runtime: tokio::runtime::Runtime,
    event_loop: EventLoop<()>,
    window: winit::window::Window,
    args: Args,
) -> Result<(), Box<dyn std::error::Error>> {
    let window = Arc::new(window);
    let wgpu_instance = wgpu::Instance::default();
    let window_surface = wgpu_instance.create_surface(window.clone()).unwrap();

    let adapter = wgpu_instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&window_surface),
        })
        .await
        .expect("Failed to find an appropriate adapter");

    let mut limits = wgpu::Limits::default().using_resolution(adapter.limits());
    limits.max_push_constant_size = 8;
    let features = wgpu::Features::PUSH_CONSTANTS
        | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
        | wgpu::Features::FLOAT32_FILTERABLE;

    let (device, command_queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: features,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
        })
        .await
        .expect("Failed to create device");

    let swapchain_capabilities = window_surface.get_capabilities(&adapter);
    let swapchain_format = get_preferred_format(&swapchain_capabilities);

    let physical_size = window.inner_size();
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: physical_size.width,
        height: physical_size.height,
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: swapchain_capabilities.alpha_modes[0],
        view_formats: vec![],
    };

    window_surface.configure(&device, &config);

    let logical_size = physical_size.to_logical(window.scale_factor());
    let settings = Arc::new(Settings::default());
    let flux = Flux::new(
        &device,
        &command_queue,
        swapchain_format,
        logical_size.width,
        logical_size.height,
        physical_size.width,
        physical_size.height,
        &Arc::clone(&settings),
    )
    .unwrap();

    window.set_visible(true);

    let (tx, rx) = mpsc::channel(32);
    let mut app = App {
        runtime,
        tx,
        rx,
        flux,
        settings,
        color_image: Arc::new(Mutex::new(None)),
    };

    let start = std::time::Instant::now();
    let target_frame_time = std::time::Duration::from_secs_f64(1.0 / args.fps as f64);
    let mut last_frame = std::time::Instant::now();

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(winit::event_loop::ControlFlow::Poll);

        app.handle_pending_messages(&device, &command_queue);

        match event {
            Event::AboutToWait => {
                let now = std::time::Instant::now();
                if now.duration_since(last_frame) >= target_frame_time {
                    window.request_redraw();
                    last_frame = now;
                }
            }
            Event::WindowEvent { event, window_id } if window_id == window.id() => match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    event: KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: ElementState::Released,
                        ..
                    },
                    ..
                } => elwt.exit(),
                WindowEvent::DroppedFile(path) => {
                    let bytes = std::fs::read(path).unwrap();
                    app.decode_image(bytes);
                    window.request_redraw();
                }
                WindowEvent::Resized(new_size) => {
                    config.width = new_size.width.max(1);
                    config.height = new_size.height.max(1);
                    window_surface.configure(&device, &config);

                    let logical = new_size.to_logical(window.scale_factor());
                    app.flux.resize(&device, &command_queue, logical.width, logical.height, new_size.width, new_size.height);
                    window.request_redraw();
                }
                WindowEvent::RedrawRequested => {
                    let frame = window_surface
                        .get_current_texture()
                        .expect("Failed to acquire next swap chain texture");
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("flux:render"),
                    });

                    app.flux.animate(&device, &command_queue, &mut encoder, &view, None, start.elapsed().as_secs_f64() * 1000.0);

                    command_queue.submit(Some(encoder.finish()));
                    window.pre_present_notify();
                    frame.present();
                }
                _ => (),
            },
            _ => (),
        }
    }).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

fn get_preferred_format(capabilities: &wgpu::SurfaceCapabilities) -> wgpu::TextureFormat {
    let preferred_formats = [
        wgpu::TextureFormat::Rgb10a2Unorm,
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ];

    for format in &preferred_formats {
        if capabilities.formats.contains(format) {
            return *format;
        }
    }

    capabilities.formats[0]
}
