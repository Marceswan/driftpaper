#![allow(unexpected_cfgs)]
use std::sync::{
    atomic::{AtomicU32, Ordering},
    OnceLock,
};
use winit::event_loop::{EventLoopBuilder, EventLoopProxy};

#[derive(Debug)]
pub(crate) enum UserEvent {
    Wake,
}
static PROXY: OnceLock<std::sync::Mutex<EventLoopProxy<UserEvent>>> = OnceLock::new();
static PAUSE_REASONS: AtomicU32 = AtomicU32::new(0);
const SYSTEM_SLEEP: u32 = 1;
const DISPLAY_SLEEP: u32 = 2;
const SESSION_INACTIVE: u32 = 4;
#[cfg(target_os = "macos")]
const SESSION_LOCKED: u32 = 8;

pub(crate) fn install_proxy(proxy: EventLoopProxy<UserEvent>) {
    let _ = PROXY.set(std::sync::Mutex::new(proxy));
}
pub(crate) fn wake() {
    if let Some(proxy) = PROXY.get() {
        let _ = proxy
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .send_event(UserEvent::Wake);
    }
}
pub(crate) fn paused() -> bool {
    PAUSE_REASONS.load(Ordering::SeqCst) != 0
}
fn set_paused(reason: u32, paused: bool) {
    if paused {
        PAUSE_REASONS.fetch_or(reason, Ordering::SeqCst);
    } else {
        PAUSE_REASONS.fetch_and(!reason, Ordering::SeqCst);
    }
    wake();
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn configure_event_loop(_: &mut EventLoopBuilder<UserEvent>) {}

#[cfg(target_os = "macos")]
pub(crate) fn observe() {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{
        class,
        declare::ClassDecl,
        msg_send,
        runtime::{Object, Sel},
        sel, sel_impl,
    };
    extern "C" fn sleep(_: &Object, _: Sel, _: id) {
        set_paused(SYSTEM_SLEEP, true);
    }
    extern "C" fn wake_system(_: &Object, _: Sel, _: id) {
        set_paused(SYSTEM_SLEEP, false);
    }
    extern "C" fn screen_sleep(_: &Object, _: Sel, _: id) {
        set_paused(DISPLAY_SLEEP, true);
    }
    extern "C" fn screen_wake(_: &Object, _: Sel, _: id) {
        set_paused(DISPLAY_SLEEP, false);
    }
    extern "C" fn inactive(_: &Object, _: Sel, _: id) {
        set_paused(SESSION_INACTIVE, true);
    }
    extern "C" fn active(_: &Object, _: Sel, _: id) {
        set_paused(SESSION_INACTIVE, false);
    }
    extern "C" fn locked(_: &Object, _: Sel, _: id) {
        set_paused(SESSION_LOCKED, true);
    }
    extern "C" fn unlocked(_: &Object, _: Sel, _: id) {
        set_paused(SESSION_LOCKED, false);
    }
    unsafe {
        let mut decl = ClassDecl::new("DriftPowerObserver", class!(NSObject)).unwrap();
        for (selector, callback) in [
            (
                sel!(sessionLocked:),
                locked as extern "C" fn(&Object, Sel, id),
            ),
            (sel!(sessionUnlocked:), unlocked),
            (sel!(systemSleep:), sleep as extern "C" fn(&Object, Sel, id)),
            (sel!(systemWake:), wake_system),
            (sel!(screenSleep:), screen_sleep),
            (sel!(screenWake:), screen_wake),
            (sel!(sessionInactive:), inactive),
            (sel!(sessionActive:), active),
        ] {
            decl.add_method(selector, callback);
        }
        let observer: id = msg_send![decl.register(), new];
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let center: id = msg_send![workspace, notificationCenter];
        for (name, selector) in [
            ("NSWorkspaceWillSleepNotification", sel!(systemSleep:)),
            ("NSWorkspaceDidWakeNotification", sel!(systemWake:)),
            ("NSWorkspaceScreensDidSleepNotification", sel!(screenSleep:)),
            ("NSWorkspaceScreensDidWakeNotification", sel!(screenWake:)),
            (
                "NSWorkspaceSessionDidResignActiveNotification",
                sel!(sessionInactive:),
            ),
            (
                "NSWorkspaceSessionDidBecomeActiveNotification",
                sel!(sessionActive:),
            ),
        ] {
            let name = NSString::alloc(nil).init_str(name);
            let _: () =
                msg_send![center, addObserver:observer selector:selector name:name object:nil];
            let _: () = msg_send![name, release];
        }
        let distributed: id = msg_send![class!(NSDistributedNotificationCenter), defaultCenter];
        for (name, selector) in [
            ("com.apple.screenIsLocked", sel!(sessionLocked:)),
            ("com.apple.screenIsUnlocked", sel!(sessionUnlocked:)),
        ] {
            let name = NSString::alloc(nil).init_str(name);
            let _: () =
                msg_send![distributed, addObserver:observer selector:selector name:name object:nil];
            let _: () = msg_send![name, release];
        }
        // The observer owns its +1 reference for the lifetime of the application.
    }
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn observe() {}

#[cfg(target_os = "windows")]
pub(crate) fn configure_event_loop(builder: &mut EventLoopBuilder<UserEvent>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::MSG;
    use winit::platform::windows::EventLoopBuilderExtWindows;
    // Posted display-change messages wake reconciliation. Power/session messages are
    // handled by each registered window's subclass (sent messages bypass this hook).
    builder.with_msg_hook(|message| {
        let message = unsafe { &*(message as *const MSG) };
        if message.message == 0x007e {
            // WM_DISPLAYCHANGE
            crate::SCREEN_CONFIG_CHANGED.store(true, Ordering::SeqCst);
            wake();
        }
        false
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn waking_display_does_not_clear_a_locked_session() {
        super::set_paused(super::SESSION_INACTIVE, true);
        super::set_paused(super::DISPLAY_SLEEP, true);
        super::set_paused(super::DISPLAY_SLEEP, false);
        assert!(super::paused());
        super::set_paused(super::SESSION_INACTIVE, false);
        assert!(!super::paused());
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn observe_window(window: &winit::window::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{
            Power::{
                RegisterPowerSettingNotification, UnregisterPowerSettingNotification,
                POWERBROADCAST_SETTING,
            },
            RemoteDesktop::{WTSRegisterSessionNotification, WTSUnRegisterSessionNotification},
        },
        UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
    };
    const DISPLAY_STATE: windows_sys::core::GUID =
        windows_sys::core::GUID::from_u128(0x2b84c20e_ad23_4ddf_93db_05ffbd7efca5);
    unsafe extern "system" fn callback(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        id: usize,
        registration: usize,
    ) -> LRESULT {
        match message {
            0x007e => {
                // WM_DISPLAYCHANGE (also delivered synchronously)
                crate::SCREEN_CONFIG_CHANGED.store(true, Ordering::SeqCst);
                wake();
            }
            0x0218 => match wparam {
                // WM_POWERBROADCAST
                4 => set_paused(SYSTEM_SLEEP, true), // PBT_APMSUSPEND
                7 | 18 => set_paused(SYSTEM_SLEEP, false), // resume
                0x8013 if lparam != 0 => {
                    // PBT_POWERSETTINGCHANGE
                    let setting = &*(lparam as *const POWERBROADCAST_SETTING);
                    if setting.PowerSetting.data1 == DISPLAY_STATE.data1
                        && setting.PowerSetting.data2 == DISPLAY_STATE.data2
                        && setting.PowerSetting.data3 == DISPLAY_STATE.data3
                        && setting.PowerSetting.data4 == DISPLAY_STATE.data4
                        && setting.DataLength >= 4
                    {
                        let state = std::ptr::read_unaligned(setting.Data.as_ptr() as *const u32);
                        set_paused(DISPLAY_SLEEP, state == 0);
                    }
                }
                _ => (),
            },
            0x02b1 => match wparam {
                // WM_WTSSESSION_CHANGE
                7 | 2 | 4 => set_paused(SESSION_INACTIVE, true),
                8 | 1 | 3 => set_paused(SESSION_INACTIVE, false),
                _ => (),
            },
            0x0082 => {
                // WM_NCDESTROY
                if registration != 0 {
                    UnregisterPowerSettingNotification(registration as isize);
                }
                WTSUnRegisterSessionNotification(hwnd);
                RemoveWindowSubclass(hwnd, Some(callback), id);
            }
            _ => (),
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(handle) = handle.as_raw() {
            let hwnd = handle.hwnd.get();
            unsafe {
                let registration = RegisterPowerSettingNotification(hwnd, &DISPLAY_STATE, 0);
                if SetWindowSubclass(hwnd, Some(callback), 0x44524946, registration as usize) == 0 {
                    if registration != 0 {
                        UnregisterPowerSettingNotification(registration);
                    }
                    log::warn!("Could not register desktop power observer");
                    return;
                }
                if registration == 0 {
                    log::warn!("Could not register display power notifications");
                }
                if WTSRegisterSessionNotification(hwnd, 0) == 0 {
                    log::warn!("Could not register session notifications");
                }
            }
        }
    }
}
