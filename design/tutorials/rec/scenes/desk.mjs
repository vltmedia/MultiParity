// Helpers for desktop-app scenes: Alice's Emberfall folder in D:\Work, opened in the app.
import fs from 'node:fs';
import { DESK, DEMO, as, daemon } from './lib.mjs';

export const EMBER = 'D:/Work/Emberfall';

/** Alice's folder up to date, no unpublished work, no checkpoints; the app open on Your work. */
export async function deskReady(r, { tab } = {}) {
  daemon(r);
  // Throw away anything left from an earlier take (keeps the scenes repeatable), on Main.
  if (!/Nothing to publish/.test(as(r, 'you', 'status', EMBER))) as(r, 'you', 'discard .', EMBER);
  if (!/ on Main,/.test(as(r, 'you', 'status', EMBER))) as(r, 'you', 'branch switch Main', EMBER);
  as(r, 'you', 'get-latest', EMBER);
  if (!/Nothing to publish/.test(as(r, 'you', 'status', EMBER))) as(r, 'you', 'discard .', EMBER);
  releaseMine(r, EMBER);
  await r.open(DESK, 1800);
  await r.click({ text: 'Emberfall', sel: '#here button' }, { ms: 10, after: 1200 });
  if (tab) await r.click({ css: `[data-tab=${tab}]` }, { ms: 10, after: 900 });
}

export const openTab = (r, tab) => r.click({ css: `[data-tab=${tab}]` }, { after: 900 });
export { DEMO, fs };

const LOCAL = 'http://127.0.0.1:18529/local';

/** Make sure a project is synced to a folder on this computer (off camera). */
export async function ensureSynced(name, folder) {
  for (let i = 0; i < 60; i++) { try { await fetch(`${LOCAL}/health`); break; } catch { await new Promise((ok) => setTimeout(ok, 250)); } }
  const all = await (await fetch(`${LOCAL}/projects`)).json();
  for (const g of all.accounts || []) for (const p of g.projects) {
    if (p.project.name !== name) continue;
    if (p.workspaces.length) return;
    const res = await fetch(`${LOCAL}/connect`, { method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ account_id: g.account.id, project_id: p.project.id, folder, dry_run: false }) });
    if (!res.ok) throw new Error(`Could not sync ${name}: ${await res.text()}`);
    return;
  }
  throw new Error(`No project ${name} for the demo account.`);
}

/** Give back every claim Alice holds in a folder's project (earlier takes leave some behind). */
export function releaseMine(r, folder) {
  const mine = JSON.parse(as(r, 'you', 'claims --json', folder)).filter((c) => c.user && /alice@/.test(c.user.email || '')).map((c) => c.path);
  if (mine.length) as(r, 'you', `release ${mine.map((p) => JSON.stringify(p)).join(' ')}`, folder);
}
