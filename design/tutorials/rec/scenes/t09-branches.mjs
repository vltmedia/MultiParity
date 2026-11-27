import { as, paint } from './lib.mjs';
import { deskReady, EMBER } from './desk.mjs';

export default async function scene(r) {
  await deskReady(r, { tab: 'branches' });
  await r.park(1000, 480, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: '.blist li strong' }, { ms: 1000 });

  await r.beat('new');
  await r.callout('ring', { css: '[data-p=new-branch]' }, { dur: 2 });
  await r.click({ css: '[data-p=new-branch]' }, { after: 700 });
  await r.click({ css: '#nb-name' }, { after: 200 });
  await r.type('lighting-pass', { cps: 13 });
  await r.callout('tag', { css: '#nb-switch' }, { label: 'Switch right away', dur: 2.2 });
  await r.click({ css: '#dlg-body button[value=go]' }, { after: 1800 });

  await r.beat('switch');
  await r.callout('spot', { css: '.blist li.here' }, { label: 'Its own space', dur: 3 });
  await r.hover({ css: '.blist li.here strong' }, { after: 300 });
  await r.callout('tag', { text: 'Main', sel: '.blist li strong' }, { label: 'Unchanged for everyone', dur: 2.6, delay: 1.4 });
  // Some lighting work, published to the branch (off camera, it is the same publish as before).
  paint(r, `${EMBER}/Environments/Forge/forge_floor_stone.png`, 'ember', 91);
  as(r, 'you', 'publish -m "Forge lighting: warmer key, ember bounce"', EMBER);
  await r.at(0.9);
  await r.click({ css: '[data-tab=history]' }, { after: 600 });
  await r.click({ css: '[data-tab=branches]' }, { after: 900 });

  await r.beat('merge');
  await r.callout('ring', { css: '[data-p=merge]' }, { dur: 2 });
  await r.click({ css: '[data-p=merge]' }, { after: 900 });
  await r.click({ css: '#dlg-body button[value=go]' }, { after: 1800 });
  await r.park(1000, 420, 900);
  await r.end(1.6);
}
scene.options = { view: 1200 };
