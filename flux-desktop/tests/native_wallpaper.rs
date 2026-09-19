#![allow(unexpected_cfgs, dead_code)]
// Run on a logged-in desktop with DRIFTPAPER_NATIVE_TESTS=1. A custom harness
// keeps AppKit/winit on the main thread and never needs a GPU or saved settings.
#[path = "../src/displays.rs"]
mod displays;
#[path = "../src/wallpaper.rs"]
mod wallpaper;

fn main() {
    if std::env::var_os("DRIFTPAPER_NATIVE_TESTS").is_none() {
        println!(
            "Native wallpaper checks skipped; set DRIFTPAPER_NATIVE_TESTS=1 on a desktop session"
        );
        return;
    }
    let mut builder = winit::event_loop::EventLoopBuilder::new();
    wallpaper::configure_event_loop(&mut builder, true);
    let event_loop = builder.build().unwrap();
    let window = wallpaper::window_builder(true)
        .with_title("DriftPaper native test")
        .build(&event_loop)
        .unwrap();
    let preview = wallpaper::window_builder(false).build(&event_loop).unwrap();
    let display = displays::DisplayInfo {
        id: "test".into(),
        origin_x: 0.0,
        origin_y: 0.0,
        width: 100.0,
        height: 100.0,
        pixels_wide: 100,
        pixels_high: 100,
    };
    wallpaper::setup_wallpaper_window(&window, &display).unwrap();
    wallpaper::show_wallpaper_window(&window);
    event_loop
        .run(move |event, target| {
            if matches!(event, winit::event::Event::AboutToWait) {
                check_native_window(&window);
                check_preview_window(&preview);
                // Reapplying policy after monitor changes must remain safe.
                wallpaper::setup_wallpaper_window(&window, &display).unwrap();
                wallpaper::show_wallpaper_window(&window);
                check_native_window(&window);
                target.exit();
            }
        })
        .unwrap();
    println!("Native wallpaper checks passed");
}

#[cfg(target_os = "macos")]
fn check_native_window(window: &winit::window::Window) {
    use cocoa::base::{id, nil, NO};
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let RawWindowHandle::AppKit(handle) = window.window_handle().unwrap().as_raw() else {
        panic!("Expected AppKit window")
    };
    unsafe {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let policy: isize = msg_send![app, activationPolicy];
        assert_eq!(
            policy, 1,
            "Wallpaper became a regular Dock application at launch"
        );
        let view = handle.ns_view.as_ptr() as id;
        let native: id = msg_send![view, window];
        let ignores: bool = msg_send![native, ignoresMouseEvents];
        assert!(ignores, "Wallpaper must pass all mouse buttons through");
        let focus: bool = msg_send![native, canBecomeKeyWindow];
        assert!(!focus, "Wallpaper must never take keyboard focus");
        let can_hide: bool = msg_send![native, canHide];
        assert!(!can_hide, "Hide/Hide Others must not hide the wallpaper");
        extern "C" {
            fn CGWindowLevelForKey(key: i32) -> i32;
        }
        let level: isize = msg_send![native, level];
        assert!(level > CGWindowLevelForKey(2) as isize);
        assert!(level < CGWindowLevelForKey(18) as isize);
        let behavior: usize = msg_send![native, collectionBehavior];
        assert_eq!(
            behavior & (1 | 16 | 64),
            1 | 16 | 64,
            "Wallpaper must join Spaces, stay stationary, and skip window cycling"
        );
        let movable: bool = msg_send![native, isMovable];
        assert!(!movable);
        // Exercise native action paths as well as inspecting the window policy.
        let _: () = msg_send![native, miniaturize: nil];
        let minimized: bool = msg_send![native, isMiniaturized];
        if minimized {
            let _: () = msg_send![native, deminiaturize: nil];
            let _: () = msg_send![native, orderOut: nil];
        }
        assert!(!minimized, "Wallpaper accepted a native minimize command");
        let _: () = msg_send![native, performMiniaturize: nil];
        let minimized: bool = msg_send![native, isMiniaturized];
        assert!(!minimized, "Wallpaper accepted the minimize menu action");
        let main: objc::runtime::BOOL = msg_send![native, canBecomeMainWindow];
        assert_eq!(main, NO);
    }
}

#[cfg(target_os = "macos")]
fn check_preview_window(window: &winit::window::Window) {
    use cocoa::base::id;
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let RawWindowHandle::AppKit(handle) = window.window_handle().unwrap().as_raw() else {
        panic!("Expected AppKit window")
    };
    unsafe {
        let native: id = msg_send![handle.ns_view.as_ptr() as id, window];
        let focus: bool = msg_send![native, canBecomeKeyWindow];
        let ignores: bool = msg_send![native, ignoresMouseEvents];
        let minimizable: bool = msg_send![native, isMiniaturizable];
        assert!(
            focus && !ignores && minimizable,
            "Preview must remain a normal window"
        );
    }
}

#[cfg(target_os = "windows")]
fn check_preview_window(window: &winit::window::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else {
        panic!("Expected Win32 window")
    };
    unsafe {
        assert_eq!(GetParent(handle.hwnd.get()), 0);
        let style = GetWindowLongPtrA(handle.hwnd.get(), GWL_STYLE) as u32;
        assert_ne!(style & WS_MINIMIZEBOX, 0);
        let ex_style = GetWindowLongPtrA(handle.hwnd.get(), GWL_EXSTYLE) as u32;
        assert_eq!(ex_style & (WS_EX_TRANSPARENT | WS_EX_NOACTIVATE), 0);
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn check_preview_window(_: &winit::window::Window) {}

#[cfg(target_os = "windows")]
fn check_native_window(window: &winit::window::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else {
        panic!("Expected Win32 window")
    };
    unsafe {
        let hwnd = handle.hwnd.get();
        assert_ne!(GetParent(hwnd), 0, "Wallpaper must be attached to Explorer");
        let style = GetWindowLongPtrA(hwnd, GWL_STYLE) as u32;
        let ex_style = GetWindowLongPtrA(hwnd, GWL_EXSTYLE) as u32;
        assert_ne!(style & WS_CHILD, 0);
        assert_eq!(
            style & (WS_POPUP | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU),
            0
        );
        assert_ne!(
            ex_style & WS_EX_TRANSPARENT,
            0,
            "Mouse input must pass through"
        );
        assert_ne!(ex_style & WS_EX_LAYERED, 0);
        assert_ne!(ex_style & WS_EX_NOACTIVATE, 0);
        assert_eq!(ex_style & WS_EX_APPWINDOW, 0);
        SendMessageA(hwnd, WM_SYSCOMMAND, SC_MINIMIZE as usize, 0);
        assert_eq!(IsIconic(hwnd), 0, "Wallpaper accepted SC_MINIMIZE");
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn check_native_window(_: &winit::window::Window) {
    panic!("Native wallpaper checks require macOS or Windows");
}
