import fs from 'node:fs';
import { DESK, daemon } from './lib.mjs';

const LOCAL = 'http://127.0.0.1:18529/local';

/** Audio and Code not on this computer yet, so the production has something to sync. */
async function unsync(r) {
  daemon(r);
  for (let i = 0; i < 40; i++) { try { await fetch(`${LOCAL}/health`); break; } catch { await new Promise((ok) => setTimeout(ok, 250)); } }
  const list = await (await fetch(`${LOCAL}/workspaces`)).json();
  for (const w of list.filter((w) => ['Audio', 'Code'].includes(w.project_name))) {
    await fetch(`${LOCAL}/workspaces/${w.id}/disconnect`, { method: 'POST' });
    fs.rmSync(String(w.root).replace(/^\\\\\?\\/, ''), { recursive: true, force: true, maxRetries: 5, retryDelay: 300 });
  }
}

export default async function scene(r) {
  await unsync(r);
  // Folders left from before (with their old sync data) would be refused as already synced.
  for (const n of ['Audio', 'Code']) fs.rmSync(`D:/Work/${n}`, { recursive: true, force: true, maxRetries: 5, retryDelay: 300 });
  await r.open(DESK, 2400);
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.callout('sweep', { css: '.group .band', index: 0 }, { dur: 2.4, pad: 4 });
  await r.hover({ css: '.group .band .b-prod', index: 0 }, { ms: 1000 });

  await r.beat('button');
  await r.callout('ring', { css: '[data-pact=sync-prod]' }, { dur: 2 });
  await r.click({ css: '[data-pact=sync-prod]' }, { after: 1200 });

  await r.beat('pick');
  await r.callout('spot', { css: '#pfolder' }, { dur: 2.4, pad: 6 });
  await r.hover({ css: '#pfolder' }, { after: 600 });
  await r.at(0.45);
  await r.callout('sweep', { css: '#dlg .prod-pick' }, { dur: 2.6, pad: 4 });
  await r.hover({ text: 'Audio', sel: '#dlg strong' }, { after: 500 });
  await r.click({ css: '#dlg button[value=go]' }, { after: 600 });

  await r.beat('done');
  await r.callout('tag', { text: 'Already at', sel: '#dlg small' }, { label: 'Kept where it is', dur: 2.6 });
  await r.hover({ text: 'Already at', sel: '#dlg small' }, { after: 2200 });
  await r.click({ css: '#dlg button[value=done]' }, { after: 1200 });
  await r.callout('tag', { css: '.group .band .b-all', index: 0 }, { label: 'All on this computer', dur: 2.4 });
  await r.end(1.8);
}
scene.options = { view: 1200 };
