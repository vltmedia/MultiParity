import { paint } from './lib.mjs';
import { deskReady, EMBER } from './desk.mjs';

export default async function scene(r) {
  await deskReady(r);
  await r.park(1000, 480, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: '.changes h2' }, { ms: 900 });

  await r.beat('edit');
  await r.park(760, 520, 900);
  await r.at(0.45);
  paint(r, `${EMBER}/Characters/Hero/hero_diffuse.png`, 'skin2', 41);

  await r.beat('shows');
  await r.callout('sweep', { css: '.changes .files li', index: 0 }, { dur: 2.4, pad: 2 });
  await r.hover({ css: '.changes .files li .name strong' }, { after: 500 });
  await r.callout('tag', { css: '.changes .files li .tag', index: 0 }, { label: 'Edited', dur: 2.2, delay: 0.8 });

  await r.beat('claimed');
  await r.callout('tag', { css: '.changes .files li .meta em', index: 0 }, { label: 'Claimed for you', dur: 3.4 });
  await r.hover({ css: '.changes .files li .meta em' }, { after: 400 });

  await r.beat('message');
  await r.hover({ css: '.changes .files li input[type=checkbox]' }, { after: 500 });
  await r.callout('spot', { css: '#w-msg' }, { dur: 4.4 });
  await r.click({ css: '#w-msg' }, { after: 300 });
  await r.at(0.55);
  await r.type('Warmer skin tones on the hero', { cps: 15 });

  await r.beat('publish');
  await r.callout('ring', { css: '[data-w=publish]' }, { dur: 1.8 });
  await r.click({ css: '[data-w=publish]' }, { after: 1800 });
  await r.park(1050, 380, 900);
  await r.end(1.6);
}
scene.options = { view: 1200 };
