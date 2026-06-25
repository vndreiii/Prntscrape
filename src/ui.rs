//! Windows system tray + WebView2 settings window.
use crate::capture::CaptureBackend;
use crate::config::Config;
use notify_rust::Notification;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
};
use wry::WebViewBuilder;

// ── icon ─────────────────────────────────────────────────────────────────────

fn make_icon() -> tray_icon::Icon {
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        let x = (i as u32) % SIZE;
        let y = (i as u32) / SIZE;
        let dx = x as i32 - 16;
        let dy = y as i32 - 16;
        if ((dx * dx + dy * dy) as f64).sqrt() < 14.0 {
            px[0] = 0; px[1] = 120; px[2] = 215; px[3] = 255;
        }
    }
    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).expect("icon")
}

// ── user events ──────────────────────────────────────────────────────────────

#[derive(Debug)]
enum AppEvent {
    ShowSettings,
    Quit,
    CaptureNow,
    PauseToggle,
    BrowseFolder,
    RefreshRunning,
    SendConfigToWebview,
}

// ── menu ids ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct MenuIds {
    pause: tray_icon::menu::MenuId,
    capture: tray_icon::menu::MenuId,
    settings: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

// ── run ──────────────────────────────────────────────────────────────────────

pub fn run(config: Arc<Mutex<Config>>, capture_now: Arc<AtomicBool>) {
    // ── Tray ─────────────────────────────────────────────────────────────────
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
    .expect("menu");

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Prntscrape")
        .with_icon(make_icon())
        .with_menu_on_left_click(false)
        .build()
        .expect("tray");

    let ids = MenuIds {
        pause: pause_item.id().clone(),
        capture: capture_item.id().clone(),
        settings: settings_item.id().clone(),
        quit: quit_item.id().clone(),
    };

    // ── Startup notification ──────────────────────────────────────────────────
    {
        let cfg = config.lock().unwrap();
        let backend = crate::capture::Backend::new();
        let running = backend.running_apps();
        let matched: Vec<String> = running
            .iter()
            .filter(|a| cfg.watchlist.iter().any(|w| a.name.to_lowercase().contains(w.as_str())))
            .map(|a| a.name.clone())
            .collect();
        let body = if matched.is_empty() {
            format!("In {} sec a screenshot will be taken when you open a watched app.", cfg.interval_secs)
        } else {
            format!("Detected {}\nIn {} sec a screenshot will be taken.", matched.join(", "), cfg.interval_secs)
        };
        let _ = Notification::new().summary("Prntscrape is running").body(&body).app_id("Prntscrape").show();
    }

    // ── Event loop ───────────────────────────────────────────────────────────
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // Background thread polls tray channels and sends AppEvents to the main loop
    {
        let proxy = proxy.clone();
        let ids = ids.clone();
        std::thread::spawn(move || loop {
            while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = ev
                {
                    let _ = proxy.send_event(AppEvent::ShowSettings);
                }
            }
            while let Ok(ev) = MenuEvent::receiver().try_recv() {
                if ev.id == ids.quit {
                    let _ = proxy.send_event(AppEvent::Quit);
                } else if ev.id == ids.settings {
                    let _ = proxy.send_event(AppEvent::ShowSettings);
                } else if ev.id == ids.capture {
                    let _ = proxy.send_event(AppEvent::CaptureNow);
                } else if ev.id == ids.pause {
                    let _ = proxy.send_event(AppEvent::PauseToggle);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        });
    }

    // ── Window ───────────────────────────────────────────────────────────────
    let window = WindowBuilder::new()
        .with_title("Prntscrape Settings")
        .with_inner_size(LogicalSize::new(820_f64, 720_f64))
        .with_min_inner_size(LogicalSize::new(600_f64, 500_f64))
        .with_visible(false)
        .with_resizable(true)
        .build(&event_loop)
        .unwrap();

    // ── WebView ──────────────────────────────────────────────────────────────
    let config_ipc = config.clone();
    let capture_ipc = capture_now.clone();
    let proxy_ipc = proxy.clone();

    // Inject initial config and running apps via init script so it's available the instant the
    // page script runs — no IPC round-trip, no timing race.
    let init_script = {
        let json = serde_json::to_string(&*config.lock().unwrap()).unwrap_or_default();
        let backend = crate::capture::Backend::new();
        let running = backend.running_apps();
        let running_json = serde_json::to_string(&running).unwrap_or_default();
        format!(
            "window.addEventListener('contextmenu',e=>e.preventDefault());\
             window.__INIT_CFG__={json};\
             window.__INIT_RUNNING__={running_json};"
        )
    };

    let webview = WebViewBuilder::new()
        .with_html(SETTINGS_HTML)
        .with_devtools(true)
        .with_initialization_script(&init_script)
        .with_ipc_handler(move |req: wry::http::Request<String>| {
            handle_ipc(req.body(), &config_ipc, &capture_ipc, &proxy_ipc);
        })
        .build(&window)
        .unwrap();

    // ── Main event loop ──────────────────────────────────────────────────────
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        // Keep tray/webview alive inside the closure
        let _ = &tray;
        let _ = &pause_item;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                window.set_visible(false);
            }

            Event::UserEvent(AppEvent::ShowSettings) => {
                window.set_visible(true);
                window.set_focus();
                // Push fresh config into the already-loaded page
                let json = serde_json::to_string(&*config.lock().unwrap())
                    .unwrap_or_default();
                let _ = webview.evaluate_script(&format!("loadConfig({})", json));

                // Also push fresh running apps
                let backend = crate::capture::Backend::new();
                let running = backend.running_apps();
                let running_json = serde_json::to_string(&running).unwrap_or_default();
                let _ = webview.evaluate_script(&format!("loadRunningApps({})", running_json));
            }

            Event::UserEvent(AppEvent::RefreshRunning) => {
                let backend = crate::capture::Backend::new();
                let running = backend.running_apps();
                let running_json = serde_json::to_string(&running).unwrap_or_default();
                let _ = webview.evaluate_script(&format!("loadRunningApps({})", running_json));
            }

            Event::UserEvent(AppEvent::SendConfigToWebview) => {
                let json = serde_json::to_string(&*config.lock().unwrap()).unwrap_or_default();
                let _ = webview.evaluate_script(&format!("loadConfig({})", json));

                let backend = crate::capture::Backend::new();
                let running = backend.running_apps();
                let running_json = serde_json::to_string(&running).unwrap_or_default();
                let _ = webview.evaluate_script(&format!("loadRunningApps({})", running_json));
            }

            Event::UserEvent(AppEvent::Quit) => {
                std::process::exit(0);
            }

            Event::UserEvent(AppEvent::CaptureNow) => {
                capture_now.store(true, Ordering::Relaxed);
            }

            Event::UserEvent(AppEvent::PauseToggle) => {
                let paused = {
                    let mut cfg = config.lock().unwrap();
                    cfg.paused = !cfg.paused;
                    let p = cfg.paused;
                    let _ = cfg.save();
                    p
                };
                pause_item.set_text(if paused { "▶  Resume" } else { "⏸  Pause" });
                let _ = tray.set_tooltip(Some(if paused { "Prntscrape (Paused)" } else { "Prntscrape" }));
            }

            Event::UserEvent(AppEvent::BrowseFolder) => {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    let s = path.to_string_lossy().into_owned();
                    let escaped = s.replace('\\', "\\\\").replace('\'', "\\'");
                    let _ = webview.evaluate_script(&format!("setSaveDir('{}')", escaped));
                }
            }

            _ => {}
        }
    });
}

