import { as } from './lib.mjs';
import { EMBER } from './desk.mjs';
import { webReady } from './web.mjs';

export default async function scene(r) {
  // Alice has the hero body claimed; Mara has the cape sim.
  try { as(r, 'you', 'claim Characters/Hero/hero_body.fbx', EMBER); } catch {}
  await webReady(r, 'files');
  await r.click({ text: 'Characters', sel: '.foldbtn' }, { ms: 10, after: 1200 });
  await r.click({ text: 'Hero', sel: '.foldbtn' }, { ms: 10, after: 1500 });
  await r.park(900, 520, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: 'table.files thead' }, { ms: 1000 });

  await r.beat('shows');
  const row = { text: 'hero_cape_sim.abc', sel: 'td.name', up: 'tr' };
  await r.callout('spot', row, { dur: 4.2, pad: 2 });
  await r.hover(row, { after: 300 });
  await r.callout('tag', { text: 'Claimed by Mara', sel: 'span.st' }, { label: 'Mara has it', dur: 3, delay: 1.2 });

  await r.beat('working');
  await r.callout('spot', { text: 'Working right now', sel: 'h3', up: 'div' }, { dur: 3.6, pad: 6 });
  await r.hover({ text: 'Working right now', sel: 'h3' }, { after: 400 });
  await r.callout('tag', { text: 'Working right now', sel: 'h3', up: 'div' }, { label: 'Live, across the project', dur: 3, delay: 0.8 });

  await r.beat('ask');
  await r.click({ css: 'input[aria-label="Select hero_cape_sim.abc"]' }, { after: 700 });
  await r.callout('ring', { text: 'Ask Mara to release', sel: 'button' }, { dur: 2.2 });
  await r.at(0.32);
  await r.click({ text: 'Ask Mara to release', sel: 'button' }, { after: 1200 });

  await r.beat('yours');
  await r.click({ css: 'input[aria-label="Select hero_cape_sim.abc"]' }, { after: 500 });
  const mine = { text: 'hero_body.fbx', sel: 'td.name', up: 'tr' };
  await r.callout('tag', mine, { label: 'Your claim: "You"', dur: 3.4 });
  await r.hover(mine, { after: 400 });
  await r.end(1.6);
}
scene.options = { view: 1440 };
