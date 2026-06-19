use super::{AppEntry, CaptureBackend, Region, WindowInfo};
use image::{ImageBuffer, RgbaImage};
use std::ffi::c_void;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
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
        if hwnd.0 == 0 {
            return None;
        }

        // Get the active window's process ID
        let mut process_id = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        }

        // xcap window matching doesn't expose HWND natively in an easy cross-platform way, 
        // but we can match process_id if we get it, or we can just find the active one.
        // xcap's Window struct has `id` which on Windows corresponds to the HWND as a u64.
        
        let windows = Window::all().unwrap_or_default();
        let active_xcap = windows.into_iter().find(|w| w.id() as isize == hwnd.0 as isize)?;

        let monitor_name = active_xcap.current_monitor().map(|m| m.name().to_string()).unwrap_or_default();

        Some(WindowInfo {
            app_class: active_xcap.app_name().to_string(), // xcap extracts the executable name
            title: active_xcap.title().to_string(),
            x: active_xcap.x(),
            y: active_xcap.y(),
            width: active_xcap.width(),
            height: active_xcap.height(),
            monitor: monitor_name,
        })
    }

    fn capture(&self, region: Region, active_window: Option<&WindowInfo>) -> Result<RgbaImage, String> {
        match region {
            Region::ActiveWindow => {
                let hwnd = unsafe { GetForegroundWindow() };
                if hwnd.0 == 0 {
                    return Err("No active window to capture".into());
                }

                let windows = Window::all().map_err(|e| e.to_string())?;
                let win = windows.into_iter().find(|w| w.id() as isize == hwnd.0 as isize)
                    .ok_or_else(|| "Could not find active window in xcap".to_string())?;

                let capture = win.capture_image().map_err(|e| format!("Capture failed: {}", e))?;
                Ok(capture)
            }
            Region::Monitor => {
                if let Some(win_info) = active_window {
                    let monitors = Monitor::all().map_err(|e| e.to_string())?;
                    let monitor = monitors.into_iter()
                        .find(|m| m.name() == win_info.monitor)
                        .unwrap_or_else(|| monitors.first().unwrap().clone());
                    
                    let capture = monitor.capture_image().map_err(|e| format!("Capture failed: {}", e))?;
                    Ok(capture)
                } else {
                    // Fallback to primary monitor
                    let monitors = Monitor::all().map_err(|e| e.to_string())?;
                    if let Some(primary) = monitors.first() {
                        let capture = primary.capture_image().map_err(|e| format!("Capture failed: {}", e))?;
                        Ok(capture)
                    } else {
                        Err("No monitors found".into())
                    }
                }
            }
        }
    }

    fn running_apps(&self) -> Vec<AppEntry> {
        let windows = Window::all().unwrap_or_default();
        let mut apps: Vec<String> = windows.into_iter()
            .map(|w| w.app_name().to_string())
            .filter(|name| !name.is_empty())
            .collect();

        apps.sort();
        apps.dedup();

        apps.into_iter().map(|name| AppEntry { name, icon: None }).collect()
    }

    fn preflight(&self) -> Result<(), String> {
        // No specific preflight needed for Windows xcap backend right now
        Ok(())
    }
}
