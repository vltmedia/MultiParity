// Record a tutorial scene: headless Edge or Chrome, real clicks and typing, a drawn cursor that
// glides between targets, and a log of where each callout belongs.
//
//   node record.mjs scenes/t01-setup.mjs out/t01-setup
//
// A scene is a module whose default export is async (r) => { ... } using the recorder below.
// Beats line up with the voice lines in ../vo-lines.json: r.beat(id) starts a line, and waits for
// the previous line to finish first. Output: frames, video.mp4 (1920x1080, 30 fps) and
// timeline.json ({ beats: [{id, t}], callouts: [{t, dur, style, rect, label}], duration }).
import { spawn, execSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [, , sceneFile, outDir] = process.argv;
const HERE = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Z]:)/, '$1'));
const VO = JSON.parse(fs.readFileSync(path.join(HERE, '..', 'vo-lines.json'), 'utf8'));
const sceneId = path.basename(sceneFile, '.mjs');
const lines = Object.fromEntries((VO[sceneId] || []).map((l) => [l.id, l.dur]));
let W = 1600, H = 900, SCALE = 1.2; // 1920 x 1080 frames; a scene can pick a smaller window (options.view)
const GAP = 0.45; // seconds between voice lines

const BROWSERS = ['C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe', 'C:/Program Files/Google/Chrome/Application/chrome.exe'];
const exe = BROWSERS.find((b) => fs.existsSync(b));
const PORT = 9340 + Math.floor(Math.random() * 50);
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'mp-rec-'));
const browser = spawn(exe, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`, '--hide-scrollbars', '--no-first-run', '--force-color-profile=srgb', '--disable-features=Translate', 'about:blank'], { stdio: 'ignore' });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function json(p, method = 'GET') {
  for (let i = 0; i < 60; i++) {
    try { return await (await fetch(`http://127.0.0.1:${PORT}${p}`, { method })).json(); } catch { await sleep(200); }
  }
  throw new Error('The browser did not start.');
}
function connect(wsUrl) {
  const ws = new WebSocket(wsUrl);
  let id = 0; const waiting = new Map(); const listeners = [];
  ws.onmessage = (m) => {
    const msg = JSON.parse(m.data);
    if (msg.id && waiting.has(msg.id)) { const { ok, fail } = waiting.get(msg.id); waiting.delete(msg.id); msg.error ? fail(new Error(msg.error.message)) : ok(msg.result); }
    else listeners.forEach((l) => l(msg));
  };
  const send = (method, params = {}) => new Promise((ok, fail) => { const i = ++id; waiting.set(i, { ok, fail }); ws.send(JSON.stringify({ id: i, method, params })); });
  const on = (fn) => listeners.push(fn);
  return new Promise((ok) => { ws.onopen = () => ok({ send, on }); });
}

