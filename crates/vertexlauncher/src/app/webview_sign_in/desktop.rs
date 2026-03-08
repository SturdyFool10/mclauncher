#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn run_webview_window(
    auth_request_uri: &str,
    redirect_uri: &str,
) -> Result<String, String> {
    use crate::app::app_icon;
    use std::sync::{Arc, Mutex};

    use tao::dpi::LogicalSize;
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::platform::run_return::EventLoopExtRunReturn;
    use tao::window::WindowBuilder;
    use wry::WebViewBuilder;

    #[cfg(target_os = "linux")]
    use tao::platform::unix::WindowExtUnix;
    #[cfg(target_os = "linux")]
    use wry::WebViewBuilderExtUnix;

    #[derive(Clone, Copy)]
    enum UserEvent {
        Finish,
    }

    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let result = Arc::new(Mutex::new(None::<Result<String, String>>));
    let result_for_nav = Arc::clone(&result);
    let redirect_prefix = redirect_uri.to_owned();

    let mut window_builder = WindowBuilder::new()
        .with_title("Microsoft Sign-In")
        .with_inner_size(LogicalSize::new(980.0, 760.0));
    if let Some(icon) = app_icon::tao_icon() {
        window_builder = window_builder.with_window_icon(Some(icon));
    }

    let window = window_builder
        .build(&event_loop)
        .map_err(|err| format!("Failed to create sign-in window: {err}"))?;

    let webview_builder = WebViewBuilder::new()
        .with_url(auth_request_uri)
        .with_navigation_handler(move |uri: String| {
            let current_uri = uri;

            // Disallow high-risk local file navigation inside auth webview.
            if current_uri.starts_with("file://") {
                return false;
            }

            if current_uri.starts_with(&redirect_prefix) {
                if let Ok(mut slot) = result_for_nav.lock() {
                    *slot = Some(Ok(current_uri));
                }
                let _ = proxy.send_event(UserEvent::Finish);
                return false;
            }
            true
        });

    #[cfg(target_os = "linux")]
    let _webview =
        webview_builder
            .build_gtk(window.default_vbox().ok_or_else(|| {
                "Failed to access Tao default GTK container for webview".to_owned()
            })?)
            .map_err(|err| format!("Failed to build webview: {err}"))?;

    #[cfg(not(target_os = "linux"))]
    let _webview = webview_builder
        .build(&window)
        .map_err(|err| format!("Failed to build webview: {err}"))?;

    let result_for_loop = Arc::clone(&result);
    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(UserEvent::Finish) => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if let Ok(mut slot) = result_for_loop.lock() {
                    if slot.is_none() {
                        *slot = Some(Err("Microsoft sign-in was canceled".to_owned()));
                    }
                }
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });

    match result.lock() {
        Ok(mut slot) => slot
            .take()
            .unwrap_or_else(|| Err("Microsoft sign-in ended without a callback URL".to_owned())),
        Err(_) => Err("Sign-in state was poisoned unexpectedly".to_owned()),
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub(super) fn run_webview_window(
    _auth_request_uri: &str,
    _redirect_uri: &str,
) -> Result<String, String> {
    Err("Webview sign-in is not supported on this platform".to_owned())
}
