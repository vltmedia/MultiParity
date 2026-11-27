import { deskReady } from './desk.mjs';

export default async function scene(r) {
  await deskReady(r, { tab: 'files' });
  await r.park(900, 520, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: '.flist' }, { ms: 1100 });

  await r.beat('open');
  await r.callout('ring', { css: '[data-p=view][data-v=map]' }, { dur: 1.8 });
  await r.click({ css: '[data-p=view][data-v=map]' }, { after: 1500 });

  await r.beat('read');
  await r.callout('spot', { css: '.tmap' }, { dur: 3, pad: 0 });
  await r.hover({ css: '.tm-file', index: 0 }, { after: 900 });
  await r.at(0.5);
  await r.callout('tag', { css: '.tm-dir', index: 0 }, { label: 'A folder', dur: 2.4 });
  await r.hover({ css: '.tm-dir > span', index: 0 }, { after: 700 });

  await r.beat('zoom');
  await r.click({ css: '.tm-dir > span', index: 0 }, { after: 1600 });
  await r.at(0.5);
  await r.click({ css: '.tm-file[data-path]', index: 0 }, { right: true, after: 1600 });
  await r.press('Escape');

  await r.beat('web');
  await r.click({ css: '[data-p=view][data-v=list]' }, { after: 1200 });
  await r.end(1.6);
}
scene.options = { view: 1200 };
