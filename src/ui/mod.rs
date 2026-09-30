//! Shared settings behavior; native integration and appearance live in platform/.
mod platform;

use crate::{
    capture::{Backend, CaptureBackend},
    config::Config,
    updater::{self, UpdateStatus},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tao::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::{Icon as WindowIcon, WindowBuilder},
};
use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};
use wry::{WebView, WebViewBuilder};

#[derive(Debug)]
pub(super) enum AppEvent {
    ShowSettings,
    Quit,
    CaptureNow,
    PauseToggle,
    BrowseFolder,
    RefreshRunning,
    Ready,
    Save(Config),
    Message(String),
    CheckUpdates,
    Update(UpdateStatus),
    Restart,
    #[cfg(target_os = "windows")]
    DismissStartup,
    #[cfg(target_os = "windows")]
    StartupPopupReady,
}

fn make_icon() -> tray_icon::Icon {
    let rgba = image::load_from_memory(include_bytes!("../../assets/prntscrape-logo.png"))
        .expect("embedded app logo")
        .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
        .into_rgba8()
        .into_raw();
    tray_icon::Icon::from_rgba(rgba, 32, 32).expect("tray icon")
}

pub(super) fn make_window_icon() -> Option<WindowIcon> {
    let rgba = image::load_from_memory(include_bytes!("../../assets/prntscrape-logo.png"))
        .ok()?
        .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    WindowIcon::from_rgba(rgba.as_raw().clone(), 32, 32).ok()
}

fn settings_html() -> String {
    use base64::Engine as _;
    let logo = base64::engine::general_purpose::STANDARD
        .encode(include_bytes!("../../assets/prntscrape-logo.png"));
    include_str!("settings.html")
        .replace("/* SHARED_STYLE */", include_str!("settings.css"))
        .replace("/* PLATFORM_STYLE */", platform::STYLE)
        .replace("/* SETTINGS_SCRIPT */", include_str!("settings.js"))
        .replace("{{LOGO_DATA}}", &format!("data:image/png;base64,{logo}"))
}

fn send(webview: &WebView, function: &str, value: &impl serde::Serialize) {
    if let Ok(json) = serde_json::to_string(value) {
        if let Err(error) = webview.evaluate_script(&format!("{function}({json})")) {
            eprintln!("Settings script failed: {error}");
        }
    }
}

fn sync_config(
    webview: &WebView,
    config: &Arc<Mutex<Config>>,
    pause: &MenuItem,
    tray: Option<&TrayIcon>,
) {
    let cfg = config.lock().unwrap();
    pause.set_text(if cfg.paused { "Resume" } else { "Pause" });
    if let Some(tray) = tray {
        let _ = tray.set_tooltip(Some(if cfg.paused {
            "Prntscrape (Paused)"
        } else {
            "Prntscrape"
        }));
    }
    send(webview, "loadConfig", &*cfg);
}

fn persist(mut cfg: Config, config: &Arc<Mutex<Config>>) -> Result<(), String> {
    cfg.validate()?;
    cfg.watchlist = cfg
        .watchlist
        .into_iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    cfg.watchlist.sort();
    cfg.watchlist.dedup();
    cfg.save()
        .map_err(|error| format!("Could not save settings: {error}"))?;
    *config.lock().unwrap() = cfg;
    Ok(())
}

