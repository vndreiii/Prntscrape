mod capture;
mod config;
mod engine;
mod naming;
mod storage;

#[cfg(target_os = "windows")]
mod ui;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
fn main() {
    let config = config::Config::load_or_default();
    let config_arc = Arc::new(Mutex::new(config));
    let capture_now = Arc::new(AtomicBool::new(false));

    // Spawn capture engine on a background thread
    let eng_config = config_arc.clone();
    let eng_capture = capture_now.clone();
    std::thread::spawn(move || {
        let mut engine = engine::Engine::new(eng_config, eng_capture);
        engine.run();
    });

    // Run tray + settings window on the main thread (required by Win32)
    ui::run(config_arc, capture_now);
}

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("Starting Prntscrape...");
    let config = config::Config::load_or_default();
    let config_arc = Arc::new(Mutex::new(config));
    let capture_now = Arc::new(AtomicBool::new(false));
    let mut engine = engine::Engine::new(config_arc, capture_now);
    engine.run();
}
