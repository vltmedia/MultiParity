import { webReady } from './web.mjs';

export default async function scene(r) {
  await webReady(r, '/admin');
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.callout('spot', { css: 'nav, aside', index: 0 }, { dur: 3, pad: 0 });
  await r.hover({ text: 'Overview', sel: 'a' }, { ms: 1000 });

  await r.beat('invite');
  await r.click({ text: 'Invites', sel: 'a' }, { after: 1200 });
  await r.click({ css: 'textarea' }, { after: 300 });
  await r.type('lena@cinderworks.studio', { cps: 18 });
  await r.callout('ring', { css: '.inv .panel select', index: 0 }, { dur: 2 });
  await r.choose({ css: '.inv .panel select', index: 0 }, 'Member');
  await r.callout('ring', { text: 'Make invites', sel: 'button' }, { dur: 1.8 });
  await r.click({ text: 'Make invites', sel: 'button' }, { after: 1500 });

  await r.beat('projects');
  await r.click({ text: 'Projects', sel: 'a' }, { after: 1200 });
  await r.callout('tag', { text: 'New project', sel: 'button' }, { label: 'Admins only', dur: 3 });
  await r.click({ text: 'New project', sel: 'button' }, { after: 1000 });
  await r.callout('spot', { text: 'Store file data in', sel: 'label span', up: 'label' }, { dur: 3.6, pad: 8 });
  await r.hover({ text: 'Store file data in', sel: 'label span' }, { after: 500 });
  await r.at(0.75);
  await r.click({ text: 'Cancel', sel: 'button' }, { after: 600 });
  await r.callout('ring', { text: 'Change storage…', sel: 'button' }, { dur: 2.2 });
  await r.hover({ text: 'Change storage…', sel: 'button' }, { after: 400 });

  await r.beat('look');
  await r.callout('ring', { text: 'Look…', sel: 'button' }, { dur: 2 });
  await r.click({ text: 'Look…', sel: 'button' }, { after: 1400 });
  await r.callout('spot', { css: 'dialog, [role=dialog]' }, { dur: 3.4, pad: 0 });
  await r.park(900, 450, 900);
  await r.at(0.85);
  await r.press('Escape');

  await r.beat('bye');
  await r.click({ text: 'All projects', sel: 'a' }, { after: 1200 });
  await r.park(900, 500, 900);
  await r.end(2);
}
scene.options = { view: 1440 };
