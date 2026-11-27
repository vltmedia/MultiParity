// Helpers for desktop-app scenes: Alice's Emberfall folder in D:\Work, opened in the app.
import fs from 'node:fs';
import { DESK, DEMO, as, daemon } from './lib.mjs';

export const EMBER = 'D:/Work/Emberfall';

/** Alice's folder up to date, no unpublished work, no checkpoints; the app open on Your work. */
export async function deskReady(r, { tab } = {}) {
  daemon(r);
  as(r, 'you', 'get-latest', EMBER);
  // Throw away anything left from an earlier take (keeps the scenes repeatable).
  if (!/Nothing to publish/.test(as(r, 'you', 'status', EMBER))) as(r, 'you', 'discard .', EMBER);
  await r.open(DESK, 1800);
  await r.click({ css: '#here button' }, { ms: 10, after: 1200 });
  if (tab) await r.click({ css: `[data-tab=${tab}]` }, { ms: 10, after: 900 });
}

export const openTab = (r, tab) => r.click({ css: `[data-tab=${tab}]` }, { after: 900 });
export { DEMO, fs };
