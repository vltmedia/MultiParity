import fs from 'node:fs';
import { paint } from './lib.mjs';
import { deskReady, EMBER } from './desk.mjs';

export default async function scene(r) {
  await deskReady(r);
  // Work in progress on the hero: two edited files.
  paint(r, `${EMBER}/Characters/Hero/hero_diffuse.png`, 'skin2', 52);
  fs.appendFileSync(`${EMBER}/Characters/Hero/hero_body.fbx`, Buffer.alloc(200000, 7));
  await r.sleep(2500);
  await r.park(1000, 480, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: '.changes .files li .name strong' }, { ms: 900 });

  await r.beat('save');
  await r.callout('ring', { css: '[data-w=checkpoint]' }, { dur: 2 });
  await r.click({ css: '[data-w=checkpoint]' }, { after: 700 });
  await r.at(0.55);
  await r.click({ css: '#cp-note' }, { after: 200 });
  await r.type('before retopo', { cps: 13 });
  await r.click({ css: '#dlg-body button[value=save]' }, { after: 1200 });

  await r.beat('private');
  await r.callout('sweep', { css: '.cps .cplist li', index: 0 }, { dur: 2.6, pad: 2 });
  await r.callout('tag', { css: '.cps .cplist li', index: 0 }, { label: 'Private to you', dur: 2.8, delay: 0.6 });
  await r.hover({ css: '.cps .cplist li strong' }, { after: 400 });

  await r.beat('oops');
  // The retopo goes badly.
  fs.writeFileSync(`${EMBER}/Characters/Hero/hero_body.fbx`, Buffer.alloc(900000, 3));
  paint(r, `${EMBER}/Characters/Hero/hero_diffuse.png`, 'rust', 53);
  await r.park(900, 400, 900);

  await r.beat('rewind');
  await r.callout('ring', { css: '[data-w=rewind]' }, { dur: 2.2 });
  await r.click({ css: '[data-w=rewind]' }, { after: 900 });
  await r.at(0.4);
  await r.click({ css: '#dlg-body button[value=go]' }, { after: 1300 });
  await r.callout('sweep', { css: '.cps .cplist' }, { dur: 2.4, pad: 2 });
  await r.park(1000, 420, 900);
  await r.end(1.6);
}
scene.options = { view: 1200 };
