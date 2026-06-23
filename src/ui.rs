//! Windows system tray + settings window.
use crate::capture::CaptureBackend;
use crate::config::{CaptureRegion, Config, Format, Mode};
use eframe::egui::{self, ViewportBuilder, ViewportCommand};
use notify_rust::Notification;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder, TrayIconEvent,
};

// ── icon ─────────────────────────────────────────────────────────────────────

fn make_icon() -> tray_icon::Icon {
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        let x = (i as u32) % SIZE;
        let y = (i as u32) / SIZE;
        let dx = x as i32 - 16;
        let dy = y as i32 - 16;
        let r = ((dx * dx + dy * dy) as f64).sqrt();
        if r < 14.0 {
            px[0] = 0;
            px[1] = 120;
            px[2] = 215;
            px[3] = 255;
        }
        // else transparent
    }
    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).expect("icon create")
}

// ── run ──────────────────────────────────────────────────────────────────────

pub fn run(config: Arc<Mutex<Config>>, capture_now: Arc<AtomicBool>) {
    let pause_item = MenuItem::new("⏸  Pause", true, None);
    let capture_item = MenuItem::new("📷  Capture Now", true, None);
    let settings_item = MenuItem::new("⚙  Settings", true, None);
    let quit_item = MenuItem::new("✕  Quit", true, None);

    let menu = Menu::new();
    menu.append_items(&[
        &pause_item,
        &capture_item,
        &PredefinedMenuItem::separator(),
        &settings_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])
    .expect("menu build");

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Prntscrape")
        .with_icon(make_icon())
        .with_menu_on_left_click(false)
        .build()
        .expect("tray icon create");

    // ── Startup notification ──────────────────────────────────────────────────
    {
        let cfg = config.lock().unwrap();
        let backend = crate::capture::Backend::new();
        let running = backend.running_apps();
        let matched: Vec<String> = running
            .iter()
            .filter(|a| {
                let lower = a.name.to_lowercase();
                cfg.watchlist.iter().any(|w| lower.contains(w.as_str()))
            })
            .map(|a| a.name.clone())
            .collect();

        let body = if matched.is_empty() {
            format!(
                "In {} min a screenshot will be taken when you open a watched app.",
                cfg.interval_minutes
            )
        } else {
            format!(
                "Detected {}\nIn {} min a screenshot will be taken.",
                matched.join(", "),
                cfg.interval_minutes
            )
        };

        let _ = Notification::new()
            .summary("Prntscrape is up and running!")
            .body(&body)
            .app_id("Prntscrape")
            .show();
    }

    let ids = MenuIds {
        pause: pause_item.id().clone(),
        capture: capture_item.id().clone(),
        settings: settings_item.id().clone(),
        quit: quit_item.id().clone(),
    };

    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Prntscrape Settings")
            .with_inner_size([660.0, 560.0])
            .with_visible(false)
            .with_resizable(true)
            .with_close_button(false), // hide to tray instead of closing
        ..Default::default()
    };

    eframe::run_native(
        "Prntscrape Settings",
        native_options,
        Box::new(move |_cc| {
            Ok(Box::new(App {
                config,
                capture_now,
                _tray: tray,
                pause_item,
                ids,
                visible: false,
                new_entry: String::new(),
            }))
        }),
    )
    .expect("eframe run");
}

// ── App ──────────────────────────────────────────────────────────────────────

struct MenuIds {
    pause: tray_icon::menu::MenuId,
    capture: tray_icon::menu::MenuId,
    settings: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

struct App {
    config: Arc<Mutex<Config>>,
    capture_now: Arc<AtomicBool>,
    _tray: TrayIcon,
    pause_item: MenuItem,
    ids: MenuIds,
    visible: bool,
    new_entry: String,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll tray left-click → show settings
        while let Ok(_ev) = TrayIconEvent::receiver().try_recv() {
            self.visible = true;
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }

        // Poll tray menu events
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            let id = ev.id.clone();
            if id == self.ids.quit {
                std::process::exit(0);
            } else if id == self.ids.settings {
                self.visible = true;
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            } else if id == self.ids.capture {
                self.capture_now.store(true, Ordering::Relaxed);
            } else if id == self.ids.pause {
                let mut cfg = self.config.lock().unwrap();
                cfg.paused = !cfg.paused;
                let paused = cfg.paused;
                let _ = cfg.save();
                let label = if paused { "▶  Resume" } else { "⏸  Pause" };
                self.pause_item.set_text(label);
                let tooltip = if paused {
                    "Prntscrape (Paused)"
                } else {
                    "Prntscrape"
                };
                let _ = self._tray.set_tooltip(Some(tooltip));
            }
        }

