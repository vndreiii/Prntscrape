use super::{AppEntry, CaptureBackend, Region, WindowInfo};
use image::RgbaImage;
use serde::Deserialize;
use std::process::Command;

pub struct HyprlandBackend;

#[derive(Deserialize)]
struct HyprWindow {
    class: String,
    title: String,
    at: [i32; 2],
    size: [u32; 2],
    monitor: i32,
}

#[derive(Deserialize)]
struct HyprMonitor {
    id: i32,
    name: String,
}

impl HyprlandBackend {
    pub fn new() -> Self {
        Self
    }
}

impl CaptureBackend for HyprlandBackend {
    fn active_window(&self) -> Option<WindowInfo> {
        let output = Command::new("hyprctl")
            .args(["activewindow", "-j"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let win: HyprWindow = serde_json::from_slice(&output.stdout).ok()?;

        let mon_output = Command::new("hyprctl")
            .args(["monitors", "-j"])
            .output()
            .ok()?;

        let mut monitor_name = win.monitor.to_string();
        if mon_output.status.success() {
            if let Ok(monitors) = serde_json::from_slice::<Vec<HyprMonitor>>(&mon_output.stdout) {
                if let Some(m) = monitors.iter().find(|m| m.id == win.monitor) {
                    monitor_name = m.name.clone();
                }
            }
        }

        Some(WindowInfo {
            app_class: win.class,
            title: win.title,
            x: win.at[0],
            y: win.at[1],
            width: win.size[0],
            height: win.size[1],
            monitor: monitor_name,
        })
    }

    fn capture(
        &self,
        region: Region,
        active_window: Option<&WindowInfo>,
    ) -> Result<RgbaImage, String> {
        let mut cmd = Command::new("grim");

        match region {
            Region::ActiveWindow => {
                if let Some(win) = active_window {
                    let geom = format!("{},{} {}x{}", win.x, win.y, win.width, win.height);
                    cmd.args(["-g", &geom]);
                } else {
                    return Err("Active window required for window region capture".into());
                }
            }
            Region::Monitor => {
                if let Some(win) = active_window {
                    cmd.args(["-o", &win.monitor]);
                } else {
                    // Fallback to all screens if no active window
                }
            }
        }

        cmd.args(["-t", "png", "-"]);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to run grim: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "grim failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let img = image::load_from_memory_with_format(&output.stdout, image::ImageFormat::Png)
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        Ok(img.to_rgba8())
    }

    fn running_apps(&self) -> Vec<AppEntry> {
        let output = Command::new("hyprctl")
            .args(["clients", "-j"])
            .output()
            .unwrap_or_else(|_| std::process::Output {
                status: std::os::unix::process::ExitStatusExt::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            });

        if !output.status.success() {
            return vec![];
        }

        if let Ok(clients) = serde_json::from_slice::<Vec<HyprWindow>>(&output.stdout) {
            let mut apps = clients
                .into_iter()
                .map(|c| c.class)
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>();
            apps.sort();
            apps.dedup();
            apps.into_iter()
                .map(|name| AppEntry { name, icon: None })
                .collect()
        } else {
            vec![]
        }
    }

    fn preflight(&self) -> Result<(), String> {
        let hypr_ok = Command::new("hyprctl")
            .arg("version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !hypr_ok {
            return Err("Hyprland not detected or hyprctl not in PATH".into());
        }

        let grim_ok = Command::new("grim")
            .arg("-h")
            .output()
            .map(|o| {
                o.status.success()
            // grim -h exits with 0 or 1 depending on version, just checking if we can execute it
            || o.status.code() == Some(1)
            })
            .unwrap_or(false);

        if !grim_ok {
            return Err("grim not found in PATH".into());
        }

        Ok(())
    }
}
