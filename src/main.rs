#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod capture;
mod config;
mod engine;
mod naming;
mod storage;
mod updater;

mod ui;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

fn parse_args() -> Option<(config::Config, bool)> {
    let mut config = config::Config::load_or_default();
    let mut args = std::env::args().skip(1);
    let mut test_mode = false;
    let mut interval_overridden = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--interval" | "-i" => {
                if let Some(val_str) = args.next() {
                    if let Ok(val) = val_str.parse::<u64>() {
                        config.interval_secs = val;
                        interval_overridden = true;
                    } else {
                        eprintln!("Invalid interval value: {}", val_str);
                        return None;
                    }
                } else {
                    eprintln!("Missing value for --interval");
                    return None;
                }
            }
            "--test" | "-t" => {
                test_mode = true;
            }
            "--help" | "-h" => {
                println!("Prntscrape - Background screenshot capture utility");
                println!();
                println!("Usage:");
                println!("  prntscrape [options]");
                println!();
                println!("Options:");
                println!(
                    "  -i, --interval <seconds>  Set capture interval (default: loaded from config)"
                );
                println!(
                    "  -t, --test                Test mode: captures active window every tick (ignores watchlist)"
                );
                println!("  -h, --help                Show this help message");
                return None;
            }
            _ => {
                eprintln!("Unknown argument: {}", arg);
                eprintln!("Run with --help to see usage.");
                return None;
            }
        }
    }

    if test_mode && !interval_overridden {
        config.interval_secs = 2; // Default to 2 seconds in test mode
    }

    Some((config, test_mode))
}

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
            eprintln!(
                "Location: {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }
        eprintln!(
            "Please check your system permissions (Automation & Accessibility) and try again."
        );
    }));

    let (config, test_mode) = match parse_args() {
        Some(val) => val,
        None => return,
    };

    println!("Starting Prntscrape...");
    if test_mode {
        println!(
            "🚀 [Test Mode Enabled] Ignoring watchlist, capturing active window every {} seconds.",
            config.interval_secs
        );
    }

    let path = config::Config::config_path();
    println!("Config loaded from: {}", path.to_string_lossy());
    if !test_mode {
        println!("Watching for applications: {:?}", config.watchlist);
    }
    println!("Save directory: {}", config.save_directory);

    let config_arc = Arc::new(Mutex::new(config));
    let capture_now = Arc::new(AtomicBool::new(false));

    // Spawn capture engine on a background thread
    let eng_config = config_arc.clone();
    let eng_capture = capture_now.clone();
    std::thread::spawn(move || {
        let mut engine = engine::Engine::new(eng_config, eng_capture);
        engine.test_mode = test_mode;
        engine.run();
    });

    // Run tray + settings window on the main thread (required by Win32 and macOS AppKit)
    ui::run(config_arc, capture_now);
}
