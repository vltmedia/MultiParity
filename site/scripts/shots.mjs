// Take the website's screenshots with headless Edge or Chrome (Chrome DevTools Protocol, no
// extra packages): node scripts/shots.mjs shots.json <out folder>
//
// shots.json is a list of { name, url, scheme: "light"|"dark", width, height, steps: [js...],
// wait } where each step is JavaScript run in the page (awaited) before the picture is taken.
// Pictures are saved as <out>/<name>-<scheme>.webp at twice the size (crisp on retina screens).
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const [, , listFile, outDir] = process.argv;
const shots = JSON.parse(fs.readFileSync(listFile, 'utf8'));
fs.mkdirSync(outDir, { recursive: true });

const BROWSERS = [
  process.env.BROWSER,
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
].filter(Boolean);
const exe = BROWSERS.find((b) => fs.existsSync(b));
if (!exe) throw new Error('No Edge or Chrome found; set BROWSER to one.');

const PORT = 9333;
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'mp-shots-'));
const browser = spawn(exe, ['--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`, '--hide-scrollbars', '--no-first-run', 'about:blank'], { stdio: 'ignore' });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function json(p, method = 'GET') {
  for (let i = 0; i < 50; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${PORT}${p}`, { method });
      return await r.json();
    } catch {
      await sleep(200);
    }
  }
  throw new Error('The browser did not start.');
}

function connect(wsUrl) {
  const ws = new WebSocket(wsUrl);
  let id = 0;
  const waiting = new Map();
  const listeners = [];
  ws.onmessage = (m) => {
    const msg = JSON.parse(m.data);
    if (msg.id && waiting.has(msg.id)) {
      const { ok, fail } = waiting.get(msg.id);
      waiting.delete(msg.id);
      msg.error ? fail(new Error(msg.error.message)) : ok(msg.result);
    } else listeners.forEach((l) => l(msg));
  };
  const send = (method, params = {}) => new Promise((ok, fail) => { const i = ++id; waiting.set(i, { ok, fail }); ws.send(JSON.stringify({ id: i, method, params })); });
  const once = (method) => new Promise((ok) => { const l = (m) => { if (m.method === method) { listeners.splice(listeners.indexOf(l), 1); ok(m.params); } }; listeners.push(l); });
  return new Promise((ok) => { ws.onopen = () => ok({ send, once, close: () => ws.close() }); });
}

try {
  await json('/json/version');
  const target = await json('/json/new?about:blank', 'PUT');
  const cdp = await connect(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  for (const s of shots) {
    const w = s.width || 1440, h = s.height || 900;
    await cdp.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 2, mobile: false });
    await cdp.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: s.scheme || 'light' }, { name: 'prefers-reduced-motion', value: 'reduce' }] });
    const loaded = cdp.once('Page.loadEventFired');
    await cdp.send('Page.navigate', { url: s.url });
    await loaded;
    await sleep(s.wait ?? 1500);
    for (const step of s.steps || []) {
      const r = await cdp.send('Runtime.evaluate', { expression: `(async () => { ${step} })()`, awaitPromise: true, returnByValue: true });
      if (r.exceptionDetails) console.warn(`${s.name}: step failed: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
      await sleep(s.stepWait ?? 700);
    }
    const { data } = await cdp.send('Page.captureScreenshot', { format: 'webp', quality: 88, captureBeyondViewport: false });
    const file = path.join(outDir, `${s.name}-${s.scheme || 'light'}.webp`);
    fs.writeFileSync(file, Buffer.from(data, 'base64'));
    console.log(`${path.basename(file)}  ${Math.round(fs.statSync(file).size / 1024)} KB`);
  }
  cdp.close();
} finally {
  browser.kill();
}