// ── IPC handler ──────────────────────────────────────────────────────────────

fn handle_ipc(
    body: &str,
    config: &Arc<Mutex<Config>>,
    capture_now: &Arc<AtomicBool>,
    proxy: &tao::event_loop::EventLoopProxy<AppEvent>,
) {
    let Ok(msg) = serde_json::from_str::<serde_json::Value>(body) else { return };
    match msg["action"].as_str() {
        Some("save") => {
            if let Ok(cfg) = serde_json::from_value::<Config>(msg["data"].clone()) {
                let mut cur = config.lock().unwrap();
                *cur = cfg;
                let _ = cur.save();
            }
        }
        Some("browse") => { let _ = proxy.send_event(AppEvent::BrowseFolder); }
        Some("capture_now") => { capture_now.store(true, Ordering::Relaxed); }
        Some("refresh_running") => { let _ = proxy.send_event(AppEvent::RefreshRunning); }
        Some("request_config") => { let _ = proxy.send_event(AppEvent::SendConfigToWebview); }
        Some("log_error") => {
            if let Some(err) = msg["error"].as_str() {
                eprintln!("JS Error: {}", err);
                if let Some(proj_dirs) = directories::ProjectDirs::from("com", "prntscrape", "Prntscrape") {
                    let log_dir = proj_dirs.config_dir();
                    let _ = std::fs::create_dir_all(log_dir);
                    let _ = std::fs::write(log_dir.join("js_error.log"), err);
                }
            }
        }
        _ => {}
    }
}

