import { as, folderOf, paint } from './lib.mjs';
import { deskReady } from './desk.mjs';

export default async function scene(r) {
  await deskReady(r);
  const before = JSON.parse(await r.eval(`fetch('http://127.0.0.1:18529/local/notifications').then((x) => x.text())`)).last || 0;
  await r.park(1000, 480, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: '.main .head h1' }, { ms: 900 });

  await r.beat('toast');
  // Mara really publishes; the app's own notification text is shown the way Windows shows it.
  const m = folderOf('mara');
  as(r, 'mara', 'get-latest', m);
  paint(r, `${m}/Characters/EmberWolf/ember_wolf_fur.png`, 'fur', 100 + (Date.now() % 900));
  as(r, 'mara', 'publish -m "Ember wolf fur, softer underbelly"', m);
  let n = null;
  for (let i = 0; i < 40 && !n; i++) {
    const j = JSON.parse(await r.eval(`fetch('http://127.0.0.1:18529/local/notifications?after=${before}').then((x) => x.text())`));
    n = (j.notifications || []).at(-1); if (!n) await r.sleep(500);
  }
  await r.toast(n ? n.title : 'Mara published to Main', n ? n.body : 'Ember wolf fur, softer underbelly', 5200);
  await r.callout('ring', { rect: { x: r.W - 18 - 340, y: r.H - 18 - 96, w: 340, h: 96 } }, { dur: 2.4, delay: 0.5 });

  await r.beat('click');
  await r.click({ text: 'Emberfall', sel: '#here button' }, { after: 1200 });
  await r.callout('ring', { css: '[data-w=latest]' }, { dur: 2.2 });
  await r.click({ css: '[data-w=latest]' }, { after: 1500 });

  await r.beat('parts');
  await r.callout('tag', { css: '#toast' }, { label: 'Only the changed parts download', dur: 3.6 }).catch(() => {});
  await r.park(900, 520, 1000);

  await r.beat('safe');
  await r.hover({ css: '.changes h2' }, { after: 400 });
  await r.end(1.6);
}
scene.options = { view: 1200 };