pub fn run(config: Arc<Mutex<Config>>, capture_now: Arc<AtomicBool>, smoke_test: bool) {
    // Construct the event loop first: this initializes GTK/AppKit before menus.
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let executable = updater::launch_path().ok();
    let window = WindowBuilder::new()
        .with_title("Prntscrape Settings")
        .with_window_icon(make_window_icon())
        .with_inner_size(LogicalSize::new(820.0, 720.0))
        .with_min_inner_size(LogicalSize::new(560.0, 480.0))
        .with_visible(platform::SHOW_ON_START)
        .build(&event_loop)
        .expect("settings window");

    let pause = MenuItem::new(
        if config.lock().unwrap().paused {
            "Resume"
        } else {
            "Pause"
        },
        true,
        None,
    );
    let capture = MenuItem::new("Capture Now", true, None);
    let settings = MenuItem::new("Settings", true, None);
    let updates = MenuItem::new("Check for Updates", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &pause,
        &capture,
        &PredefinedMenuItem::separator(),
        &settings,
        &updates,
        &PredefinedMenuItem::separator(),
        &quit,
    ])
    .expect("tray menu");
    let ids = (
        pause.id().clone(),
        capture.id().clone(),
        settings.id().clone(),
        updates.id().clone(),
        quit.id().clone(),
    );
    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let action = if event.id == ids.0 {
            AppEvent::PauseToggle
        } else if event.id == ids.1 {
            AppEvent::CaptureNow
        } else if event.id == ids.2 {
            AppEvent::ShowSettings
        } else if event.id == ids.3 {
            AppEvent::CheckUpdates
        } else if event.id == ids.4 {
            AppEvent::Quit
        } else {
            return;
        };
        let _ = menu_proxy.send_event(action);
    }));
    let tray_proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            let _ = tray_proxy.send_event(AppEvent::ShowSettings);
        }
    }));
    let mut tray_menu = Some(menu);
    let mut tray: Option<TrayIcon> = None;

    let initial = serde_json::json!({
        "config": &*config.lock().unwrap(),
        "running": Backend::new().running_apps(),
        "version": env!("CARGO_PKG_VERSION"),
        "help": platform::HELP,
        "backend_error": Backend::new().preflight().err(),
    });
    let script = format!(
        "window.__INIT__={initial}; window.__INIT_CFG__=window.__INIT__.config; window.__INIT_RUNNING__=window.__INIT__.running;"
    );
    let ipc_proxy = proxy.clone();
    let builder = WebViewBuilder::new()
        .with_html(settings_html())
        .with_devtools(cfg!(debug_assertions))
        .with_initialization_script(&script)
        .with_ipc_handler(move |request| {
            let Ok(msg) = serde_json::from_str::<serde_json::Value>(request.body()) else {
                return;
            };
            let event = match msg["action"].as_str() {
                Some("save") => match serde_json::from_value(msg["data"].clone()) {
                    Ok(config) => AppEvent::Save(config),
                    Err(error) => AppEvent::Message(format!("Invalid settings: {error}")),
                },
                Some("browse") => AppEvent::BrowseFolder,
                Some("capture_now") => AppEvent::CaptureNow,
                Some("refresh_running") => AppEvent::RefreshRunning,
                Some("request_config") => AppEvent::Ready,
                Some("check_updates") => AppEvent::CheckUpdates,
                Some("restart") => AppEvent::Restart,
                Some("quit") => AppEvent::Quit,
                Some("log_error") => {
                    eprintln!("Settings error: {}", msg["error"]);
                    return;
                }
                _ => return,
            };
            let _ = ipc_proxy.send_event(event);
        });
    let webview = platform::build_webview(builder, &window).expect("settings webview");
    let mut update_status =
        UpdateStatus::new("idle", "Updates are checked automatically from GitHub.");
    let mut updating = false;
    let mut ready = false;
    #[cfg(target_os = "windows")]
    let mut popup: Option<platform::StartupPopup> = None;
    #[cfg(target_os = "windows")]
    let mut startup_popup_ready = false;

    let update_proxy = proxy.clone();
    if !smoke_test {
        std::thread::spawn(move || {
            loop {
                if update_proxy.send_event(AppEvent::CheckUpdates).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_secs(6 * 60 * 60));
            }
        });
    }

    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                match TrayIconBuilder::new()
                    .with_menu(Box::new(tray_menu.take().unwrap()))
                    .with_tooltip("Prntscrape")
                    .with_icon(make_icon())
                    .with_menu_on_left_click(cfg!(target_os = "linux"))
                    .build()
                {
                    Ok(icon) => tray = Some(icon),
                    Err(error) => {
                        eprintln!("Tray unavailable: {error}");
                        window.set_visible(true);
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    match platform::StartupPopup::new(
                        target,
                        proxy.clone(),
                        config.lock().unwrap().paused,
                    ) {
                        Ok(notification) => popup = Some(notification),
                        Err(error) => {
                            eprintln!("Startup popup failed: {error}");
                            window.set_visible(true);
                        }
                    }
                    let dismiss_proxy = proxy.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_secs(15));
                        let _ = dismiss_proxy.send_event(AppEvent::DismissStartup);
                    });
                }
                #[cfg(not(target_os = "windows"))]
                let _ = target;
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } if window_id == window.id() => window.set_visible(false),
            #[cfg(target_os = "windows")]
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } if popup.as_ref().is_some_and(|p| p.window.id() == window_id) => {
                popup = None;
            }
            #[cfg(target_os = "windows")]
            Event::UserEvent(AppEvent::DismissStartup) => {
                popup = None;
            }
            #[cfg(target_os = "windows")]
            Event::UserEvent(AppEvent::StartupPopupReady) => {
                startup_popup_ready = true;
                if smoke_test && ready {
                    println!("Windows Settings and startup popup smoke test passed");
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }
            Event::UserEvent(AppEvent::ShowSettings) => {
                #[cfg(target_os = "windows")]
                {
                    popup = None;
                }
                window.set_visible(true);
                window.set_minimized(false);
                window.set_focus();
                #[cfg(target_os = "windows")]
                {
                    let size = window.inner_size();
                    window.set_inner_size(tao::dpi::PhysicalSize::new(size.width, size.height + 1));
                    window.set_inner_size(size);
                }
                if ready {
                    sync_config(&webview, &config, &pause, tray.as_ref());
                    send(&webview, "loadRunningApps", &Backend::new().running_apps());
                    send(&webview, "loadUpdate", &update_status);
                }
            }
            Event::UserEvent(AppEvent::Ready) => {
                ready = true;
                if smoke_test {
                    #[cfg(target_os = "windows")]
                    if startup_popup_ready {
                        println!("Windows Settings and startup popup smoke test passed");
                        *control_flow = ControlFlow::Exit;
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        println!("Settings WebView smoke test passed");
                        *control_flow = ControlFlow::Exit;
                    }
                    return;
                }
                sync_config(&webview, &config, &pause, tray.as_ref());
                send(&webview, "loadUpdate", &update_status);
            }
            Event::UserEvent(AppEvent::Save(cfg)) => match persist(cfg, &config) {
                Ok(()) => {
                    sync_config(&webview, &config, &pause, tray.as_ref());
                    send(&webview, "showMessage", &"Settings saved");
                }
                Err(error) => send(&webview, "showMessage", &error),
            },
            Event::UserEvent(AppEvent::Message(message)) => send(&webview, "showMessage", &message),
            Event::UserEvent(AppEvent::RefreshRunning) => {
                send(&webview, "loadRunningApps", &Backend::new().running_apps())
            }
            Event::UserEvent(AppEvent::PauseToggle) => {
                let mut cfg = config.lock().unwrap().clone();
                cfg.paused = !cfg.paused;
                if let Err(error) = persist(cfg, &config) {
                    send(&webview, "showMessage", &error);
                }
                sync_config(&webview, &config, &pause, tray.as_ref());
            }
            Event::UserEvent(AppEvent::CaptureNow) => {
                capture_now.store(true, Ordering::Relaxed);
                send(&webview, "showMessage", &"Capture requested");
            }
            Event::UserEvent(AppEvent::BrowseFolder) => {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    send(&webview, "setSaveDir", &path.to_string_lossy());
                }
            }
            Event::UserEvent(AppEvent::CheckUpdates)
                if !updating && update_status.phase != "updated" =>
            {
                updating = true;
                let status_proxy = proxy.clone();
                updater::check(move |status| {
                    let _ = status_proxy.send_event(AppEvent::Update(status));
                });
            }
            Event::UserEvent(AppEvent::Update(status)) => {
                updating = matches!(status.phase, "checking" | "downloading" | "installing");
                update_status = status;
                if ready {
                    send(&webview, "loadUpdate", &update_status);
                }
            }
            Event::UserEvent(AppEvent::Restart) if update_status.phase == "updated" => {
                let result = executable
                    .as_ref()
                    .ok_or_else(|| "Cannot locate the executable".to_string())
                    .and_then(|path| updater::restart(path).map_err(|error| error.to_string()));
                match result {
                    Ok(_) => *control_flow = ControlFlow::Exit,
                    Err(error) => {
                        send(&webview, "showMessage", &format!("Restart failed: {error}"))
                    }
                }
            }
            Event::UserEvent(AppEvent::Quit) => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}
