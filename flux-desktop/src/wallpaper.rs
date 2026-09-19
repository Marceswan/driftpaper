#![allow(unexpected_cfgs)]
use crate::displays::DisplayInfo;
use winit::window::{Window, WindowBuilder, WindowButtons, WindowLevel};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(crate) fn window_builder(wallpaper: bool) -> WindowBuilder {
    let builder = WindowBuilder::new()
        .with_title("DriftPaper")
        .with_visible(false)
        .with_decorations(!wallpaper)
        .with_resizable(!wallpaper)
        .with_active(!wallpaper)
        .with_enabled_buttons(if wallpaper {
            WindowButtons::empty()
        } else {
            WindowButtons::all()
        });
    if !wallpaper {
        return builder;
    }
    let builder = builder.with_window_level(WindowLevel::AlwaysOnBottom);
    #[cfg(target_os = "macos")]
    let builder = {
        use winit::platform::macos::WindowBuilderExtMacOS;
        builder.with_accepts_first_mouse(false)
    };
    #[cfg(target_os = "windows")]
    let builder = {
        use winit::platform::windows::WindowBuilderExtWindows;
        builder.with_skip_taskbar(true).with_drag_and_drop(false)
    };
    builder
}

pub(crate) fn configure_event_loop<T>(
    _builder: &mut winit::event_loop::EventLoopBuilder<T>,
    _wallpaper: bool,
) {
    #[cfg(target_os = "macos")]
    if _wallpaper {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        // winit applies this when NSApplication launches, after menu setup.
        _builder
            .with_activation_policy(ActivationPolicy::Accessory)
            .with_activate_ignoring_other_apps(false)
            .with_default_menu(false);
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn setup_wallpaper_window(window: &Window, display: &DisplayInfo) -> Result<()> {
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
    extern "C" fn ignore_window_action(_: &Object, _: Sel, _: id) {}
    let handle = window.window_handle()?;
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
                for selector in [sel!(miniaturize:), sel!(performMiniaturize:)] {
                    class.add_method(
                        selector,
                        ignore_window_action as extern "C" fn(&Object, Sel, id),
                    );
                }
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
            let _: () = msg_send![native, setCanHide: NO];
            let _: () = msg_send![native, setMovable: NO];
            let _: () = msg_send![native, setAnimationBehavior: 2isize]; // NSWindowAnimationBehaviorNone
            let _: () = msg_send![native, setReleasedWhenClosed: NO];
            // CoreGraphics keys: desktopWindow = 2, desktopIconWindow = 18.
            // Stay above the system background and strictly below Finder icons.
            extern "C" {
                fn CGWindowLevelForKey(key: i32) -> i32;
            }
            let level = CGWindowLevelForKey(2) + 1;
            let _: () = msg_send![native, setLevel: level as isize];
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
            let ignores: BOOL = msg_send![native, ignoresMouseEvents];
            let can_focus: BOOL = msg_send![native, canBecomeKeyWindow];
            log::debug!(
                "Desktop input policy: ignores_mouse={}, can_focus={}",
                ignores != NO,
                can_focus != NO
            );
        }
    } else {
        return Err("Expected an AppKit wallpaper window".into());
    }
    window.set_cursor_hittest(false)?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn show_wallpaper_window(window: &Window) {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::AppKit(handle) = handle.as_raw() {
            unsafe {
                let native: id = msg_send![handle.ns_view.as_ptr() as id, window];
                // winit's set_visible(true) calls makeKeyAndOrderFront.
                let _: () = msg_send![native, orderBack: nil];
            }
        }
    }
}

// ==================== Windows Implementation ====================

#[cfg(target_os = "windows")]
mod windows_wallpaper {
    use super::Result;
    use std::{io, ptr::null};
    use windows_sys::Win32::{
        Foundation::{BOOL, FALSE, HWND, LPARAM, LRESULT, POINT, TRUE, WPARAM},
        Graphics::Gdi::MapWindowPoints,
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    };

