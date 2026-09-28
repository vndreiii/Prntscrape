use super::{AppEntry, CaptureBackend, Region, WindowInfo};
use image::RgbaImage;
use std::process::Command;
use std::sync::Mutex;
use xcap::{Monitor, Window};

pub struct MacosBackend {
    last_error: Mutex<Option<String>>,
}

impl MacosBackend {
    pub fn new() -> Self {
        Self {
            last_error: Mutex::new(None),
        }
    }

    fn get_active_app_name(&self) -> Option<String> {
        let output = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to get name of first application process whose frontmost is true")
            .output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !name.is_empty() {
                        if let Ok(mut guard) = self.last_error.lock() {
                            *guard = None;
                        }
                        return Some(name);
                    }
                } else {
                    let err_msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    let should_print = if let Ok(mut guard) = self.last_error.lock() {
                        if guard.as_ref() != Some(&err_msg) {
                            *guard = Some(err_msg.clone());
                            true
                        } else {
                            false
                        }
                    } else {
                        true
                    };

                    if should_print {
                        eprintln!("[Engine] osascript failed: {}", err_msg);
                        eprintln!(
                            "[Engine] Hint: This usually means Terminal/iTerm lacks Automation or Accessibility permissions."
                        );
                        eprintln!(
                            "[Engine] Grant permission in System Settings -> Privacy & Security -> Automation."
                        );
                    }
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                let should_print = if let Ok(mut guard) = self.last_error.lock() {
                    if guard.as_ref() != Some(&err_msg) {
                        *guard = Some(err_msg.clone());
                        true
                    } else {
                        false
                    }
                } else {
                    true
                };

                if should_print {
                    eprintln!("[Engine] Failed to execute osascript: {}", err_msg);
                }
            }
        }
        None
    }
}

fn is_app_match(window_app: &str, active_app: &str) -> bool {
    let w_lower = window_app.to_lowercase();
    let a_lower = active_app.to_lowercase();
    w_lower == a_lower || w_lower.contains(&a_lower) || a_lower.contains(&w_lower)
}

impl CaptureBackend for MacosBackend {
    fn active_window(&self) -> Option<WindowInfo> {
        let active_app = self.get_active_app_name()?;
        let windows = match Window::all() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[Engine] Window::all() failed: {}", e);
                return None;
            }
        };

        if windows.is_empty() {
            eprintln!(
                "[Engine] Window::all() returned 0 windows. This usually indicates missing Screen Recording permission or that no windows are open."
            );
        }

        let active = windows.iter().find(|w| {
            w.app_name()
                .ok()
                .map(|name| is_app_match(&name, &active_app))
                .unwrap_or(false)
        });

        match active {
            Some(win) => {
                let monitor_name = win
                    .current_monitor()
                    .ok()
                    .and_then(|m| m.name().ok())
                    .unwrap_or_default();

                Some(WindowInfo {
                    app_class: win.app_name().unwrap_or_default(),
                    title: win.title().unwrap_or_default(),
                    x: win.x().unwrap_or(0),
                    y: win.y().unwrap_or(0),
                    width: win.width().unwrap_or(0),
                    height: win.height().unwrap_or(0),
                    monitor: monitor_name,
                })
            }
            None => {
                let mut app_names: Vec<String> = windows
                    .iter()
                    .filter_map(|w| w.app_name().ok())
                    .filter(|n| !n.is_empty())
                    .collect();
                app_names.sort();
                app_names.dedup();
                eprintln!(
                    "[Engine] Could not find window matching active app '{}'. Detected apps: {:?}",
                    active_app, app_names
                );
                None
            }
        }
    }

    fn capture(
        &self,
        region: Region,
        active_window: Option<&WindowInfo>,
    ) -> Result<RgbaImage, String> {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("prntscrape_temp_{}.png", std::process::id()));

        let mut cmd = Command::new("/usr/sbin/screencapture");
        cmd.arg("-x"); // suppress shutter sound

        let mut use_window_id = false;

        if matches!(region, Region::ActiveWindow) {
            if let Some(active_app) = self.get_active_app_name() {
                if let Ok(windows) = Window::all() {
                    if let Some(win) = windows.iter().find(|w| {
                        w.app_name()
                            .ok()
                            .map(|name| is_app_match(&name, &active_app))
                            .unwrap_or(false)
                    }) {
                        if let Ok(win_id) = win.id() {
                            cmd.arg("-l").arg(win_id.to_string());
                            use_window_id = true;
                        }
                    }
                }
            }
        }

        cmd.arg(&temp_file);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute screencapture: {e}"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            // If window capture failed, retry without window ID (capturing entire screen) as fallback
            if use_window_id {
                eprintln!(
                    "[Engine] Window-specific screencapture failed: {}. Retrying with full screen capture...",
                    err
                );
                let mut fallback_cmd = Command::new("/usr/sbin/screencapture");
                fallback_cmd.arg("-x").arg(&temp_file);
                let fb_output = fallback_cmd
                    .output()
                    .map_err(|e| format!("Failed to execute fallback screencapture: {e}"))?;
                if !fb_output.status.success() {
                    let fb_err = String::from_utf8_lossy(&fb_output.stderr)
                        .trim()
                        .to_string();
                    return Err(format!("screencapture failed: {}", fb_err));
                }
            } else {
                return Err(format!("screencapture failed: {}", err));
            }
        }

        let img = image::open(&temp_file)
            .map_err(|e| format!("Failed to load captured image: {e}"))?
            .to_rgba8();

        let _ = std::fs::remove_file(&temp_file);
        Ok(img)
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
