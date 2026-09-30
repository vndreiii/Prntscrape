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
    #[cfg(target_os = "windows")]
    pub native_id: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
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
    fn capture(
        &self,
        region: Region,
        active_window: Option<&WindowInfo>,
    ) -> Result<RgbaImage, String>;
    /// Distinct apps with open windows.
    fn running_apps(&self) -> Vec<AppEntry>;
    /// Verify prerequisites.
    fn preflight(&self) -> Result<(), String>;
}

#[cfg(target_os = "linux")]
pub mod linux_hypr;

#[cfg(target_os = "linux")]
pub use linux_hypr::HyprlandBackend as Backend;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::WindowsBackend as Backend;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "macos")]
pub use macos::MacosBackend as Backend;
