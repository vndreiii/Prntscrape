use super::super::AppEvent;
use tao::{
    dpi::{LogicalSize, PhysicalPosition},
    event_loop::{EventLoopProxy, EventLoopWindowTarget},
    platform::windows::WindowBuilderExtWindows,
    window::{Window, WindowBuilder},
};
use windows::Win32::{
    Foundation::RECT,
    UI::WindowsAndMessaging::{
        SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    },
};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtWindows};

pub const STYLE: &str = include_str!("windows.css");
pub const SHOW_ON_START: bool = false;
pub const HELP: &str = "Prntscrape runs in the background. Click the tray icon to open settings.";

pub fn build_webview(builder: WebViewBuilder<'_>, window: &Window) -> wry::Result<WebView> {
    // Some Windows GPU/driver combinations leave WebView2 surfaces black.
    // The UI is lightweight, so software rendering is preferable to an empty window.
    builder
        .with_background_color((243, 243, 243, 255))
        .with_additional_browser_args("--disable-gpu")
        .build(window)
}

pub struct StartupPopup {
    _webview: WebView,
    pub window: Window,
}

impl StartupPopup {
    pub fn new(
        event_loop: &EventLoopWindowTarget<AppEvent>,
        proxy: EventLoopProxy<AppEvent>,
        paused: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let window = WindowBuilder::new()
            .with_title("Prntscrape")
            .with_inner_size(LogicalSize::new(360.0, 144.0))
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top(true)
            .with_skip_taskbar(true)
            .with_focused(false)
            .with_visible(false)
            .build(event_loop)?;
        // Keep the popup above the taskbar, including on scaled desktops.
        let mut work = RECT::default();
        let found = unsafe {
            SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some((&mut work as *mut RECT).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .is_ok();
        if !found {
            if let Some(monitor) = window.primary_monitor() {
                let position = monitor.position();
                let size = monitor.size();
                work = RECT {
                    left: position.x,
                    top: position.y,
                    right: position.x + size.width as i32,
                    bottom: position.y + size.height as i32,
                };
            }
        }
        let size = window.outer_size();
        let margin = (16.0 * window.scale_factor()) as i32;
        window.set_outer_position(PhysicalPosition::new(
            (work.right - size.width as i32 - margin).max(work.left),
            (work.bottom - size.height as i32 - margin).max(work.top),
        ));
        let html = include_str!("startup.html").replace(
            "{{STATUS}}",
            if paused {
                "Prntscrape is paused"
            } else {
                "Prntscrape is working"
            },
        );
        let webview = WebViewBuilder::new()
            .with_html(&html)
            .with_background_color((248, 248, 248, 255))
            .with_additional_browser_args("--disable-gpu")
            .with_ipc_handler(move |request| {
                let event = match request.body().as_str() {
                    "settings" => AppEvent::ShowSettings,
                    "dismiss" => AppEvent::DismissStartup,
                    _ => return,
                };
                let _ = proxy.send_event(event);
            })
            .build(&window)?;
        window.set_visible(true);
        let size = window.inner_size();
        window.set_inner_size(tao::dpi::PhysicalSize::new(size.width, size.height + 1));
        window.set_inner_size(size);
        Ok(Self {
            _webview: webview,
            window,
        })
    }
}
