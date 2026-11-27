import { DESK, as, daemon } from './lib.mjs';

const off = (r) => { try { as(r, 'you', 'plugin remove example-checks'); } catch {} };

export default async function scene(r) {
  off(r); // installed on camera, from the official plugins on GitHub
  daemon(r);
  await r.open(DESK, 2200);
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.callout('ring', { css: '.nav [data-view=plugins]' }, { dur: 1.8 });
  await r.click({ css: '.nav [data-view=plugins]' }, { after: 1200 });

  await r.beat('browse');
  await r.click({ css: '[data-pl=tab][data-t=browse]' }, { after: 600 });
  await r.hover({ css: 'li.cat' }, { after: 400 }); // waits for the sources to load
  await r.callout('spot', { css: '#v-plugins .panel', index: 0 }, { dur: 3, pad: 0 });

  await r.beat('version');
  await r.callout('ring', { css: '[data-pl=pick]' }, { dur: 1.6 });
  await r.click({ css: '[data-pl=pick]' }, { after: 1400 });
  await r.callout('spot', { css: '#dlg .choices' }, { dur: 2.6, pad: 4 });
  await r.hover({ css: '#dlg .choices label' }, { after: 600 });
  await r.click({ css: '#dlg button[value=go]' }, { after: 2600 });

  await r.beat('check');
  await r.callout('spot', { css: '#pl-preview .pl-card' }, { dur: 3.6, pad: 4 });
  await r.hover({ css: '#pl-preview .pl-what' }, { after: 900 });
  await r.at(0.7);
  await r.click({ css: '#pl-preview [data-pl=install]' }, { after: 1500 });

  await r.beat('use');
  await r.click({ text: 'Emberfall', sel: '#here button' }, { after: 1400 });
  await r.click({ css: '[data-tab=files]' }, { after: 1200 });
  await r.click({ css: '#p-q' }, { after: 200 });
  await r.type('hero_diffuse', { cps: 14 });
  await r.press('Enter');
  await r.click({ css: '.flist li[data-path]' }, { right: true, after: 900 });
  await r.callout('ring', { text: 'Image size (Example checks)', sel: '#ctx li' }, { dur: 1.6 });
  await r.click({ text: 'Image size (Example checks)', sel: '#ctx li' }, { after: 2200 });
  await r.at(0.75);
  await r.click({ css: '[data-tab="plug:example-checks:overview"]' }, { after: 2600 });
  await r.end(1.8);
  off(r);
}
scene.options = { view: 1200 };