    unsafe extern "system" fn desktop_policy(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        id: usize,
        _: usize,
    ) -> LRESULT {
        match message {
            WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
            WM_SYSCOMMAND => match (wparam as u32) & 0xfff0 {
                SC_MINIMIZE | SC_MAXIMIZE | SC_MOVE | SC_SIZE | SC_CLOSE => return 0,
                _ => (),
            },
            WM_CLOSE => return 0, // Quit belongs to the tray, not the desktop surface.
            WM_NCDESTROY => {
                RemoveWindowSubclass(hwnd, Some(desktop_policy), id);
            }
            _ => (),
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }

    // LPARAM carries this enumeration's result; no global mutable shell handles.
    unsafe extern "system" fn enum_windows_proc(hwnd: HWND, result: LPARAM) -> BOOL {
        let icons = FindWindowExA(hwnd, 0, b"SHELLDLL_DefView\0".as_ptr(), null());
        if icons != 0 {
            let worker = FindWindowExA(0, hwnd, b"WorkerW\0".as_ptr(), null());
            if worker != 0 {
                *(result as *mut HWND) = worker;
                return FALSE;
            }
        }
        TRUE
    }

    pub unsafe fn setup_as_wallpaper(
        hwnd: HWND,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<()> {
        let progman = FindWindowA(b"Progman\0".as_ptr(), null());
        if progman == 0 {
            return Err("Explorer desktop is unavailable; will retry".into());
        }
        // Explorer's WorkerW protocol is undocumented. Bound the wait and verify
        // the resulting hierarchy instead of showing a top-level fallback window.
        let mut result = 0;
        SendMessageTimeoutA(progman, 0x052C, 0xD, 1, SMTO_ABORTIFHUNG, 1000, &mut result);
        let icons = FindWindowExA(progman, 0, b"SHELLDLL_DefView\0".as_ptr(), null());
        let nested_worker = FindWindowExA(progman, 0, b"WorkerW\0".as_ptr(), null());
        let mut worker: HWND = 0;
        EnumWindows(Some(enum_windows_proc), &mut worker as *mut HWND as LPARAM);
        let (parent, insert_after) = if icons != 0 && nested_worker != 0 {
            // Newer Explorer versions host the wallpaper WorkerW inside Progman.
            (nested_worker, HWND_BOTTOM)
        } else if worker != 0 {
            // Classic layout: a top-level WorkerW behind the desktop icon host.
            (worker, HWND_BOTTOM)
        } else if icons != 0 {
            // Direct Progman layout; only order OUR child below the icons.
            (progman, icons)
        } else {
            return Err("Explorer has no wallpaper host; will retry".into());
        };
        let style = GetWindowLongPtrA(hwnd, GWL_STYLE) as u32;
        SetWindowLongPtrA(
            hwnd,
            GWL_STYLE,
            ((style
                & !(WS_POPUP
                    | WS_CAPTION
                    | WS_THICKFRAME
                    | WS_SYSMENU
                    | WS_MINIMIZEBOX
                    | WS_MAXIMIZEBOX
                    | WS_MINIMIZE
                    | WS_MAXIMIZE))
                | WS_CHILD) as isize,
        );
        SetParent(hwnd, parent);
        // SetParent can return NULL on success (the previous parent was NULL).
        if GetParent(hwnd) != parent {
            return Err(io::Error::last_os_error().into());
        }
        let ex_style = GetWindowLongPtrA(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongPtrA(
            hwnd,
            GWL_EXSTYLE,
            ((ex_style & !(WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE))
                | WS_EX_TOOLWINDOW
                | WS_EX_NOACTIVATE
                | WS_EX_LAYERED
                | WS_EX_TRANSPARENT) as isize,
        );
        // Layered + transparent makes hit testing pass through across processes.
        // HTTRANSPARENT alone would only pass to windows on our own thread.
        if SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) == 0 {
            return Err(io::Error::last_os_error().into());
        }
        if SetWindowSubclass(hwnd, Some(desktop_policy), 0x44505750, 0) == 0 {
            return Err("Cannot install wallpaper window policy".into());
        }
        let mut origin = POINT { x, y };
        MapWindowPoints(0, parent, &mut origin, 1);
        if SetWindowPos(
            hwnd,
            insert_after,
            origin.x,
            origin.y,
            width,
            height,
            SWP_NOACTIVATE | SWP_FRAMECHANGED,
        ) == 0
        {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn setup_wallpaper_window(window: &Window, display: &DisplayInfo) -> Result<()> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    // Keep winit's cached mouse policy in agreement with the native styles.
    window.set_cursor_hittest(false)?;
    let handle = window.window_handle()?;
    if let RawWindowHandle::Win32(handle) = handle.as_raw() {
        unsafe {
            windows_wallpaper::setup_as_wallpaper(
                handle.hwnd.get(),
                display.origin_x as i32,
                display.origin_y as i32,
                display.pixels_wide as i32,
                display.pixels_high as i32,
            )
        }
    } else {
        Err("Expected a Win32 wallpaper window".into())
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn show_wallpaper_window(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(handle) = handle.as_raw() {
            unsafe {
                ShowWindow(handle.hwnd.get(), SW_SHOWNOACTIVATE);
            }
        }
    }
}

/// Explorer can destroy its child windows when the shell restarts. Recreate the
/// renderer rather than continuing to render to a dead HWND (or a floating one).
pub(crate) fn wallpaper_window_is_attached(_window: &Window) -> bool {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetParent, IsWindow};
        if let Ok(handle) = _window.window_handle() {
            if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                unsafe {
                    return IsWindow(handle.hwnd.get()) != 0
                        && IsWindow(GetParent(handle.hwnd.get())) != 0;
                }
            }
        }
        return false;
    }
    #[cfg(not(target_os = "windows"))]
    true
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn setup_wallpaper_window(_window: &Window, _display: &DisplayInfo) -> Result<()> {
    Err("Wallpaper mode is only supported on macOS and Windows; use --windowed".into())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn show_wallpaper_window(_window: &Window) {}
