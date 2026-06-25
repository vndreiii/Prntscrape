use super::{AppEntry, CaptureBackend, Region, WindowInfo};
use image::RgbaImage;
use std::process::Command;
use xcap::{Monitor, Window};

pub struct MacosBackend;

impl MacosBackend {
    pub fn new() -> Self {
        Self
    }
}

fn get_active_app_name() -> Option<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to get name of first application process whose frontmost is true")
        .output()
        .ok()?;
    
    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}

impl CaptureBackend for MacosBackend {
    fn active_window(&self) -> Option<WindowInfo> {
        let active_app = get_active_app_name()?;
        
        let windows = Window::all().ok()?;
        let active = windows.into_iter().find(|w| {
            w.app_name().ok().as_deref() == Some(&active_app)
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

    fn capture(&self, region: Region, active_window: Option<&WindowInfo>) -> Result<RgbaImage, String> {
        match region {
            Region::ActiveWindow => {
                let active_app = get_active_app_name().ok_or_else(|| "No active app found".to_string())?;
                let windows = Window::all().map_err(|e| e.to_string())?;
                let win = windows
                    .into_iter()
                    .find(|w| w.app_name().ok().as_deref() == Some(&active_app))
                    .ok_or_else(|| format!("Active window for app {} not found in xcap", active_app))?;
                win.capture_image().map_err(|e| format!("Capture failed: {e}"))
            }
            Region::Monitor => {
                let monitors = Monitor::all().map_err(|e| e.to_string())?;
                let target_name = active_window.map(|w| w.monitor.as_str()).unwrap_or("");
                let monitor = monitors
                    .into_iter()
                    .find(|m| m.name().ok().as_deref() == Some(target_name))
                    .or_else(|| Monitor::all().ok().and_then(|ms| ms.into_iter().next()))
                    .ok_or_else(|| "No monitors found".to_string())?;
                monitor.capture_image().map_err(|e| format!("Capture failed: {e}"))
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
        apps.into_iter().map(|name| AppEntry { name, icon: None }).collect()
    }

    fn preflight(&self) -> Result<(), String> {
        Ok(())
    }
}