// ── Settings HTML (Windows 11 Fluent Design) ─────────────────────────────────

const SETTINGS_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Prntscrape Settings</title>
<script>
window.onerror = function(message, source, lineno, colno, error) {
  const errText = message + " at " + source + ":" + lineno + ":" + colno;
  if (window.ipc && window.ipc.postMessage) {
    window.ipc.postMessage(JSON.stringify({action: 'log_error', error: errText}));
  }
};
</script>
<style>
:root {
  --bg:      #f3f3f3;
  --surface: #ffffff;
  --surf2:   #f2f2f2;
  --text:    #1b1b1b;
  --text2:   #616161;
  --accent:  #0067c0;
  --border:  rgba(0,0,0,0.07);
  color-scheme: light dark;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg:      #202020;
    --surface: #2d2d2d;
    --surf2:   #383838;
    --text:    #ffffff;
    --text2:   #9d9d9d;
    --accent:  #60cdff;
    --border:  rgba(255,255,255,0.07);
  }
}
*,*::before,*::after { box-sizing: border-box; margin: 0; padding: 0; }
html, body { height: 100%; }
body {
  font-family: "Segoe UI Variable Display","Segoe UI",system-ui,sans-serif;
  font-size: 14px;
  background: var(--bg);
  color: var(--text);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  -webkit-font-smoothing: antialiased;
}
::-webkit-scrollbar { width: 5px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: rgba(120,120,120,.4); border-radius: 3px; }

/* ── Layout ── */
.header {
  flex-shrink: 0;
  padding: 18px 20px 12px;
  border-bottom: 1px solid var(--border);
}
.header h1 { font-size: 18px; font-weight: 600; }

.scroll {
  flex: 1;
  overflow-y: auto;
  padding: 10px 20px 4px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.footer {
  flex-shrink: 0;
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  padding: 10px 20px;
  border-top: 1px solid var(--border);
}

/* ── Card / Row ── */
.card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
}
.row {
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 10px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border);
  min-height: 0;
}
.row:last-child { border-bottom: none; }
.lbl { flex: 1; min-width: 80px; }
.lbl .t { font-size: 14px; white-space: nowrap; }
.lbl .s { font-size: 11px; color: var(--text2); margin-top: 2px; white-space: nowrap; }
.ctrl { flex-shrink: 0; display: flex; align-items: center; gap: 6px; }

/* ── Segmented ── */
.seg {
  display: flex;
  background: var(--surf2);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 2px;
  gap: 2px;
}
.seg input[type=radio] { display: none; }
.seg label {
  padding: 4px 12px;
  border-radius: 3px;
  cursor: pointer;
  font-size: 13px;
  color: var(--text2);
  user-select: none;
  white-space: nowrap;
  transition: background .1s, color .1s;
}
.seg input:checked + label {
  background: var(--surface);
  color: var(--text);
  box-shadow: 0 1px 3px rgba(0,0,0,.12);
}
@media (prefers-color-scheme: dark) {
  .seg input:checked + label { box-shadow: 0 1px 4px rgba(0,0,0,.5); }
}

