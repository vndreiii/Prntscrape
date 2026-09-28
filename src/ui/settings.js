
const PRESETS = [30,60,120,300,600];
var watchlist = [];
var runningApps = [];
var loading = false;

function showMessage(message) {
  document.getElementById('save-status').textContent = message;
}

function loadUpdate(status) {
  document.getElementById('update-status').textContent = status.message;
  document.getElementById('check-updates').disabled = status.phase === 'checking' || status.phase === 'updated';
  document.getElementById('restart').hidden = status.phase !== 'updated';
}

window.addEventListener('error', event => {
  sendIpc('log_error', {error: event.message});
});

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
    const label = document.createElement('span');
    label.textContent = e;
    const remove = document.createElement('button');
    remove.className = 'tag-x';
    remove.textContent = '×';
    remove.setAttribute('aria-label', 'Remove ' + e);
    remove.onclick = () => removeEntry(i);
    d.append(label, remove);
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
  const ivVal = Number(document.getElementById('iv-custom')?.value);
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
  const interval = document.getElementById('iv-custom');
  if (!interval.value || !interval.reportValidity()) return;
  if (!document.getElementById('savedir').value.trim()) {
    showMessage('Choose a save folder.');
    return;
  }
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

if (window.__INIT__) {
  document.getElementById('platform-help').textContent = window.__INIT__.help;
  document.getElementById('app-version').textContent = 'v' + window.__INIT__.version;
  const error = document.getElementById('backend-error');
  error.hidden = !window.__INIT__.backend_error;
  error.textContent = window.__INIT__.backend_error || '';
}
sendIpc('request_config', {});
