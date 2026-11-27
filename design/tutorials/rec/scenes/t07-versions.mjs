import { deskReady, openTab } from './desk.mjs';

export default async function scene(r) {
  await deskReady(r, { tab: 'files' });
  await r.click({ text: 'Environments', sel: '.folder strong' }, { ms: 10, after: 900 });
  await r.click({ text: 'Forge', sel: '.folder strong' }, { ms: 10, after: 900 });
  await r.park(1000, 480, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ text: 'forge_door_rust.png', sel: 'li strong' }, { ms: 1000 });

  await r.beat('open');
  await r.click({ text: 'forge_door_rust.png', sel: 'li strong' }, { right: true, after: 700 });
  await r.callout('ring', { text: 'Older versions…', sel: '#ctx li, #ctx button' }, { dur: 1.6 });
  await r.click({ text: 'Older versions…', sel: '#ctx li, #ctx button' }, { after: 1000 });
  await r.callout('spot', { css: '#dlg' }, { dur: 4, pad: 0 });
  await r.hover({ css: '.versions li .what', index: 1 }, { after: 400 });

  await r.beat('choices');
  await r.callout('tag', { text: 'Save a copy', sel: '.versions button', index: 1 }, { label: 'A copy beside it', dur: 2.6 });
  await r.hover({ text: 'Save a copy', sel: '.versions button', index: 1 }, { after: 300 });
  await r.at(0.5);
  await r.callout('tag', { text: 'Put back', sel: '.versions button' }, { label: 'Into your folder', dur: 2.6 });
  await r.hover({ text: 'Put back', sel: '.versions button' }, { after: 300 });

  await r.beat('latest');
  await r.callout('tag', { text: 'Make latest', sel: '.versions button' }, { label: 'Everyone gets it', dur: 2.6 });
  await r.hover({ text: 'Make latest', sel: '.versions button' }, { after: 300 });

  await r.beat('safe');
  await r.callout('spot', { css: '#dlg .note' }, { label: 'Nothing is lost', dur: 3 });
  await r.hover({ css: '#dlg .note' }, { after: 300 });
  await r.at(0.6);
  await r.click({ css: '#dlg-body .dlg-acts button[value=cancel]' }, { after: 600 });
  await r.end(1.4);
}
export { openTab };
scene.options = { view: 1200 };
