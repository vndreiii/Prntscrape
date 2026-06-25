use crate::capture::{Backend, CaptureBackend, Region};
use crate::config::{Config, Format, Mode};
use crate::naming::generate_filepath;
use crate::storage::save_image;
use image::RgbaImage;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub struct Engine {
    config: Arc<Mutex<Config>>,
    backend: Backend,
    pub capture_now: Arc<AtomicBool>,
    pub test_mode: bool,
}

impl Engine {
    pub fn new(config: Arc<Mutex<Config>>, capture_now: Arc<AtomicBool>) -> Self {
        Self {
            config,
            backend: Backend::new(),
            capture_now,
            test_mode: false,
        }
    }

    pub fn run(&mut self) {
        if let Err(e) = self.backend.preflight() {
            eprintln!("Backend preflight failed: {e}");
            return;
        }

        let mut last_capture_time = Instant::now();
        let mut last_hash: Option<u64> = None;
        let mut interval_running = false;
        let mut last_printed_app: Option<String> = None;

        loop {
            thread::sleep(Duration::from_secs(1));

            let config = self.config.lock().unwrap().clone();
            let force_capture = self.capture_now.swap(false, Ordering::Relaxed);

            if config.paused && !force_capture {
                interval_running = false;
                continue;
            }

            let active_window = self.backend.active_window();

            let active_app_name = active_window.as_ref().map(|win| win.app_class.clone());
            if active_app_name != last_printed_app {
                match &active_window {
                    Some(win) => {
                        println!("[Engine] Active window: app_class='{}', title='{}'", win.app_class, win.title);
                    }
                    None => {
                        println!("[Engine] Active window: None");
                    }
                }
                last_printed_app = active_app_name;
            }

            let matched_app = active_window.as_ref().and_then(|win| {
                let class_lower = win.app_class.to_lowercase();
                if self.test_mode || config.watchlist.iter().any(|w| class_lower.contains(&w.to_lowercase())) {
                    Some(win.app_class.clone())
                } else {
                    None
                }
            });

            // Determine if we should capture this tick
            let should_attempt = if force_capture {
                true
            } else if let Some(_) = &matched_app {
                if !interval_running {
                    last_capture_time = Instant::now();
                    interval_running = true;
                }
                last_capture_time.elapsed().as_secs() >= config.interval_secs
            } else if self.test_mode {
                if !interval_running {
                    last_capture_time = Instant::now();
                    interval_running = true;
                }
                last_capture_time.elapsed().as_secs() >= config.interval_secs
            } else {
                interval_running = false;
                false
            };

            if !should_attempt {
                continue;
            }

            // Use matched_app or fall back to a generic name for forced/test captures
            let app_name = matched_app.unwrap_or_else(|| "prntscrape".to_string());
            last_capture_time = Instant::now();

            let region = match config.capture_region {
                crate::config::CaptureRegion::Window => Region::ActiveWindow,
                crate::config::CaptureRegion::Monitor => Region::Monitor,
            };

            // In test mode, if capturing active window fails, fallback to monitor capture
            let capture_result = match self.backend.capture(region, active_window.as_ref()) {
                Ok(img) => Ok(img),
                Err(e) => {
                    if self.test_mode && matches!(region, Region::ActiveWindow) {
                        println!("[Engine] Active window capture failed: {}. Falling back to Monitor capture...", e);
                        self.backend.capture(Region::Monitor, None)
                    } else {
                        Err(e)
                    }
                }
            };

            match capture_result {
                Ok(img) => {
                    let mut do_save = true;
                    if config.skip_unchanged && !force_capture && !self.test_mode {
                        let hash = hash_image(&img);
                        if Some(hash) == last_hash {
                            do_save = false;
                        }
                        last_hash = Some(hash);
                    }

                    if do_save {
                        match config.mode {
                            Mode::Capture => {
                                let ext = match config.format {
                                    Format::Png => "png",
                                    Format::Jpeg => "jpg",
                                };
                                let path = generate_filepath(
                                    &config.save_directory,
                                    &app_name,
                                    config.current_project.as_deref(),
                                    ext,
                                );
                                match save_image(&img, &path, &config.format, config.quality) {
                                    Ok(()) => println!("Saved: {:?}", path),
                                    Err(e) => eprintln!("Save failed: {e}"),
                                }
                            }
                            Mode::Notify => {
                                let _ = notify_rust::Notification::new()
                                    .summary("Prntscrape Reminder")
                                    .body(&format!(
                                        "Working in {}. Click 'Capture Now' in settings to save your progress.",
                                        app_name
                                    ))
                                    .show();
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Capture error: {e}"),
            }
        }
    }
}

fn hash_image(img: &RgbaImage) -> u64 {
    let small = image::imageops::resize(img, 16, 16, image::imageops::FilterType::Nearest);
    let mut hasher = DefaultHasher::new();
    small.pixels().for_each(|p| p.0.hash(&mut hasher));
    hasher.finish()
}
