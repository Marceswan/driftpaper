#![allow(unexpected_cfgs)] // objc 0.2 emits obsolete cargo-clippy cfg checks.
use crate::*;

#[cfg(target_os = "macos")]
pub(crate) fn setup_wallpaper_window(window: &Window, display: &DisplayInfo) {
    use cocoa::{
        appkit::{NSWindow, NSWindowCollectionBehavior},
        base::{id, NO, YES},
        foundation::{NSPoint, NSRect, NSSize},
    };
    use objc::{
        declare::ClassDecl,
        msg_send,
        runtime::{Class, Object, Sel, BOOL},
        sel, sel_impl,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    extern "C" fn cannot_focus(_: &Object, _: Sel) -> BOOL {
        NO
    }
    let Ok(handle) = window.window_handle() else {
        return;
    };
    if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
        unsafe {
            let view = handle.ns_view.as_ptr() as id;
            let native: id = msg_send![view, window];
            // Only wallpaper windows get this subclass. Do not swizzle WinitView,
            // which would also change event routing for previews and file dialogs.
            let wallpaper_class = Class::get("DriftDesktopWindow").unwrap_or_else(|| {
                let superclass: &Class = (&*native).class();
                let mut class = ClassDecl::new("DriftDesktopWindow", superclass).unwrap();
                class.add_method(
                    sel!(canBecomeKeyWindow),
                    cannot_focus as extern "C" fn(&Object, Sel) -> BOOL,
                );
                class.add_method(
                    sel!(canBecomeMainWindow),
                    cannot_focus as extern "C" fn(&Object, Sel) -> BOOL,
                );
                class.register()
            });
            // objc 0.2 does not expose this Objective-C runtime entry point.
            extern "C" {
                fn object_setClass(object: *mut Object, class: *const Class) -> *const Class;
            }
            object_setClass(native, wallpaper_class);
            let _: () = msg_send![native, setStyleMask: 0u64];
            let _: () = msg_send![native, setHasShadow: NO];
            let _: () = msg_send![native, setHidesOnDeactivate: NO];
            let _: () = msg_send![native, setReleasedWhenClosed: NO];
            let _: () = msg_send![native, setLevel: -2147483623i64];
            let behavior = NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle;
            native.setCollectionBehavior_(behavior);
            // NSWindow ignores every mouse button, including secondary/control-click,
            // allowing WindowServer to route the original event to Finder below it.
            let _: () = msg_send![native, setIgnoresMouseEvents: YES];
            let _: () = msg_send![native, setAcceptsMouseMovedEvents: NO];
            let _: () = msg_send![native, setExcludedFromWindowsMenu: YES];
            let _: () = msg_send![native, setFrame: NSRect::new(NSPoint::new(display.origin_x, display.origin_y), NSSize::new(display.width, display.height)) display: YES];
            let _: () = msg_send![native, orderBack: std::ptr::null::<Object>()];
            let ignores: BOOL = msg_send![native, ignoresMouseEvents];
            let can_focus: BOOL = msg_send![native, canBecomeKeyWindow];
            log::debug!(
                "Desktop input policy: ignores_mouse={}, can_focus={}",
                ignores != NO,
                can_focus != NO
            );
        }
    }
    if let Err(err) = window.set_cursor_hittest(false) {
        log::warn!("Cannot enable desktop click-through: {err}");
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn get_all_displays() -> Vec<DisplayInfo> {
    use cocoa::appkit::NSScreen;
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSRect;
    use cocoa::foundation::NSString;
    use objc::{msg_send, sel, sel_impl};

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

            log::debug!(
                "NSScreen {}: frame=({}, {}, {}x{}), visible=({}, {}, {}x{}), scale={}, pixels={}x{}",
                i,
                frame.origin.x, frame.origin.y,
                frame.size.width, frame.size.height,
                visible_frame.origin.x, visible_frame.origin.y,
                visible_frame.size.width, visible_frame.size.height,
                scale,
                pixels_wide, pixels_high
            );

            let description: id = msg_send![screen, deviceDescription];
            let key = cocoa::foundation::NSString::alloc(nil).init_str("NSScreenNumber");
            let number: id = msg_send![description, objectForKey: key];
            let display_id: u32 = msg_send![number, unsignedIntValue];
            let _: () = msg_send![key, release];
            displays.push(DisplayInfo {
                id: display_id.to_string(),
                origin_x: frame.origin.x,
                origin_y: frame.origin.y,
                width: frame.size.width,
                height: frame.size.height,
                pixels_wide,
                pixels_high,
            });
        }
    }

    displays
}

