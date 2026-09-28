use super::{AppEntry, CaptureBackend, Region, WindowInfo};
use image::RgbaImage;
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use xcap::{Monitor, Window};

pub struct WindowsBackend;

impl WindowsBackend {
    pub fn new() -> Self {
        Self
    }
}

impl CaptureBackend for WindowsBackend {
    fn active_window(&self) -> Option<WindowInfo> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return None;
        }
        let hwnd_val = hwnd.0 as usize;

        let windows = Window::all().ok()?;
        let active = windows.into_iter().find(|w| {
            w.id()
                .ok()
                .map(|id| id as usize == hwnd_val)
                .unwrap_or(false)
        })?;

        let monitor_name = active
            .current_monitor()
            .ok()
            .and_then(|m| m.name().ok())
            .unwrap_or_default();

        Some(WindowInfo {
            app_class: active.app_name().unwrap_or_default(),
            title: active.title().unwrap_or_default(),
            x: active.x().unwrap_or(0),
            y: active.y().unwrap_or(0),
            width: active.width().unwrap_or(0),
            height: active.height().unwrap_or(0),
            monitor: monitor_name,
        })
    }

    fn capture(
        &self,
        region: Region,
        active_window: Option<&WindowInfo>,
    ) -> Result<RgbaImage, String> {
        match region {
            Region::ActiveWindow => {
                let hwnd = unsafe { GetForegroundWindow() };
                if hwnd.0.is_null() {
                    return Err("No active window to capture".into());
                }
                let hwnd_val = hwnd.0 as usize;
                let windows = Window::all().map_err(|e| e.to_string())?;
                let win = windows
                    .into_iter()
                    .find(|w| {
                        w.id()
                            .ok()
                            .map(|id| id as usize == hwnd_val)
                            .unwrap_or(false)
                    })
                    .ok_or_else(|| "Active window not found in xcap".to_string())?;
                win.capture_image()
                    .map_err(|e| format!("Capture failed: {e}"))
            }
            Region::Monitor => {
                let monitors = Monitor::all().map_err(|e| e.to_string())?;
                let target_name = active_window.map(|w| w.monitor.as_str()).unwrap_or("");
                let monitor = monitors
                    .into_iter()
                    .find(|m| m.name().ok().as_deref() == Some(target_name))
                    .or_else(|| Monitor::all().ok().and_then(|ms| ms.into_iter().next()))
                    .ok_or_else(|| "No monitors found".to_string())?;
                monitor
                    .capture_image()
                    .map_err(|e| format!("Capture failed: {e}"))
            }
        }
    }

    fn running_apps(&self) -> Vec<AppEntry> {
        let windows = Window::all().unwrap_or_default();
        let mut apps: Vec<String> = windows
            .into_iter()
            .filter_map(|w| w.app_name().ok())
            .filter(|n| !n.is_empty())
            .collect();
        apps.sort();
        apps.dedup();
        apps.into_iter()
            .map(|name| AppEntry { name, icon: None })
            .collect()
    }

    fn preflight(&self) -> Result<(), String> {
        Ok(())
    }
}