// Injected into every page: the cursor, text swaps (scratch addresses -> studio names), toasts.
const PAGE_KIT = (swaps, subSwaps) => `(() => {
  if (window.__kit) return; window.__kit = true;
  // The desktop app re-reads a folder every 10 s; on camera an edit should show within 2 s.
  setInterval(() => { try { if (typeof refreshWork === 'function' && typeof work !== 'undefined' && work.ws && !work.busy && work.tab === 'work' && document.getElementById('v-work').classList.contains('on')) refreshWork(false); } catch (_) {} }, 2000);
  const SW = ${JSON.stringify(swaps)};
  const fix = (n) => { let v = n.nodeValue, o = v; for (const [a, b] of SW) v = v.split(a).join(b); if (v !== o) n.nodeValue = v; };
  const sweep = (root) => { const w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT); let n; while ((n = w.nextNode())) fix(n); };
  const fixInputs = () => document.querySelectorAll('input,textarea').forEach((i) => { if (document.activeElement === i) return; let v = i.value, o = v; for (const [a, b] of SW) v = v.split(a).join(b); if (v !== o) i.value = v; });
  const start = () => {
    sweep(document.body); fixInputs();
    new MutationObserver((ms) => { for (const m of ms) { if (m.type === 'characterData') fix(m.target); m.addedNodes.forEach((n) => n.nodeType === 3 ? fix(n) : n.nodeType === 1 && sweep(n)); } fixInputs(); })
      .observe(document.body, { subtree: true, childList: true, characterData: true });
    const c = document.createElement('div'); c.id = '__cur';
    c.innerHTML = '<svg width="30" height="30" viewBox="0 0 30 30"><path d="M5 3 L5 24 L10.5 18.6 L14.2 27 L18 25.4 L14.4 17.2 L22 17.2 Z" fill="#fff" stroke="#111" stroke-width="1.6" stroke-linejoin="round"/></svg>';
    c.style.cssText = 'position:fixed;left:0;top:0;width:30px;height:30px;margin:-3px 0 0 -5px;z-index:2147483647;pointer-events:none;filter:drop-shadow(0 2px 3px rgba(0,0,0,.45));transition:transform .09s;transform-origin:5px 3px;will-change:transform';
    document.documentElement.appendChild(c);
    const p = window.__curPos || [800, 450]; c.style.translate = p[0] + 'px ' + p[1] + 'px';
  };
  if (document.body) start(); else document.addEventListener('DOMContentLoaded', start);
  // What people type on screen (the studio's address) is sent as the demo server's real one.
  const SUB = ${JSON.stringify(subSwaps)};
  document.addEventListener('submit', (e) => {
    const changed = [];
    e.target.querySelectorAll('input').forEach((i) => { for (const [a, b] of SUB) if (i.value.trim() === a) { changed.push([i, i.value]); i.value = b; } });
    setTimeout(() => changed.forEach(([i, v]) => { i.value = v; }), 0);
  }, true);
  window.__curTo = (x, y) => { window.__curPos = [x, y]; const c = document.getElementById('__cur'); if (c) c.style.translate = x + 'px ' + y + 'px'; };
  window.__ripple = (x, y) => {
    const c = document.getElementById('__cur'); if (c) { c.style.transform = 'scale(.82)'; setTimeout(() => c.style.transform = '', 140); }
    const r = document.createElement('div');
    r.style.cssText = 'position:fixed;left:' + (x - 22) + 'px;top:' + (y - 22) + 'px;width:44px;height:44px;border-radius:50%;border:2.5px solid rgba(38,202,243,.95);background:rgba(38,202,243,.18);z-index:2147483646;pointer-events:none;transform:scale(.3);opacity:1;transition:transform .45s cubic-bezier(.2,.8,.2,1),opacity .45s';
    document.documentElement.appendChild(r); requestAnimationFrame(() => { r.style.transform = 'scale(1)'; r.style.opacity = '0'; }); setTimeout(() => r.remove(), 600);
  };
  window.__toast = (title, body, ms) => {
    const t = document.createElement('div');
    t.innerHTML = '<div style="display:flex;gap:10px;align-items:center;margin-bottom:8px;font:600 12px Segoe UI,sans-serif;color:#cfd8e0"><img src="data:image/png;base64,__ICON__" width="16" height="16" style="border-radius:3px">MultiParity</div><div style="font:600 14px Segoe UI,sans-serif;color:#fff;margin-bottom:3px"></div><div style="font:13px Segoe UI,sans-serif;color:#c9d2da;line-height:1.4"></div>';
    t.children[1].textContent = title; t.children[2].textContent = body;
    t.style.cssText = 'position:fixed;right:18px;bottom:18px;width:340px;padding:14px 16px;background:rgba(32,36,42,.97);border:1px solid rgba(255,255,255,.08);border-radius:8px;box-shadow:0 12px 32px rgba(0,0,0,.45);z-index:2147483645;transform:translateX(380px);transition:transform .45s cubic-bezier(.2,.8,.2,1)';
    document.documentElement.appendChild(t); requestAnimationFrame(() => t.style.transform = 'none');
    setTimeout(() => { t.style.transform = 'translateX(380px)'; setTimeout(() => t.remove(), 500); }, ms || 5000);
  };
  window.__find = (spec) => {
    const vis = (e) => { const r = e.getBoundingClientRect(); const s = getComputedStyle(e); return r.width > 2 && r.height > 2 && s.visibility !== 'hidden' && s.display !== 'none'; };
    let scope = document;
    if (spec.within) { const ws = [...document.querySelectorAll(spec.within)].filter(vis); scope = ws[spec.withinIndex || 0]; if (!scope) return null; }
    let els;
    if (spec.css) els = [...scope.querySelectorAll(spec.css)].filter(vis);
    else {
      const want = spec.text.toLowerCase();
      const cand = [...scope.querySelectorAll(spec.sel || 'button,a,[role=tab],[role=menuitem],[role=button],label,summary,li,td,th,span,strong,b,small,div,p,h1,h2,h3,input,textarea,option,dt,dd,code')].filter(vis);
      const txt = (e) => (e.value && e.tagName === 'INPUT' ? e.value : (e.textContent || '')).replace(/\\s+/g, ' ').trim().toLowerCase();
      els = cand.filter((e) => txt(e) === want);
      if (!els.length && !spec.exact) els = cand.filter((e) => txt(e).includes(want));
      // Prefer the innermost match, then a clickable ancestor if asked.
      els = els.filter((e) => !els.some((o) => o !== e && e.contains(o)));
    }
    const e = spec.last ? els[els.length - 1] : els[spec.index || 0]; if (!e) return null;
    const t = spec.up ? e.closest(spec.up) || e : e;
    const r0 = t.getBoundingClientRect();
    if ((r0.top < 0 || r0.bottom > innerHeight - 10) && !t.closest('dialog,[role=dialog],#ctx') && !window.__scrolledFor?.has(t)) {
      (window.__scrolledFor ||= new WeakSet()).add(t);
      t.scrollIntoView({ block: 'center', behavior: 'smooth' }); return { scrolled: true };
    }
    return { x: r0.left, y: r0.top, w: r0.width, h: r0.height };
  };
})();`;

