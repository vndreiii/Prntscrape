use tao::window::Window;
use wry::{WebView, WebViewBuilder};

pub const STYLE: &str = include_str!("macos.css");
pub const SHOW_ON_START: bool = false;
pub const HELP: &str = "Allow Screen Recording and Automation in System Settings → Privacy & Security. Open settings again from the menu bar icon.";

pub fn build_webview(builder: WebViewBuilder<'_>, window: &Window) -> wry::Result<WebView> {
    builder.build(window)
}