/* ── Pills ── */
.pills { display: flex; gap: 4px; align-items: center; }
.pills input[type=radio] { display: none; }
.pills label {
  padding: 4px 11px;
  border-radius: 14px;
  cursor: pointer;
  font-size: 12px;
  border: 1px solid var(--border);
  background: var(--surf2);
  color: var(--text);
  user-select: none;
  white-space: nowrap;
  transition: background .12s, border-color .12s;
}
.pills input:checked + label {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}
@media (prefers-color-scheme: dark) { .pills input:checked + label { color: #111; } }

/* ── Toggle ── */
.toggle { position: relative; width: 40px; height: 20px; flex-shrink: 0; }
.toggle input { opacity: 0; position: absolute; }
.trk {
  position: absolute; inset: 0;
  background: var(--surf2);
  border: 1.5px solid rgba(128,128,128,.45);
  border-radius: 10px;
  cursor: pointer;
  transition: background .2s, border-color .2s;
}
.toggle input:checked ~ .trk { background: var(--accent); border-color: var(--accent); }
.thmb {
  position: absolute; top: 3px; left: 3px;
  width: 12px; height: 12px;
  border-radius: 50%;
  background: rgba(100,100,100,.85);
  transition: transform .2s, background .2s;
  pointer-events: none;
}
.toggle input:checked ~ .trk .thmb { transform: translateX(20px); background: #fff; }

/* ── Inputs / Buttons ── */
.inp {
  padding: 5px 9px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surf2);
  color: var(--text);
  font: inherit;
  font-size: 13px;
  outline: none;
  transition: border-color .15s;
  min-width: 0;
}
.inp:focus { border-color: var(--accent); }
input[type=number].inp { -moz-appearance: textfield; }
input[type=number].inp::-webkit-inner-spin-button { opacity: 1; }

.btn {
  padding: 5px 14px;
  border-radius: 4px;
  border: 1px solid var(--border);
  background: var(--surf2);
  color: var(--text);
  font: inherit;
  font-size: 13px;
  cursor: pointer;
  white-space: nowrap;
  flex-shrink: 0;
  transition: filter .1s;
}
.btn:hover  { filter: brightness(.96); }
.btn:active { filter: brightness(.90); }
.btn-accent { background: var(--accent); border-color: var(--accent); color: #fff; }
@media (prefers-color-scheme: dark) { .btn-accent { color: #111; } }
.btn-accent:hover { filter: brightness(1.1); }

/* ── Slider ── */
.slider-row { display: flex; align-items: center; gap: 8px; }
input[type=range] {
  -webkit-appearance: none;
  width: 140px; height: 4px; border-radius: 2px;
  background: linear-gradient(to right, var(--accent) var(--v,50%), var(--surf2) var(--v,50%));
  outline: none; flex-shrink: 0;
}
input[type=range]::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 16px; height: 16px; border-radius: 50%;
  background: var(--accent);
  border: 2.5px solid var(--surface);
  box-shadow: 0 1px 3px rgba(0,0,0,.25);
  cursor: pointer;
}
.qv { font-size: 13px; color: var(--text2); min-width: 26px; }

/* ── Watchlist ── */
.tags {
  display: flex; flex-wrap: wrap; gap: 5px;
  padding: 8px 14px;
  min-height: 36px;
}
.tag {
  display: flex; align-items: center; gap: 3px;
  padding: 3px 6px 3px 9px;
  background: var(--surf2);
  border: 1px solid var(--border);
  border-radius: 12px;
  font-size: 12px;
}
.tag-x {
  background: none; border: none;
  color: var(--text2); cursor: pointer;
  font-size: 14px; line-height: 1; padding: 0 2px;
}
.tag-x:hover { color: #c42b1c; }
.add-row { display: flex; gap: 8px; padding: 6px 14px 10px; }

/* ── Misc ── */
#q-row { display: none; }
#q-row.show { display: flex; }
</style>
</head>
<body>

<div class="header"><h1>Prntscrape Settings</h1></div>

<div class="scroll">

  <div class="card">
    <div class="row">
      <div class="lbl"><div class="t">Mode</div></div>
      <div class="ctrl">
        <div class="seg">
          <input type="radio" name="mode" id="m-cap" value="capture">
          <label for="m-cap">Auto Capture</label>
          <input type="radio" name="mode" id="m-not" value="notify">
          <label for="m-not">Just Remind Me</label>
        </div>
      </div>
    </div>

    <div class="row">
      <div class="lbl">
        <div class="t">Interval</div>
        <div class="s">How often to capture</div>
      </div>
      <div class="ctrl" style="flex-wrap:wrap;gap:6px">
        <div class="pills">
          <input type="radio" name="iv" id="iv30"  value="30" onchange="ivPreset(30)"><label  for="iv30">30s</label>
          <input type="radio" name="iv" id="iv60"  value="60" onchange="ivPreset(60)"><label  for="iv60">1m</label>
          <input type="radio" name="iv" id="iv120" value="120" onchange="ivPreset(120)"><label for="iv120">2m</label>
          <input type="radio" name="iv" id="iv300" value="300" onchange="ivPreset(300)"><label for="iv300">5m</label>
          <input type="radio" name="iv" id="iv600" value="600" onchange="ivPreset(600)"><label for="iv600">10m</label>
        </div>
        <div style="display:flex;align-items:center;gap:5px">
          <input id="iv-custom" class="inp" type="number" min="5" max="86400" style="width:70px" placeholder="sec" oninput="ivCustom()">
          <span style="font-size:12px;color:var(--text2)">sec</span>
        </div>
      </div>
    </div>

    <div class="row">
      <div class="lbl"><div class="t">Capture region</div></div>
      <div class="ctrl">
        <div class="seg">
          <input type="radio" name="region" id="r-win" value="window">
          <label for="r-win">Active Window</label>
          <input type="radio" name="region" id="r-mon" value="monitor">
          <label for="r-mon">Full Monitor</label>
        </div>
      </div>
    </div>
  </div>

  <div class="card">
    <div class="row">
      <div class="lbl"><div class="t">Save folder</div></div>
      <div class="ctrl" style="flex:1;min-width:0">
        <input id="savedir" class="inp" type="text" style="flex:1;min-width:0">
        <button class="btn" onclick="sendIpc('browse',{})">Browse…</button>
      </div>
    </div>
    <div class="row">
      <div class="lbl">
        <div class="t">Project name</div>
        <div class="s">Blank = per-app folders</div>
      </div>
      <div class="ctrl">
        <input id="project" class="inp" type="text" style="width:180px" placeholder="optional">
      </div>
    </div>
  </div>

  <div class="card">
    <div class="row">
      <div class="lbl"><div class="t">Format</div></div>
      <div class="ctrl">
        <div class="seg">
          <input type="radio" name="fmt" id="f-png"  value="png"  onchange="fmtChange()">
          <label for="f-png">PNG</label>
          <input type="radio" name="fmt" id="f-jpeg" value="jpeg" onchange="fmtChange()">
          <label for="f-jpeg">JPEG</label>
        </div>
      </div>
    </div>
    <div class="row" id="q-row">
      <div class="lbl"><div class="t">JPEG quality</div></div>
      <div class="ctrl slider-row">
        <input id="qual" type="range" min="50" max="100" value="80" oninput="qualChange(this)">
        <span id="qval" class="qv">80</span>
      </div>
    </div>
  </div>

  <div class="card">
    <div class="row">
      <div class="lbl">
        <div class="t">Skip unchanged frames</div>
        <div class="s">Don't save if screen hasn't changed</div>
      </div>
      <div class="ctrl">
        <label class="toggle">
          <input id="skip" type="checkbox">
          <div class="trk"><div class="thmb"></div></div>
        </label>
      </div>
    </div>
    <div class="row">
      <div class="lbl"><div class="t">Paused</div></div>
      <div class="ctrl">
        <label class="toggle">
          <input id="paused" type="checkbox">
          <div class="trk"><div class="thmb"></div></div>
        </label>
      </div>
    </div>
  </div>

  <div class="card">
    <div class="row" style="border-bottom:1px solid var(--border)">
      <div class="lbl">
        <div class="t">Watchlist</div>
        <div class="s">Apps to watch for</div>
      </div>
    </div>
    <div class="tags" id="tags"></div>
    <div class="add-row">
      <input id="new-e" class="inp" type="text" style="flex:1" placeholder="Add app name…">
      <button class="btn btn-accent" onclick="addEntry()">Add</button>
    </div>
    <div id="running-apps-section" style="padding: 10px 14px 14px; border-top: 1px dashed var(--border);">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
        <span style="font-size: 12px; font-weight: 600; color: var(--text2)">Currently Running Apps (click to add):</span>
        <button class="btn" style="padding: 2px 8px; font-size: 11px;" onclick="sendIpc('refresh_running', {})">Refresh</button>
      </div>
      <div id="running-pills" style="display: flex; flex-wrap: wrap; gap: 6px; max-height: 120px; overflow-y: auto; padding: 2px 0;">
        <!-- Running apps will be inserted here -->
      </div>
    </div>
  </div>

</div>

<div class="footer">
  <button class="btn" onclick="sendIpc('capture_now',{})">Capture Now</button>
  <button class="btn btn-accent" onclick="save()">Save Settings</button>
</div>

<script>
const PRESETS = [30,60,120,300,600];
var watchlist = [];
var runningApps = [];
var loading = false;

function loadConfig(cfg) {
  loading = true;

  const m = document.querySelector(`input[name=mode][value="${cfg.mode}"]`);
  if (m) m.checked = true;

  applyIv(cfg.interval_secs || 120);

  const r = document.querySelector(`input[name=region][value="${cfg.capture_region}"]`);
  if (r) r.checked = true;

  const sd = document.getElementById('savedir');
  if (sd) sd.value = cfg.save_directory || '';

  const pj = document.getElementById('project');
  if (pj) pj.value = cfg.current_project || '';

  const f = document.querySelector(`input[name=fmt][value="${cfg.format}"]`);
  if (f) f.checked = true;
  fmtChange();

  const q = document.getElementById('qual');
  if (q) {
    q.value = cfg.quality || 80;
    qualChange(q);
  }

  const sk = document.getElementById('skip');
  if (sk) sk.checked = !!cfg.skip_unchanged;

  const ps = document.getElementById('paused');
  if (ps) ps.checked = !!cfg.paused;

  watchlist = cfg.watchlist ? [...cfg.watchlist] : [];
  renderTags();

  loading = false;
}

function loadRunningApps(running) {
  runningApps = running ? [...running] : [];
  renderRunningApps();
}

function renderRunningApps() {
  const c = document.getElementById('running-pills');
  if (!c) return;
  c.innerHTML = '';
  if (runningApps.length === 0) {
    c.innerHTML = '<span style="font-size:12px;color:var(--text2);font-style:italic;">No active programs detected. Click Refresh to reload.</span>';
    return;
  }
  runningApps.forEach(app => {
    const alreadyAdded = watchlist.includes(app.name.toLowerCase());
    const btn = document.createElement('button');
    btn.className = 'btn';
    btn.style.padding = '3px 8px';
    btn.style.borderRadius = '12px';
    btn.style.fontSize = '12px';
    btn.style.background = alreadyAdded ? 'var(--surf2)' : 'var(--surface)';
    btn.style.borderColor = alreadyAdded ? 'transparent' : 'var(--border)';
    btn.style.color = alreadyAdded ? 'var(--text2)' : 'var(--text)';
    btn.style.cursor = alreadyAdded ? 'default' : 'pointer';
    btn.style.opacity = alreadyAdded ? '0.6' : '1';
    btn.textContent = app.name;
    if (!alreadyAdded) {
      btn.onclick = () => addRunningApp(app.name);
    }
    c.appendChild(btn);
  });
}

function applyIv(secs) {
  const custom = document.getElementById('iv-custom');
  if (custom) custom.value = secs;
  const match = PRESETS.includes(secs) ? document.getElementById('iv' + secs) : null;
  const radios = document.querySelectorAll('input[name=iv]');
  for (let i = 0; i < radios.length; i++) {
    radios[i].checked = false;
  }
  if (match) match.checked = true;
}

// Preset and Custom inputs trigger save() immediately on change
function ivPreset(v) { 
  const custom = document.getElementById('iv-custom');
  if (custom) custom.value = v; 
  applyIv(v); 
  save(); 
}
function ivCustom() {
  const custom = document.getElementById('iv-custom');
  if (!custom) return;
  const v = parseInt(custom.value);
  const radios = document.querySelectorAll('input[name=iv]');
  for (let i = 0; i < radios.length; i++) {
    radios[i].checked = false;
  }
  if (PRESETS.includes(v)) {
    const el = document.getElementById('iv' + v);
    if (el) el.checked = true;
  }
  save();
}

function fmtChange() {
  const qrow = document.getElementById('q-row');
  const fjpe = document.getElementById('f-jpeg');
  if (qrow && fjpe) {
    qrow.classList.toggle('show', fjpe.checked);
  }
  save();
}
function qualChange(el) {
  const pct = (el.value - el.min) / (el.max - el.min) * 100;
  el.style.setProperty('--v', pct + '%');
  const qval = document.getElementById('qval');
  if (qval) qval.textContent = el.value;
}

function addListener(id, event, cb) {
  const el = document.getElementById(id);
  if (el) el.addEventListener(event, cb);
}

addListener('qual', 'change', save);
addListener('skip', 'change', save);
addListener('paused', 'change', save);
addListener('savedir', 'change', save);
addListener('project', 'change', save);

const modes = document.querySelectorAll('input[name=mode]');
for (let i = 0; i < modes.length; i++) {
  modes[i].addEventListener('change', save);
}
const regions = document.querySelectorAll('input[name=region]');
for (let i = 0; i < regions.length; i++) {
  regions[i].addEventListener('change', save);
}

function renderTags() {
  const c = document.getElementById('tags');
  if (!c) return;
  c.innerHTML = '';
  watchlist.forEach((e, i) => {
    const d = document.createElement('div');
    d.className = 'tag';
    d.innerHTML = `<span>${e}</span><button class="tag-x" onclick="removeEntry(${i})">&#x2715;</button>`;
    c.appendChild(d);
  });
  renderRunningApps();
}
function removeEntry(i) {
  watchlist.splice(i, 1);
  renderTags();
  save();
}
function addEntry() {
  const el = document.getElementById('new-e');
  if (!el) return;
  const v = el.value.trim().toLowerCase();
  if (v && !watchlist.includes(v)) {
    watchlist.push(v);
    renderTags();
    save();
  }
  el.value = '';
}
function addRunningApp(name) {
  const v = name.trim().toLowerCase();
  if (v && !watchlist.includes(v)) {
    watchlist.push(v);
    renderTags();
    save();
  }
}

const newe = document.getElementById('new-e');
if (newe) {
  newe.addEventListener('keydown', e => { if (e.key==='Enter') addEntry(); });
}

function getData() {
  const modeVal = document.querySelector('input[name=mode]:checked')?.value || 'capture';
  const ivVal = parseInt(document.getElementById('iv-custom')?.value) || 120;
  const regVal = document.querySelector('input[name=region]:checked')?.value || 'window';
  const dirVal = document.getElementById('savedir')?.value || '';
  const projVal = document.getElementById('project')?.value.trim() || null;
  const fmtVal = document.querySelector('input[name=fmt]:checked')?.value || 'png';
  const qualVal = parseInt(document.getElementById('qual')?.value) || 80;
  const skipVal = document.getElementById('skip')?.checked || false;
  const pausedVal = document.getElementById('paused')?.checked || false;

  return {
    mode:           modeVal,
    interval_secs:  ivVal,
    capture_region: regVal,
    save_directory: dirVal,
    current_project: projVal,
    format:         fmtVal,
    quality:        qualVal,
    skip_unchanged: skipVal,
    paused:         pausedVal,
    watchlist:      [...watchlist],
  };
}

function save() { 
  if (loading) return;
  sendIpc('save', { data: getData() }); 
}
function setSaveDir(p) { 
  const sd = document.getElementById('savedir');
  if (sd) sd.value = p; 
  save(); 
}
function sendIpc(action, extra) { window.ipc.postMessage(JSON.stringify({action,...extra})); }

const qualEl = document.getElementById('qual');
if (qualEl) qualChange(qualEl);

// __INIT_CFG__ is injected by Rust's initialization script before this code runs.
// loadConfig is called here (DOM is ready, all functions defined) — no timing race.
if (window.__INIT_CFG__) {
  loadConfig(window.__INIT_CFG__);
} else {
  setTimeout(function() {
    sendIpc('request_config', {});
  }, 100);
}
if (window.__INIT_RUNNING__) {
  loadRunningApps(window.__INIT_RUNNING__);
}
</script>
</body>
</html>"#;
