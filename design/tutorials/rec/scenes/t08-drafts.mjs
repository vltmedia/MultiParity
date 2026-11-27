import fs from 'node:fs';
import path from 'node:path';
import { DEMO, DESK, SECRETS, as, daemon, paint } from './lib.mjs';
import { deskReady, EMBER } from './desk.mjs';

export default async function scene(r) {
  await deskReady(r);
  paint(r, `${EMBER}/Characters/EmberWolf/ember_wolf_fur.png`, 'fur2', 61);
  fs.appendFileSync(`${EMBER}/Characters/EmberWolf/ember_wolf.blend`, Buffer.alloc(150000, 9));
  // "Your other computer": Alice's laptop, its own config and folder, the same account.
  for (const d of [path.join(DEMO, 'cfg-laptop'), 'D:/Work/Laptop']) fs.rmSync(d, { recursive: true, force: true, maxRetries: 5, retryDelay: 300 });
  try { as(r, 'you', 'draft delete --all', EMBER); } catch {}
  as(r, 'laptop', `login http://127.0.0.1:18570 --email alice@cinderworks.studio --password "${SECRETS.ADMIN_PW}"`);
  as(r, 'laptop', `connect Emberfall "D:/Work/Laptop/Emberfall"`);
  await r.sleep(2000);
  await r.click({ css: '[data-tab=drafts]' }, { ms: 10, after: 900 });
  await r.park(1000, 480, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: '[data-tab=work]' }, { ms: 1000 });

  await r.beat('save');
  await r.callout('ring', { css: '[data-p=save-draft]' }, { dur: 2 });
  await r.click({ css: '[data-p=save-draft]' }, { after: 700 });
  await r.click({ css: '#dr-title' }, { after: 200 });
  await r.type('Wolf fur, half done', { cps: 14 });
  await r.click({ css: '#dlg-body button[value=go]' }, { after: 1500 });
  await r.callout('tag', { css: '.cplist li', index: 0 }, { label: 'Only you can see it', dur: 2.6 });

  await r.beat('pickup');
  // Cut to the laptop.
  daemon(r, 'cfg-laptop');
  await r.open(DESK, 1500);
  await r.click({ text: 'Emberfall', sel: '#here button' }, { ms: 10, after: 1100 });
  await r.callout('tag', { css: '.main .head h1' }, { label: 'Your other computer', dur: 2.6 });
  await r.callout('ring', { css: '[data-tab=drafts]' }, { dur: 1.8, delay: 0.6 });
  await r.click({ css: '[data-tab=drafts]' }, { after: 900 });
  await r.callout('ring', { css: '[data-p=pickup]' }, { dur: 1.8 });
  await r.click({ css: '[data-p=pickup]' }, { after: 800 });
  await r.click({ css: '#dlg-body button[value=go]' }, { after: 1500 });

  await r.beat('pocket');
  await r.callout('sweep', { css: '.changes .files' }, { dur: 2.4, pad: 2 });
  await r.park(1000, 420, 900);
  await r.end(1.6);
}
scene.options = { view: 1200 };
