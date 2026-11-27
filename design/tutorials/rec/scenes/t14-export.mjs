import fs from 'node:fs';
import { DESK, WEB, as, daemon, webSignIn } from './lib.mjs';
import { ensureSynced } from './desk.mjs';
import { scopeCinder } from './web.mjs';

const FROST = 'D:/Work/Frostline';
const INTO = 'D:/Exports/Deliveries';

export default async function scene(r) {
  daemon(r);
  await ensureSynced('Frostline', FROST);
  as(r, 'you', 'get-latest', FROST);
  fs.rmSync(INTO, { recursive: true, force: true });
  await webSignIn(r); // for the Admin half, signed in before the camera rolls
  await scopeCinder(r);
  await r.open(DESK, 1800);
  await r.click({ text: 'Frostline', sel: '#here button' }, { ms: 10, after: 1500 });
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: 'h1' }, { ms: 1000 });

  await r.beat('desk');
  await r.callout('ring', { css: '[data-w=export]' }, { dur: 1.8 });
  await r.click({ css: '[data-w=export]' }, { after: 900 });
  await r.click({ css: '#ex-into' }, { after: 200 });
  await r.type('D:\\Exports\\Deliveries', { cps: 16 });
  await r.callout('spot', { css: '#dlg .choices' }, { dur: 3, pad: 4 });
  await r.hover({ text: 'One .zip file', sel: 'strong' }, { after: 500 });

  await r.beat('how');
  await r.click({ css: '#dlg button[value=go]' }, { after: 2600 });
  await r.callout('tag', { css: '.toast, #toast', index: 0 }, { label: 'Done', dur: 2.6 });
  await r.park(1050, 420, 900);

  await r.beat('admin');
  await r.open(`${WEB}/admin/projects`, 2200);
  const row = await r.eval(`[...document.querySelectorAll('tbody tr')].findIndex((tr) => tr.textContent.includes('Frostline') && tr.querySelector('button'))`);
  const exportIndex = await r.eval(`[...document.querySelectorAll('tbody tr')].slice(0, ${row}).reduce((n, tr) => n + [...tr.querySelectorAll('button')].filter((b) => b.textContent.trim().startsWith('Export')).length, 0)`);
  await r.callout('ring', { text: 'Export…', sel: 'button', index: exportIndex }, { dur: 1.8 });
  await r.click({ text: 'Export…', sel: 'button', index: exportIndex }, { after: 1400 });
  await r.at(0.55);
  await r.choose({ css: '[role=dialog] select, dialog select', index: 1 }, 'delivery');
  await r.callout('tag', { css: '[role=dialog] select, dialog select', index: 1 }, { label: 'An S3 bucket', dur: 2.4 });

  await r.beat('archive');
  await r.callout('ring', { text: 'Archive the project when the export has finished', sel: 'label' }, { dur: 2.2 });
  await r.hover({ text: 'Archive the project when the export has finished', sel: 'label' }, { after: 900 });
  await r.click({ text: 'Close', sel: 'button' }, { after: 800 });
  await r.end(1.6);
}
scene.options = { view: 1440 };
