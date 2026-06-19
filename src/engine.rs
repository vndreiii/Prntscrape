use crate::capture::{Backend, CaptureBackend, Region};
use crate::config::{Config, Format, Mode};
use crate::naming::generate_filepath;
use crate::storage::save_image;
use image::RgbaImage;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub struct Engine {
    config: Arc<Mutex<Config>>,
    backend: Backend,
}

impl Engine {
    pub fn new(config: Arc<Mutex<Config>>) -> Self {
        Self {
            config,
            backend: Backend::new(),
        }
    }

    pub fn run(&mut self) {
        if let Err(e) = self.backend.preflight() {
            eprintln!("Backend preflight failed: {}", e);
            return;
        }

        let mut last_capture_time = Instant::now();
        let mut last_hash: Option<u64> = None;
        let mut interval_running = false;

        loop {
            thread::sleep(Duration::from_secs(1));

            let config = self.config.lock().unwrap().clone();

            if config.paused {
                interval_running = false;
                continue;
            }

            let active_window = self.backend.active_window();

            let matched_app = active_window.as_ref().and_then(|win| {
                let class_lower = win.app_class.to_lowercase();
                if config.watchlist.iter().any(|w| class_lower.contains(&w.to_lowercase())) {
                    Some(win.app_class.clone())
                } else {
                    None
                }
            });

            if let Some(app_class) = matched_app {
                if !interval_running {
                    // Just switched to a matched app, reset timer
                    last_capture_time = Instant::now();
                    interval_running = true;
                }

                let elapsed = last_capture_time.elapsed();
                let interval_secs = config.interval_minutes * 60;

                if elapsed.as_secs() >= interval_secs {
                    last_capture_time = Instant::now();

                    let region = match config.capture_region {
                        crate::config::CaptureRegion::Window => Region::ActiveWindow,
                        crate::config::CaptureRegion::Monitor => Region::Monitor,
                    };

                    match self.backend.capture(region, active_window.as_ref()) {
                        Ok(img) => {
                            let mut should_save = true;

                            if config.skip_unchanged {
                                let hash = hash_image(&img);
                                if Some(hash) == last_hash {
                                    should_save = false;
                                }
                                last_hash = Some(hash);
                            }

                            if should_save {
                                match config.mode {
                                    Mode::Capture => {
                                        let ext = match config.format {
                                            Format::Png => "png",
                                            Format::Jpeg => "jpg",
                                        };
                                        let path = generate_filepath(
                                            &config.save_directory,
                                            &app_class,
                                            config.current_project.as_deref(),
                                            ext,
                                        );
                                        if let Err(e) = save_image(&img, &path, &config.format, config.quality) {
                                            eprintln!("Failed to save image: {}", e);
                                        } else {
                                            println!("Saved screenshot: {:?}", path);
                                        }
                                    }
                                    Mode::Notify => {
                                        // M4 notify mode
                                        println!("Notify mode triggered (not yet implemented)");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Capture failed: {}", e);
                        }
                    }
                }
            } else {
                interval_running = false;
            }
        }
    }
}

fn hash_image(img: &RgbaImage) -> u64 {
    // Downscale to 16x16 to ignore minor noise/artifacts and compute hash quickly
    let small = image::imageops::resize(img, 16, 16, image::imageops::FilterType::Nearest);
    let mut hasher = DefaultHasher::new();
    small.pixels().for_each(|p| p.0.hash(&mut hasher));
    hasher.finish()
}
