use tao::{platform::unix::WindowExtUnix, window::Window};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtUnix};

pub const STYLE: &str = include_str!("linux.css");
pub const SHOW_ON_START: bool = true;
pub const HELP: &str = "Linux capture requires Hyprland and grim. Settings open on startup; a system tray host is needed for background access.";

pub fn build_webview(builder: WebViewBuilder<'_>, window: &Window) -> wry::Result<WebView> {
    builder.build_gtk(window.default_vbox().expect("Tao GTK container"))
}
