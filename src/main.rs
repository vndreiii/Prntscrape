mod capture;
mod config;
mod engine;
mod naming;
mod storage;

use std::sync::{Arc, Mutex};

fn main() {
    println!("Starting Prntscrape (Headless M1)...");

    let config = config::Config::load_or_default();
    let config_arc = Arc::new(Mutex::new(config));

    let mut engine = engine::Engine::new(config_arc);
    engine.run();
}
