use eframe::CreationContext;

/// Applies platform-specific window blur/backdrop effects when enabled.
pub fn apply(cc: &CreationContext<'_>, blur_enabled: bool) -> Result<(), String> {
    if !blur_enabled {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    return windows::apply(cc);
    #[cfg(target_os = "linux")]
    return linux::apply(cc);
    #[cfg(target_os = "macos")]
    return macos::apply(cc);

    #[allow(unreachable_code)]
    Err("window blur is not supported on this platform".to_owned())
}

#[cfg(target_os = "windows")]
mod windows {
    use core::mem::size_of;
    use eframe::CreationContext;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    const DWMWA_SYSTEMBACKDROP_TYPE: i32 = 38;
    const DWMSBT_TRANSIENTWINDOW: i32 = 3;

    pub fn apply(cc: &CreationContext<'_>) -> Result<(), String> {
        let window_handle = cc
            .window_handle()
            .map_err(|error| format!("window handle unavailable: {error}"))?;
        let RawWindowHandle::Win32(handle) = window_handle.as_raw() else {
            return Err("unsupported window handle for Windows blur".to_owned());
        };
        let hwnd: HWND = handle.hwnd.get() as HWND;
        let backdrop_type = DWMSBT_TRANSIENTWINDOW;
        let result = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                &backdrop_type as *const _ as *const _,
                size_of::<i32>() as u32,
            )
        };

        if result != 0 {
            return Err(format!(
                "Windows backdrop API rejected the blur request ({result:#x})"
            ));
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use eframe::CreationContext;
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
    use std::ffi::CStr;
    use std::ffi::c_void;
    use std::os::raw::c_uchar;
    use wayland_backend::client::{Backend, ObjectId};
    use wayland_client::globals::GlobalListContents;
    use wayland_client::protocol::{wl_registry, wl_surface::WlSurface};
    use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, delegate_noop};
    use wayland_protocols_plasma::blur::client::org_kde_kwin_blur::OrgKdeKwinBlur;
    use wayland_protocols_plasma::blur::client::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager;
    use x11_dl::xlib;

    static KDE_BLUR_ATOM: &CStr = c"_KDE_NET_WM_BLUR_BEHIND_REGION";
    static CARDINAL_ATOM: &CStr = c"CARDINAL";

    pub fn apply(cc: &CreationContext<'_>) -> Result<(), String> {
        let window_handle = cc
            .window_handle()
            .map_err(|error| format!("window handle unavailable: {error}"))?;
        let display_handle = cc
            .display_handle()
            .map_err(|error| format!("display handle unavailable: {error}"))?;

        match (display_handle.as_raw(), window_handle.as_raw()) {
            (RawDisplayHandle::Xlib(display), RawWindowHandle::Xlib(window)) => {
                let Some(display) = display.display else {
                    return Err("X11 display pointer unavailable".to_owned());
                };
                apply_x11(display.as_ptr().cast::<xlib::Display>(), window.window)
            }
            (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(window)) => {
                apply_wayland(
                    display.display.as_ptr().cast::<c_void>(),
                    window.surface.as_ptr().cast::<c_void>(),
                )
            }
            _ => Err("unsupported Linux window/display backend for blur".to_owned()),
        }
    }

    fn apply_x11(display: *mut xlib::Display, window: std::os::raw::c_ulong) -> Result<(), String> {
        let xlib =
            xlib::Xlib::open().map_err(|_| "failed to load Xlib for blur support".to_owned())?;

        let kde_blur_atom =
            unsafe { (xlib.XInternAtom)(display, KDE_BLUR_ATOM.as_ptr(), xlib::False) };
        let cardinal_atom =
            unsafe { (xlib.XInternAtom)(display, CARDINAL_ATOM.as_ptr(), xlib::False) };
        if kde_blur_atom == 0 || cardinal_atom == 0 {
            return Err("KDE X11 blur atoms are unavailable on this display".to_owned());
        }

        unsafe {
            // Empty region means "blur the whole window".
            (xlib.XChangeProperty)(
                display,
                window,
                kde_blur_atom,
                cardinal_atom,
                32,
                xlib::PropModeReplace,
                std::ptr::null::<c_uchar>(),
                0,
            );
            (xlib.XFlush)(display);
        }
        Ok(())
    }

    struct KdeBlurState;

    impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for KdeBlurState {
        fn event(
            _: &mut Self,
            _: &wl_registry::WlRegistry,
            _: wl_registry::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    delegate_noop!(KdeBlurState: ignore OrgKdeKwinBlurManager);
    delegate_noop!(KdeBlurState: ignore OrgKdeKwinBlur);

    fn apply_wayland(display: *mut c_void, surface: *mut c_void) -> Result<(), String> {
        if display.is_null() || surface.is_null() {
            return Err("Wayland display or surface pointer unavailable".to_owned());
        }

        let backend = unsafe { Backend::from_foreign_display(display.cast()) };
        let conn = Connection::from_backend(backend);

        let surface_id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.cast()) };
        let Ok(surface_id) = surface_id else {
            return Err("failed to resolve Wayland surface object id".to_owned());
        };
        let Ok(surface) = WlSurface::from_id(&conn, surface_id) else {
            return Err("failed to resolve Wayland surface proxy".to_owned());
        };

        let Ok((globals, mut queue)) =
            wayland_client::globals::registry_queue_init::<KdeBlurState>(&conn)
        else {
            return Err("failed to initialize Wayland registry for blur support".to_owned());
        };

        let qh = queue.handle();
        let Ok(manager) = globals.bind::<OrgKdeKwinBlurManager, _, _>(&qh, 1..=1, ()) else {
            return Err("KDE Wayland blur manager is unavailable".to_owned());
        };

        let blur = manager.create(&surface, &qh, ());
        blur.commit();

        let _ = conn.flush();
        let _ = queue.dispatch_pending(&mut KdeBlurState);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use eframe::CreationContext;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSRect {
        origin: NSPoint,
        size: NSSize,
    }

    const NS_VISUAL_EFFECT_BLENDING_MODE_BEHIND_WINDOW: isize = 0;
    const NS_VISUAL_EFFECT_MATERIAL_UNDER_WINDOW_BACKGROUND: isize = 21;
    const NS_VISUAL_EFFECT_STATE_ACTIVE: isize = 1;
    const NS_VIEW_WIDTH_SIZABLE: usize = 1 << 1;
    const NS_VIEW_HEIGHT_SIZABLE: usize = 1 << 4;

    pub fn apply(cc: &CreationContext<'_>) -> Result<(), String> {
        let window_handle = cc
            .window_handle()
            .map_err(|error| format!("window handle unavailable: {error}"))?;
        let RawWindowHandle::AppKit(handle) = window_handle.as_raw() else {
            return Err("unsupported window handle for macOS blur".to_owned());
        };

        let ns_view = handle.ns_view.as_ptr().cast::<Object>();
        unsafe {
            let ns_window: *mut Object = msg_send![ns_view, window];
            if ns_window.is_null() {
                return Err("AppKit window pointer unavailable".to_owned());
            }

            let content_view: *mut Object = msg_send![ns_window, contentView];
            if content_view.is_null() {
                return Err("AppKit content view unavailable".to_owned());
            }
            let bounds: NSRect = msg_send![content_view, bounds];
            let effect_view: *mut Object = msg_send![class!(NSVisualEffectView), alloc];
            let effect_view: *mut Object = msg_send![effect_view, initWithFrame: bounds];
            if effect_view.is_null() {
                return Err("failed to allocate NSVisualEffectView".to_owned());
            }

            let _: () = msg_send![effect_view, setAutoresizingMask: (NS_VIEW_WIDTH_SIZABLE | NS_VIEW_HEIGHT_SIZABLE)];
            let _: () = msg_send![effect_view, setBlendingMode: NS_VISUAL_EFFECT_BLENDING_MODE_BEHIND_WINDOW];
            let _: () = msg_send![effect_view, setMaterial: NS_VISUAL_EFFECT_MATERIAL_UNDER_WINDOW_BACKGROUND];
            let _: () = msg_send![effect_view, setState: NS_VISUAL_EFFECT_STATE_ACTIVE];
            let _: () = msg_send![content_view, addSubview: effect_view positioned: 0isize relativeTo: std::ptr::null::<Object>()];
        }
        Ok(())
    }
}
