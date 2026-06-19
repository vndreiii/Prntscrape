use image::RgbaImage;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub app_class: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub monitor: String,
}

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub icon: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy)]
pub enum Region {
    ActiveWindow,
    Monitor,
}

pub trait CaptureBackend {
    /// The currently focused window, if any.
    fn active_window(&self) -> Option<WindowInfo>;
    /// Capture the requested region as raw pixels.
    fn capture(&self, region: Region, active_window: Option<&WindowInfo>) -> Result<RgbaImage, String>;
    /// Distinct apps with open windows.
    fn running_apps(&self) -> Vec<AppEntry>;
    /// Verify prerequisites.
    fn preflight(&self) -> Result<(), String>;
}

#[cfg(target_os = "linux")]
pub mod linux_hypr;

#[cfg(target_os = "linux")]
pub use linux_hypr::HyprlandBackend as Backend;

// Stubs for other OS
#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;
    pub struct WindowsBackend;
    impl CaptureBackend for WindowsBackend {
        fn active_window(&self) -> Option<WindowInfo> { None }
        fn capture(&self, _: Region, _: Option<&WindowInfo>) -> Result<RgbaImage, String> { Err("Not implemented".into()) }
        fn running_apps(&self) -> Vec<AppEntry> { vec![] }
        fn preflight(&self) -> Result<(), String> { Ok(()) }
    }
}
#[cfg(target_os = "windows")]
pub use windows::WindowsBackend as Backend;

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    pub struct MacosBackend;
    impl CaptureBackend for MacosBackend {
        fn active_window(&self) -> Option<WindowInfo> { None }
        fn capture(&self, _: Region, _: Option<&WindowInfo>) -> Result<RgbaImage, String> { Err("Not implemented".into()) }
        fn running_apps(&self) -> Vec<AppEntry> { vec![] }
        fn preflight(&self) -> Result<(), String> { Ok(()) }
    }
}
#[cfg(target_os = "macos")]
pub use macos::MacosBackend as Backend;