const ICON = fs.readFileSync(path.join(HERE, 'icon32.png')).toString('base64');

async function main() {
  fs.rmSync(outDir, { recursive: true, force: true });
  fs.mkdirSync(path.join(outDir, 'frames'), { recursive: true });
  await json('/json/version');
  const target = await json('/json/new?about:blank', 'PUT');
  const cdp = await connect(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable'); await cdp.send('Runtime.enable');
  // No white flash between pages: the app's own dark ground while one loads.
  await cdp.send('Emulation.setDefaultBackgroundColorOverride', { color: { r: 5, g: 13, b: 20, a: 1 } });
  const scene = (await import(pathToFileURL(path.resolve(sceneFile)).href)).default;
  const opts = scene.options || {};
  if (opts.view) { W = opts.view; H = Math.round(W * 9 / 16); SCALE = 1920 / W; }
  await cdp.send('Emulation.setDeviceMetricsOverride', { width: W, height: H, deviceScaleFactor: SCALE, mobile: false });
  await cdp.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: opts.scheme || 'dark' }] });
  const swaps = opts.swaps || [['127.0.0.1:18570', 'mp.cinderworks.studio'], ['http://mp.cinderworks.studio', 'https://mp.cinderworks.studio']];
  const subSwaps = opts.submitSwaps || [['mp.cinderworks.studio', 'http://127.0.0.1:18570'], ['https://mp.cinderworks.studio', 'http://127.0.0.1:18570']];
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', { source: PAGE_KIT(swaps, subSwaps).replace('__ICON__', ICON) });

  let cur = [W / 2, H / 2];
  const frames = []; let recording = false, t0 = 0;
  const beats = [], callouts = [];
  let beatEnd = 0; // recording-clock time the current voice line ends
  const now = () => Date.now() / 1000 - t0;
  cdp.on(async (m) => {
    if (m.method !== 'Page.screencastFrame') return;
    cdp.send('Page.screencastFrameAck', { sessionId: m.params.sessionId }).catch(() => {});
    if (!recording) return;
    const f = path.resolve(outDir, 'frames', `f${String(frames.length).padStart(6, '0')}.jpg`);
    fs.writeFileSync(f, Buffer.from(m.params.data, 'base64'));
    frames.push({ f, t: Date.now() / 1000 - t0 });
  });

  const ev = async (expr) => {
    const r = await cdp.send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    return r.result.value;
  };
  const setCur = () => ev(`window.__curTo && window.__curTo(${cur[0]},${cur[1]})`).catch(() => {});
  const mouse = (type, x, y, extra = {}) => cdp.send('Input.dispatchMouseEvent', { type, x, y, ...extra });

  async function find(spec, timeout = 8000) {
    if (typeof spec === 'string') spec = { text: spec };
    const end = Date.now() + timeout;
    while (Date.now() < end) {
      const r = await ev(`window.__find(${JSON.stringify(spec)})`).catch(() => null);
      if (r && !r.scrolled) return r;
      await sleep(r && r.scrolled ? 650 : 150);
    }
    throw new Error(`Not found: ${JSON.stringify(spec)}`);
  }
  async function moveTo(x, y, ms) {
    const [sx, sy] = cur; const d = Math.hypot(x - sx, y - sy);
    ms = ms ?? Math.min(1150, 380 + d * 0.55);
    const steps = Math.max(8, Math.round(ms / 16));
    // A slight arc, like a hand, not a ruler.
    const bend = Math.min(60, d * 0.12) * (sx < x ? -1 : 1);
    for (let i = 1; i <= steps; i++) {
      const k = i / steps; const e = k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2;
      const arc = Math.sin(Math.PI * e) * bend;
      const nx = sx + (x - sx) * e + (-(y - sy) / (d || 1)) * arc, ny = sy + (y - sy) * e + ((x - sx) / (d || 1)) * arc;
      cur = [nx, ny]; await setCur();
      if (i % 3 === 0 || i === steps) await mouse('mouseMoved', nx, ny);
      await sleep(16);
    }
  }
  const centre = (r, at) => at ? [r.x + r.w * at[0], r.y + r.h * at[1]] : [r.x + Math.min(r.w / 2, 60 + r.w * 0.15), r.y + r.h / 2];
  const toOut = (r, pad = 6) => ({ x: (r.x - pad) * SCALE, y: (r.y - pad) * SCALE, w: (r.w + 2 * pad) * SCALE, h: (r.h + 2 * pad) * SCALE });

  const rec = {
    get W() { return W; }, get H() { return H; },
    sleep,
    eval: ev,
    sh: (cmd) => execSync(cmd, { shell: 'C:/Program Files/Git/usr/bin/bash.exe', stdio: 'pipe', encoding: 'utf8' }),
    // For commands that leave a process running (the daemon): no pipes for it to hold open.
    shDetached: (cmd) => execSync(cmd, { shell: 'C:/Program Files/Git/usr/bin/bash.exe', stdio: 'ignore' }),
    async open(url, wait = 1500) {
      const loaded = new Promise((ok) => { const l = (m) => { if (m.method === 'Page.loadEventFired') ok(); }; cdp.on(l); });
      await cdp.send('Page.navigate', { url }); await loaded; await sleep(wait); await setCur();
    },
    async reload(wait = 1200) { await ev('location.reload()').catch(() => {}); await sleep(wait); await setCur(); },
    find,
    async hover(spec, o = {}) { const r = await find(spec); const [x, y] = centre(r, o.at); await moveTo(x, y, o.ms); await sleep(o.after ?? 250); return r; },
    async click(spec, o = {}) {
      const r = await find(spec); const [x, y] = centre(r, o.at); await moveTo(x, y, o.ms); await sleep(o.before ?? 180);
      const button = o.right ? 'right' : 'left';
      await ev(`window.__ripple(${x},${y})`).catch(() => {});
      // o.shift: a shift-click (ranges); the drawn cursor looks the same.
      const modifiers = o.shift ? 8 : 0;
      await mouse('mousePressed', x, y, { button, clickCount: 1, buttons: o.right ? 2 : 1, modifiers });
      await sleep(70); await mouse('mouseReleased', x, y, { button, clickCount: 1, modifiers });
      await sleep(o.after ?? 450); await setCur(); return r;
    },
    // A <select>: native dropdowns do not show in a recording, so click it, then pick by text the
    // way React sees a real choice.
    async choose(spec, optionText, o = {}) {
      await rec.click(spec, { after: 350, ...o });
      await ev(`(() => { const s = document.activeElement.tagName === 'SELECT' ? document.activeElement : document.querySelector(${JSON.stringify(spec.css || 'select')});
        const opt = [...s.options].find((x) => x.textContent.trim().startsWith(${JSON.stringify(optionText)})); if (!opt) throw new Error('no option');
        const set = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set; set.call(s, opt.value);
        s.dispatchEvent(new Event('change', { bubbles: true })); s.dispatchEvent(new Event('input', { bubbles: true })); s.blur(); })()`);
      await sleep(o.after ?? 400);
    },
    async moveBy(dx, dy, ms) { await moveTo(cur[0] + dx, cur[1] + dy, ms); },
    async park(x, y, ms) { await moveTo(Math.min(x * W / 1600, W - 20), Math.min(y * H / 900, H - 20), ms); },
    async type(text, o = {}) {
      for (const ch of text) {
        await cdp.send('Input.insertText', { text: ch });
        await sleep((o.cps ? 1000 / o.cps : 55) + Math.random() * 45 + (ch === ' ' ? 30 : 0));
      }
      await sleep(o.after ?? 300);
    },
    async press(combo) {
      const parts = combo.split('+'); const key = parts.pop();
      const mods = (parts.includes('ctrl') ? 2 : 0) | (parts.includes('shift') ? 8 : 0);
      const named = { Enter: [13, 'Enter', '\r'], Tab: [9, 'Tab'], Escape: [27, 'Escape'], Backspace: [8, 'Backspace'] }[key];
      const [vk, code, text] = named || [key.toUpperCase().charCodeAt(0), 'Key' + key.toUpperCase()];
      await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', modifiers: mods, key, code, windowsVirtualKeyCode: vk, text: mods ? undefined : text });
      await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', modifiers: mods, key, code, windowsVirtualKeyCode: vk });
      await sleep(250);
    },
    async scroll(dy, o = {}) {
      const steps = Math.max(4, Math.round(Math.abs(dy) / 60));
      for (let i = 0; i < steps; i++) { await mouse('mouseWheel', cur[0], cur[1], { deltaX: 0, deltaY: dy / steps }); await sleep(28); }
      await sleep(o.after ?? 500);
    },
    async toast(title, body, ms = 5200) { await ev(`window.__toast(${JSON.stringify(title)}, ${JSON.stringify(body)}, ${ms})`); },
    // Callouts are drawn later on the Kitlig timeline; here we note when and where.
    async callout(style, spec, o = {}) {
      const r = spec.rect ? spec.rect : await find(spec);
      callouts.push({ t: +(now() + (o.delay || 0)).toFixed(3), dur: o.dur ?? 2.6, style, rect: toOut(r, o.pad ?? 6), label: o.label || '' });
      return r;
    },
    async record() {
      t0 = Date.now() / 1000; recording = true;
      await cdp.send('Page.startScreencast', { format: 'jpeg', quality: 92, maxWidth: W * SCALE, maxHeight: H * SCALE, everyNthFrame: 1 });
      await setCur(); await sleep(400);
    },
    async beat(id) {
      if (!(id in lines)) throw new Error(`No voice line ${sceneId}/${id}`);
      const wait = beatEnd - now(); if (wait > 0) await sleep(wait * 1000);
      const t = now(); beats.push({ id, t: +t.toFixed(3) }); beatEnd = t + lines[id] + GAP;
    },
    // Wait until this much of the current line has been said.
    async at(frac) { const b = beats[beats.length - 1]; const target = b.t + lines[b.id] * frac; const w = target - now(); if (w > 0) await sleep(w * 1000); },
    async end(tail = 1.6) {
      const wait = beatEnd - now(); if (wait > 0) await sleep(wait * 1000);
      await sleep(tail * 1000);
      // Keep frames flowing to the very end.
      await setCur(); await sleep(120);
      recording = false; await cdp.send('Page.stopScreencast');
    },
  };

  try { await scene(rec); }
  catch (e) {
    const { data } = await cdp.send('Page.captureScreenshot', { format: 'png' }).catch(() => ({}));
    if (data) fs.writeFileSync(path.join(outDir, 'error.png'), Buffer.from(data, 'base64'));
    throw e;
  }
  const duration = now();
  // Constant 30 fps video from the frames' real times.
  const list = frames.map((f, i) => `file '${f.f.replace(/\\/g, '/')}'\nduration ${Math.max(0.001, ((frames[i + 1]?.t ?? duration) - f.t)).toFixed(4)}`).join('\n') + `\nfile '${frames.at(-1).f.replace(/\\/g, '/')}'\n`;
  fs.writeFileSync(path.join(outDir, 'frames.txt'), list);
  const lead = frames[0].t; // time before the first frame: hold it
  execSync(`ffmpeg -v error -y -f concat -safe 0 -i frames.txt -vf "tpad=start_duration=${lead.toFixed(3)}:start_mode=clone,fps=30,scale=1920:1080:flags=lanczos,format=yuv420p" -c:v libx264 -crf 16 -preset slow video.mp4`, { cwd: outDir, stdio: 'inherit' });
  fs.writeFileSync(path.join(outDir, 'timeline.json'), JSON.stringify({ scene: sceneId, duration: +duration.toFixed(3), beats, callouts }, null, 1));
  fs.rmSync(path.join(outDir, 'frames'), { recursive: true, force: true });
  console.log(`${sceneId}: ${duration.toFixed(1)} s, ${frames.length} frames, ${beats.length} beats, ${callouts.length} callouts`);
}

try { await main(); } finally { browser.kill(); setTimeout(() => process.exit(process.exitCode || 0), 300); }
