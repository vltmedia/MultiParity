import { as, folderOf } from './lib.mjs';
import { webReady } from './web.mjs';

export default async function scene(r) {
  // Tags the studio uses, on a few files (idempotent enough: errors are ignored).
  const d = folderOf('dev');
  for (const [t, c] of [['Approved', 'green'], ['Needs review', 'orange'], ['Hero asset', 'purple']]) try { as(r, 'dev', `tag new "${t}" --color ${c}`, d); } catch {}
  for (const [t, p] of [['Approved', 'Environments/Forge/forge_door_rust.png'], ['Needs review', 'Environments/Forge/forge_floor_stone.png'], ['Approved', 'Environments/Forge/forge_beams_wood.png']]) try { as(r, 'dev', `tag add "${t}" ${p}`, d); } catch {}
  await webReady(r, 'today');
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: 'h1' }, { ms: 1000 });

  await r.beat('today');
  await r.callout('ring', { text: 'Today', sel: 'nav.nav a' }, { dur: 1.8 });
  await r.hover({ text: 'Today', sel: 'nav.nav a' }, { after: 300 });
  await r.callout('sweep', { css: '.strip', index: 0 }, { dur: 2.4, delay: 0.6 });
  await r.hover({ css: '.strip', index: 0 }, { after: 600 });
  await r.scroll(420);
  await r.hover({ css: '.chg .body h2', index: 3 }, { after: 300 });

  await r.beat('files');
  await r.click({ text: 'Files', sel: 'nav.nav a' }, { after: 1200 });
  await r.click({ text: 'Environments', sel: '.foldbtn' }, { after: 1000 });
  await r.click({ text: 'Forge', sel: '.foldbtn' }, { after: 1400 });
  await r.callout('sweep', { css: 'table.files tbody' }, { dur: 2.4 });
  await r.at(0.5);
  await r.callout('spot', { text: 'Approved', sel: 'td span, td button, td em' }, { label: 'Tags', dur: 3 });
  await r.hover({ text: 'Approved', sel: 'td span, td button, td em' }, { after: 400 });

  await r.beat('history');
  await r.callout('ring', { text: 'History', sel: 'nav.nav a' }, { dur: 1.6 });
  await r.click({ text: 'History', sel: 'nav.nav a' }, { after: 1600 });
  await r.at(0.55);
  await r.click({ text: 'People', sel: 'nav.nav a' }, { after: 1400 });

  await r.beat('leads');
  await r.click({ text: 'Today', sel: 'nav.nav a' }, { after: 1000 });
  await r.callout('tag', { css: 'h1' }, { label: 'For leads and producers', dur: 3.2 });
  await r.park(1000, 500, 900);
  await r.end(1.6);
}
scene.options = { view: 1440 };