// ==================== Windows Implementation ====================

#[cfg(target_os = "windows")]
mod windows_wallpaper {
    use std::ptr::null;
    use windows_sys::Win32::Foundation::{BOOL, FALSE, HWND, LPARAM, TRUE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowA, FindWindowExA, GetWindowLongPtrA, SendMessageTimeoutA,
        SetLayeredWindowAttributes, SetParent, SetWindowLongPtrA, SetWindowPos, ShowWindow,
        GWL_EXSTYLE, GWL_STYLE, LWA_ALPHA, SMTO_NORMAL, SWP_FRAMECHANGED, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SW_SHOWNA, WS_CHILD, WS_DISABLED, WS_EX_LAYERED, WS_POPUP,
    };

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
                log::info!(
                    "Found WorkerW {:?} (sibling after SHELLDLL_DefView parent)",
                    worker
                );
                return FALSE; // Stop enumeration
            }
        }
        TRUE // Continue enumeration
    }

    pub unsafe fn setup_as_wallpaper(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
        // Find Progman window
        let progman = FindWindowA(b"Progman\0".as_ptr(), null());
        if progman == 0 as HWND {
            log::error!("Failed to find Progman window");
            return;
        }
        log::info!("Found Progman window: {:?}", progman);

        // Send message to spawn/prepare WorkerW
        let mut result: usize = 0;
        SendMessageTimeoutA(
            progman,
            0x052C,
            0xD,
            0x1,
            SMTO_NORMAL,
            1000,
            &mut result as *mut usize,
        );
        log::info!("SendMessage 0x052C result: {}", result);

        // Small delay to let Windows process the message
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Find SHELLDLL_DefView and WorkerW
        let shell_view = FindWindowExA(progman, 0 as HWND, b"SHELLDLL_DefView\0".as_ptr(), null());
        WORKERW = 0 as HWND;
        EnumWindows(Some(enum_windows_proc), 0);

        if shell_view != 0 as HWND {
            // Windows 11 style: SHELLDLL_DefView is in Progman
            // We parent to Progman and position below shell_view
            log::info!("Using Windows 11 style injection (SHELLDLL_DefView in Progman)");
            let worker = WORKERW;
            log::debug!("shell_view: {:?}, workerw: {:?}", shell_view, worker);

            // Step 1: Modify window style - remove popup/disabled, add child
            let style = GetWindowLongPtrA(hwnd, GWL_STYLE);
            let new_style =
                (style & !(WS_POPUP as isize) & !(WS_DISABLED as isize)) | WS_CHILD as isize;
            SetWindowLongPtrA(hwnd, GWL_STYLE, new_style);
            log::info!("Style: {:X} -> {:X}", style, new_style);

            // Step 2: Parent to Progman
            let old_parent = SetParent(hwnd, progman);
            log::info!("SetParent to Progman, old parent: {:?}", old_parent);

            // Step 3: Make layered and set alpha
            let ex_style = GetWindowLongPtrA(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrA(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED as isize);
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);

            let mut origin = windows_sys::Win32::Foundation::POINT { x, y };
            windows_sys::Win32::Graphics::Gdi::MapWindowPoints(0, progman, &mut origin, 1);
            // Step 4: Position BELOW shell_view (this is the key!)
            // Using shell_view as hWndInsertAfter places our window directly below it in z-order
            let pos_result = SetWindowPos(
                hwnd,
                shell_view, // Insert after (below) shell_view
                origin.x,
                origin.y,
                width,
                height,
                SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
            log::info!("SetWindowPos below shell_view result: {}", pos_result);

            // Step 5: If we have WorkerW, reorder it above our window
            if WORKERW != 0 as HWND {
                SetWindowPos(
                    WORKERW,
                    hwnd, // Insert after (below) our window
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
            let new_style =
                (style & !(WS_POPUP as isize) & !(WS_DISABLED as isize)) | WS_CHILD as isize;
            SetWindowLongPtrA(hwnd, GWL_STYLE, new_style);

            SetParent(hwnd, WORKERW);

            let ex_style = GetWindowLongPtrA(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrA(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED as isize);
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);

            let mut origin = windows_sys::Win32::Foundation::POINT { x, y };
            windows_sys::Win32::Graphics::Gdi::MapWindowPoints(0, WORKERW, &mut origin, 1);
            SetWindowPos(
                hwnd,
                0 as HWND,
                origin.x,
                origin.y,
                width,
                height,
                SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
            ShowWindow(hwnd, SW_SHOWNA);
            log::info!("Window shown as wallpaper (WorkerW parent)");
        } else {
            log::error!("Could not find shell_view or WorkerW - wallpaper injection failed");
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn setup_wallpaper_window(window: &Window, display: &DisplayInfo) {
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
pub(crate) fn get_all_displays() -> Vec<DisplayInfo> {
    use windows_sys::Win32::Foundation::{BOOL, LPARAM, RECT, TRUE};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoA, HDC, HMONITOR, MONITORINFO,
    };

    let mut displays = Vec::new();

    unsafe extern "system" fn monitor_enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _lprect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let displays = &mut *(lparam as *mut Vec<DisplayInfo>);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;

        if GetMonitorInfoA(hmonitor, &mut info) != 0 {
            let rect = info.rcMonitor;
            let width = (rect.right - rect.left) as f64;
            let height = (rect.bottom - rect.top) as f64;

            displays.push(DisplayInfo {
                id: format!("{:x}", hmonitor as usize),
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
        EnumDisplayMonitors(
            0 as HDC,
            std::ptr::null(),
            Some(monitor_enum_proc),
            &mut displays as *mut Vec<DisplayInfo> as LPARAM,
        );

        displays
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn setup_wallpaper_window(_window: &Window, _display: &DisplayInfo) {
    log::warn!("Wallpaper mode is only supported on macOS and Windows");
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn get_all_displays() -> Vec<DisplayInfo> {
    vec![DisplayInfo {
        id: "default".into(),
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

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>me.sandydoo.driftpaper</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#,
        exe_path
    );

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
pub(crate) fn setup_screen_change_observer() {
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSAutoreleasePool, NSString};
    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};

    extern "C" fn screen_did_change(_this: &Object, _cmd: Sel, _notification: id) {
        log::info!("Screen configuration changed - will reinitialize displays");
        SCREEN_CONFIG_CHANGED.store(true, Ordering::SeqCst);
        wake();
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
        let notification_name =
            NSString::alloc(nil).init_str("NSApplicationDidChangeScreenParametersNotification");

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
pub(crate) fn setup_screen_change_observer() {
    log::warn!("Screen change observer is only supported on macOS");
}

/// Setup macOS menu bar item for wallpaper control
#[cfg(target_os = "macos")]
pub(crate) fn setup_menu_bar() {
    use cocoa::appkit::{NSMenu, NSMenuItem, NSStatusBar};
    use cocoa::base::{id, nil, selector, NO, YES};
    use cocoa::foundation::{NSAutoreleasePool, NSString};
    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel, BOOL};
    use objc::{class, msg_send, sel, sel_impl};

    // Action handlers
    extern "C" fn quit_action(_this: &Object, _cmd: Sel, _sender: id) {
        log::info!("Quit requested from menu bar");
        SHOULD_QUIT.store(true, Ordering::SeqCst);
        wake();
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

    extern "C" fn set_color_custom_image(_this: &Object, _cmd: Sel, _sender: id) {
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
                        update_preferences(|prefs| {
                            prefs.color_scheme = 4;
                            prefs.custom_color_wheel = Some(wheel);
                            prefs.custom_image_path = Some(path.to_string_lossy().to_string());
                        });
                        CURRENT_COLOR_SCHEME.store(4, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        wake();
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
    }

    fn set_color_scheme(scheme: u32, sender: id) {
        log::info!("Setting color scheme to: {}", scheme);
        CURRENT_COLOR_SCHEME.store(scheme, Ordering::SeqCst);
        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
        wake();
        // Save preference
        update_preferences(|prefs| {
            prefs.color_scheme = scheme;
        });
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
        wake();
        // Save preference
        update_preferences(|prefs| {
            prefs.density = density;
        });
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
        wake();
        // Save preference
        update_preferences(|prefs| {
            prefs.noise_strength = strength;
        });
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
        wake();
        update_preferences(|prefs| {
            prefs.line_length = length;
        });
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
        wake();
        update_preferences(|prefs| {
            prefs.line_width = width;
        });
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
        wake();
        update_preferences(|prefs| {
            prefs.view_scale = scale;
        });
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
        wake();
        update_preferences(|prefs| {
            prefs.brightness = brightness;
        });
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

    extern "C" fn set_fps(_this: &Object, _cmd: Sel, sender: id) {
        unsafe {
            let fps: i64 = msg_send![sender, tag];
            CURRENT_FPS.store(fps as u32, Ordering::SeqCst);
            update_preferences(|prefs| prefs.fps = fps as u32);
            wake();
            let menu: id = msg_send![sender, menu];
            let count: i64 = msg_send![menu, numberOfItems];
            for index in 0..count {
                let item: id = msg_send![menu, itemAtIndex: index];
                let tag: i64 = msg_send![item, tag];
                let _: () = msg_send![item, setState: if tag == fps { 1i64 } else { 0i64 }];
            }
        }
    }

    // Delegate method to update menu when opened
    extern "C" fn menu_will_open(_this: &Object, _cmd: Sel, menu: id) {
        // Reflect committed state, including asynchronous custom image selection.
        unsafe {
            let custom: id = msg_send![menu, itemWithTag: 4i64];
            let count: i64 = msg_send![menu, numberOfItems];
            if custom != nil && count == 5 {
                for index in 0..count {
                    let item: id = msg_send![menu, itemAtIndex: index];
                    let tag: i64 = msg_send![item, tag];
                    let state = if tag == CURRENT_COLOR_SCHEME.load(Ordering::SeqCst) as i64 {
                        1i64
                    } else {
                        0i64
                    };
                    let _: () = msg_send![item, setState: state];
                }
            }
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
            decl.add_method(sel!(setFps:), set_fps as extern "C" fn(&Object, Sel, id));
            decl.add_method(
                sel!(quitAction:),
                quit_action as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(toggleLoginAction:),
                toggle_login_action as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setColorOriginal:),
                set_color_original as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setColorPlasma:),
                set_color_plasma as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setColorPoolside:),
                set_color_poolside as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setColorSpacegrey:),
                set_color_spacegrey as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setColorCustomImage:),
                set_color_custom_image as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setDensitySparse:),
                set_density_sparse as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setDensityNormal:),
                set_density_normal as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setDensityDense:),
                set_density_dense as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setNoiseLow:),
                set_noise_low as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setNoiseMedium:),
                set_noise_medium as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setNoiseHigh:),
                set_noise_high as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setNoiseMax:),
                set_noise_max as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setLineShort:),
                set_line_short as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setLineMedium:),
                set_line_medium as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setLineLong:),
                set_line_long as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setLineExtraLong:),
                set_line_extra_long as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setWidthThin:),
                set_width_thin as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setWidthMedium:),
                set_width_medium as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setWidthThick:),
                set_width_thick as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setScaleCompact:),
                set_scale_compact as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setScaleNormal:),
                set_scale_normal as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setScaleWide:),
                set_scale_wide as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setBrightnessDim:),
                set_brightness_dim as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setBrightnessNormal:),
                set_brightness_normal as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setBrightnessBright:),
                set_brightness_bright as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(setBrightnessVivid:),
                set_brightness_vivid as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(menuWillOpen:),
                menu_will_open as extern "C" fn(&Object, Sel, id),
            );
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
        let _: () = msg_send![color_menu, setDelegate: handler];
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
            log::info!(
                "Color item '{}': enabled={}, has_target={}, action={:?}",
                name,
                is_enabled != NO,
                target != nil,
                item_action
            );

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

        for (i, (name, action)) in density_names
            .iter()
            .zip(density_selectors.iter())
            .enumerate()
        {
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

        for (i, (name, action)) in line_length_names
            .iter()
            .zip(line_length_selectors.iter())
            .enumerate()
        {
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

        for (i, (name, action)) in line_width_names
            .iter()
            .zip(line_width_selectors.iter())
            .enumerate()
        {
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

        for (i, (name, action)) in view_scale_names
            .iter()
            .zip(view_scale_selectors.iter())
            .enumerate()
        {
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

        for (i, (name, action)) in brightness_names
            .iter()
            .zip(brightness_selectors.iter())
            .enumerate()
        {
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

        // ===== Frame Rate =====
        let fps_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            NSString::alloc(nil).init_str("Frame Rate"),
            selector(""),
            NSString::alloc(nil).init_str(""),
        );
        let fps_menu = NSMenu::new(nil).autorelease();
        let _: () = msg_send![fps_menu, setAutoenablesItems: NO];
        let mut rates = vec![15u32, 30, 60];
        let current_fps = CURRENT_FPS.load(Ordering::SeqCst);
        if !rates.contains(&current_fps) {
            rates.push(current_fps);
            rates.sort();
        }
        for fps in rates {
            let item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str(&format!("{fps} FPS")),
                sel!(setFps:),
                NSString::alloc(nil).init_str(""),
            );
            let _: () = msg_send![item, setTarget: handler];
            let _: () = msg_send![item, setTag: fps as i64];
            let _: () = msg_send![item, setState: if fps == current_fps { 1i64 } else { 0i64 }];
            fps_menu.addItem_(item);
        }
        let _: () = msg_send![fps_item, setSubmenu: fps_menu];
        menu.addItem_(fps_item);

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
        STATUS_ITEM = status_item;

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
    if let Ok(key) = hkcu.open_subkey_with_flags(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
        KEY_SET_VALUE | KEY_QUERY_VALUE,
    ) {
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
pub(crate) fn setup_menu_bar() -> Option<tray_icon::TrayIcon> {
    use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
    use tray_icon::{Icon, TrayIconBuilder};

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

    let fps_submenu = Submenu::new("Frame Rate", true);
    let mut fps_values = vec![15u32, 30, 60];
    let current_fps = CURRENT_FPS.load(Ordering::SeqCst);
    if !fps_values.contains(&current_fps) {
        fps_values.push(current_fps);
        fps_values.sort();
    }
    let fps_items: Vec<_> = fps_values
        .iter()
        .map(|&fps| CheckMenuItem::new(format!("{fps} FPS"), true, fps == current_fps, None))
        .collect();
    for item in &fps_items {
        let _ = fps_submenu.append(item);
    }
    let _ = menu.append(&fps_submenu);

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

    MENU_GROUPS.with(|groups| {
        *groups.borrow_mut() = vec![
            (
                vec![
                    color_original.clone(),
                    color_plasma.clone(),
                    color_poolside.clone(),
                    color_spacegrey.clone(),
                    color_custom.clone(),
                ],
                &CURRENT_COLOR_SCHEME,
                vec![0, 1, 2, 3, 4],
            ),
            (
                vec![
                    density_sparse.clone(),
                    density_normal.clone(),
                    density_dense.clone(),
                ],
                &CURRENT_DENSITY,
                vec![0, 1, 2],
            ),
            (
                vec![
                    noise_low.clone(),
                    noise_medium.clone(),
                    noise_high.clone(),
                    noise_max.clone(),
                ],
                &CURRENT_NOISE_STRENGTH,
                vec![0, 1, 2, 3],
            ),
            (
                vec![
                    length_short.clone(),
                    length_medium.clone(),
                    length_long.clone(),
                    length_extra.clone(),
                ],
                &CURRENT_LINE_LENGTH,
                vec![0, 1, 2, 3],
            ),
            (
                vec![
                    width_thin.clone(),
                    width_medium.clone(),
                    width_thick.clone(),
                ],
                &CURRENT_LINE_WIDTH,
                vec![0, 1, 2],
            ),
            (
                vec![
                    scale_compact.clone(),
                    scale_normal.clone(),
                    scale_wide.clone(),
                ],
                &CURRENT_VIEW_SCALE,
                vec![0, 1, 2],
            ),
            (
                vec![
                    brightness_dim.clone(),
                    brightness_normal.clone(),
                    brightness_bright.clone(),
                    brightness_vivid.clone(),
                ],
                &CURRENT_BRIGHTNESS,
                vec![0, 1, 2, 3],
            ),
            (fps_items.clone(), &CURRENT_FPS, fps_values.clone()),
        ];
    });
    let fps_ids: Vec<_> = fps_items.iter().map(|item| item.id().0.clone()).collect();

    // Extract string IDs before spawning thread (MenuId contains Rc which is not Send)
    let color_ids: Vec<String> = [
        &color_original,
        &color_plasma,
        &color_poolside,
        &color_spacegrey,
        &color_custom,
    ]
    .iter()
    .map(|item| item.id().0.clone())
    .collect();
    let density_ids: Vec<String> = [&density_sparse, &density_normal, &density_dense]
        .iter()
        .map(|item| item.id().0.clone())
        .collect();
    let noise_ids: Vec<String> = [&noise_low, &noise_medium, &noise_high, &noise_max]
        .iter()
        .map(|item| item.id().0.clone())
        .collect();
    let length_ids: Vec<String> = [&length_short, &length_medium, &length_long, &length_extra]
        .iter()
        .map(|item| item.id().0.clone())
        .collect();
    let width_ids: Vec<String> = [&width_thin, &width_medium, &width_thick]
        .iter()
        .map(|item| item.id().0.clone())
        .collect();
    let scale_ids: Vec<String> = [&scale_compact, &scale_normal, &scale_wide]
        .iter()
        .map(|item| item.id().0.clone())
        .collect();
    let brightness_ids: Vec<String> = [
        &brightness_dim,
        &brightness_normal,
        &brightness_bright,
        &brightness_vivid,
    ]
    .iter()
    .map(|item| item.id().0.clone())
    .collect();
    let run_on_login_id_str = run_on_login_item.id().0.clone();
    let quit_id_str = quit_id.0.clone();

    // Spawn thread to handle menu events
    std::thread::spawn(move || {
        use muda::MenuEvent;
        let menu_channel = MenuEvent::receiver();

        loop {
            if let Ok(event) = menu_channel.recv() {
                let id_str = &event.id.0;

                for (id, fps) in fps_ids.iter().zip(&fps_values) {
                    if id == id_str {
                        CURRENT_FPS.store(*fps, Ordering::SeqCst);
                        update_preferences(|prefs| prefs.fps = *fps);
                        wake();
                    }
                }
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
                                        update_preferences(|prefs| {
                                            prefs.color_scheme = 4;
                                            prefs.custom_color_wheel = Some(wheel);
                                            prefs.custom_image_path =
                                                Some(path.to_string_lossy().to_string());
                                        });
                                        CURRENT_COLOR_SCHEME.store(4, Ordering::SeqCst);
                                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                                        wake();
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
                            wake();
                            update_preferences(|prefs| {
                                prefs.color_scheme = i as u32;
                            });
                        }
                        log::info!("Color scheme changed to {}", i);
                    }
                }

                // Check density
                for (i, density_id) in density_ids.iter().enumerate() {
                    if id_str == density_id {
                        CURRENT_DENSITY.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        wake();
                        update_preferences(|prefs| {
                            prefs.density = i as u32;
                        });
                        log::info!("Density changed to {}", i);
                    }
                }

                // Check noise
                for (i, noise_id) in noise_ids.iter().enumerate() {
                    if id_str == noise_id {
                        CURRENT_NOISE_STRENGTH.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        wake();
                        update_preferences(|prefs| {
                            prefs.noise_strength = i as u32;
                        });
                        log::info!("Noise strength changed to {}", i);
                    }
                }

                // Check line length
                for (i, length_id) in length_ids.iter().enumerate() {
                    if id_str == length_id {
                        CURRENT_LINE_LENGTH.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        wake();
                        update_preferences(|prefs| {
                            prefs.line_length = i as u32;
                        });
                        log::info!("Line length changed to {}", i);
                    }
                }

                // Check line width
                for (i, width_id) in width_ids.iter().enumerate() {
                    if id_str == width_id {
                        CURRENT_LINE_WIDTH.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        wake();
                        update_preferences(|prefs| {
                            prefs.line_width = i as u32;
                        });
                        log::info!("Line width changed to {}", i);
                    }
                }

                // Check view scale
                for (i, scale_id) in scale_ids.iter().enumerate() {
                    if id_str == scale_id {
                        CURRENT_VIEW_SCALE.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        wake();
                        update_preferences(|prefs| {
                            prefs.view_scale = i as u32;
                        });
                        log::info!("View scale changed to {}", i);
                    }
                }

                // Check brightness
                for (i, brightness_id) in brightness_ids.iter().enumerate() {
                    if id_str == brightness_id {
                        CURRENT_BRIGHTNESS.store(i as u32, Ordering::SeqCst);
                        SETTINGS_CHANGED.store(true, Ordering::SeqCst);
                        wake();
                        update_preferences(|prefs| {
                            prefs.brightness = i as u32;
                        });
                        log::info!("Brightness changed to {}", i);
                    }
                }

                // Check run on login toggle
                if id_str == &run_on_login_id_str {
                    // Toggle the current state
                    let currently_enabled = is_run_on_login_enabled();
                    set_run_on_login(!currently_enabled);
                    update_preferences(|prefs| {
                        prefs.run_on_login = !currently_enabled;
                    });
                    log::info!("Run on login toggled to {}", !currently_enabled);
                }

                // Check quit
                if id_str == &quit_id_str {
                    log::info!("Quit requested from tray");
                    SHOULD_QUIT.store(true, Ordering::SeqCst);
                    wake();
                    break;
                }
            }
        }
    });

    Some(tray_icon)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn setup_menu_bar() {
    log::warn!("System tray is only supported on macOS and Windows");
}

#[cfg(target_os = "windows")]
type MenuGroup = (Vec<muda::CheckMenuItem>, &'static AtomicU32, Vec<u32>);
#[cfg(target_os = "windows")]
thread_local! { static MENU_GROUPS: std::cell::RefCell<Vec<MenuGroup>> = const { std::cell::RefCell::new(Vec::new()) }; }

pub(crate) fn sync_menu_state() {
    #[cfg(target_os = "windows")]
    MENU_GROUPS.with(|groups| {
        for (items, current, values) in groups.borrow().iter() {
            for (item, value) in items.iter().zip(values) {
                let checked = current.load(Ordering::SeqCst) == *value;
                if item.is_checked() != checked {
                    item.set_checked(checked);
                }
            }
        }
    });
}
