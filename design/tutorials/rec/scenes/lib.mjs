// Shared bits for the tutorial scenes. The demo data lives outside the repo (MP_DEMO).
import fs from 'node:fs';
import path from 'node:path';

export const DEMO = process.env.MP_DEMO || 'P:/temp/Claude/claude/7df7f3888d1632d5/a22ff03a-1f55-4c9c-8b8f-318966a068cc/scratchpad/demo';
export const SECRETS = Object.fromEntries(fs.readFileSync(path.join(DEMO, 'secrets.env'), 'utf8').split('\n').filter((l) => l.includes('=')).map((l) => l.trim().split(/=(.*)/s).slice(0, 2)));
export const DESK = 'http://127.0.0.1:4176/?daemon=18529';
export const WEB = 'http://127.0.0.1:18570';
export const YOU = 'cfg-you';
export const MP = 'P:/Projects/UAI/MultiParity/Code/multiparity/target/debug/mp.exe';
const DAEMON = 'P:/Projects/UAI/MultiParity/Code/multiparity/design/tutorials/rec/daemon.ps1';

/** Restart the desktop daemon on a config folder (fresh: wipe it first). */
export function stopDaemon(r) { r.sh(`pwsh -NoProfile -File "${DAEMON}"`); }
export function daemon(r, cfg = YOU, { fresh = false } = {}) {
  const dir = path.join(DEMO, cfg);
  if (fresh) { stopDaemon(r); fs.rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 300 }); }
  fs.mkdirSync(dir, { recursive: true });
  r.shDetached(`pwsh -NoProfile -File "${DAEMON}" -Config "${dir}"`);
}

/** Run the mp command line as one of the demo people (mara, dev, teo, sana, alice). */
export function as(r, who, args, cwd) {
  const cfg = path.join(DEMO, who === 'you' ? YOU : `cfg-${who}`).replace(/\\/g, '/');
  return r.sh(`cd "${cwd || DEMO}" && MP_CONFIG_DIR="${cfg}" MP_DEVICE=${who}-workstation "${MP}" ${args}`);
}
export const folderOf = (who, project = 'Emberfall') => path.join(DEMO, 'work', who, project).replace(/\\/g, '/');

/** Sign in to the web app before recording (not on camera). */
export async function webSignIn(r, email = 'alice@cinderworks.studio', password = SECRETS.ADMIN_PW) {
  await r.open(`${WEB}/sign-in`);
  await r.eval(`(async () => { const res = await fetch('/api/auth/login', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(${JSON.stringify({ email, password })}) }); const j = await res.json(); localStorage.setItem('mp.token', j.token); return res.status; })()`);
}

/** A texture-like image written with Python, for "editing" a file on camera. */
export function paint(r, file, kind, seed) {
  // A different picture every take: the same one as the last take would not count as an edit.
  const take = seed * 1000 + (Date.now() % 997);
  r.sh(`python "${path.join(DEMO, 'paint.py')}" "${file}" ${kind} ${take}`);
}
