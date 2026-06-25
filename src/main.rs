#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

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
    std::panic::set_hook(Box::new(|info| {
        eprintln!("\n❌ Prntscrape has crashed due to a panic!");
        if let Some(s) = info.payload().downcast_ref::<&str>() {
            eprintln!("Panic message: {s}");
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            eprintln!("Panic message: {s}");
        } else {
            eprintln!("Panic message: Unknown error");
        }
        if let Some(location) = info.location() {
            eprintln!("Location: {}:{}:{}", location.file(), location.line(), location.column());
        }
        eprintln!("Please check your system permissions (Automation & Accessibility) and try again.");
    }));

    println!("Starting Prntscrape...");
    let config = config::Config::load_or_default();
    
    // Convert path to string or debug representation for display
    let path = config::Config::config_path();
    println!("Config loaded from: {}", path.to_string_lossy());
    println!("Watching for applications: {:?}", config.watchlist);
    println!("Save directory: {}", config.save_directory);
    
    let config_arc = Arc::new(Mutex::new(config));
    let capture_now = Arc::new(AtomicBool::new(false));
    let mut engine = engine::Engine::new(config_arc, capture_now);
    engine.run();
}