        // Keep loop alive (necessary for tray event polling when window hidden)
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        if !self.visible {
            return;
        }

        // ── Settings UI ──────────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Prntscrape Settings");
            ui.separator();

            let mut cfg = self.config.lock().unwrap();

            // Mode
            ui.horizontal(|ui| {
                ui.label("Mode:");
                let capture = matches!(cfg.mode, Mode::Capture);
                if ui.selectable_label(capture, "📸 Auto Capture").clicked() {
                    cfg.mode = Mode::Capture;
                }
                if ui.selectable_label(!capture, "🔔 Just Remind Me").clicked() {
                    cfg.mode = Mode::Notify;
                }
            });

            // Interval
            ui.horizontal(|ui| {
                ui.label("Interval (min):");
                for &v in &[1u64, 2, 5, 10] {
                    if ui
                        .selectable_label(cfg.interval_minutes == v, format!("{v}"))
                        .clicked()
                    {
                        cfg.interval_minutes = v;
                    }
                }
            });

            // Region
            ui.horizontal(|ui| {
                ui.label("Capture region:");
                let win = matches!(cfg.capture_region, CaptureRegion::Window);
                if ui.selectable_label(win, "Active Window").clicked() {
                    cfg.capture_region = CaptureRegion::Window;
                }
                if ui.selectable_label(!win, "Full Monitor").clicked() {
                    cfg.capture_region = CaptureRegion::Monitor;
                }
            });

            // Save folder
            ui.horizontal(|ui| {
                ui.label("Save folder:");
                ui.add(
                    egui::TextEdit::singleline(&mut cfg.save_directory)
                        .desired_width(300.0),
                );
                if ui.button("Browse…").clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        cfg.save_directory = p.to_string_lossy().to_string();
                    }
                }
            });

            // Project
            ui.horizontal(|ui| {
                ui.label("Project name:");
                let mut proj = cfg.current_project.clone().unwrap_or_default();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut proj)
                            .desired_width(220.0)
                            .hint_text("blank = per-app folders"),
                    )
                    .changed()
                {
                    cfg.current_project = if proj.trim().is_empty() {
                        None
                    } else {
                        Some(proj.trim().to_string())
                    };
                }
            });

            // Format + quality
            ui.horizontal(|ui| {
                ui.label("Format:");
                let png = matches!(cfg.format, Format::Png);
                if ui.selectable_label(png, "PNG").clicked() {
                    cfg.format = Format::Png;
                }
                if ui.selectable_label(!png, "JPEG").clicked() {
                    cfg.format = Format::Jpeg;
                }
                if !png {
                    ui.label("Quality:");
                    ui.add(egui::Slider::new(&mut cfg.quality, 50_u8..=100));
                }
            });

            ui.checkbox(&mut cfg.skip_unchanged, "Skip unchanged frames");
            ui.checkbox(&mut cfg.paused, "Paused");

            ui.separator();
            ui.label("Watchlist:");
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .show(ui, |ui| {
                    let mut to_remove: Option<usize> = None;
                    for (i, entry) in cfg.watchlist.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(entry);
                            if ui.small_button("✕").clicked() {
                                to_remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = to_remove {
                        cfg.watchlist.remove(i);
                    }
                });

            // Add entry
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_entry)
                        .desired_width(220.0)
                        .hint_text("app name to add…"),
                );
                let can_add = !self.new_entry.trim().is_empty();
                if ui.add_enabled(can_add, egui::Button::new("+ Add")).clicked() {
                    let entry = self.new_entry.trim().to_lowercase().to_string();
                    if !cfg.watchlist.contains(&entry) {
                        cfg.watchlist.push(entry);
                    }
                    self.new_entry.clear();
                }
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("💾  Save Settings").clicked() {
                    let _ = cfg.save();
                }
                if ui.button("📷  Capture Now").clicked() {
                    self.capture_now.store(true, Ordering::Relaxed);
                }
            });
        });
    }
}
