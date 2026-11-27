import { deskReady } from './desk.mjs';
import { webReady } from './web.mjs';

// The montage: a spread across the groups, each shown on the project view with its name.
const TOUR = [
  ['aero', 'Frutiger Aero'], ['terminal', 'Phosphor Terminal'], ['dracula', 'Dracula'], ['neon', 'Neon Grid'],
  ['acid', 'Acid Pop'], ['pocket', 'Pocket Handheld'], ['bevel', 'Classic Bevel'], ['sticky', 'Sticky Notes'],
  ['vapor', 'Vaporwave'], ['bauhaus', 'Bauhaus'], ['arcade', 'Pixel Arcade'], ['flowers', 'Pressed Flowers'],
];
const navTo = (r, name) => r.click({ text: name, sel: 'nav.nav a, nav.nav button' }, { after: 1000 });

export default async function scene(r) {
  await deskReady(r);
  // Put back whatever look the desktop had, after the take.
  const before = await r.eval(`MPTheme.saved()`);
  await r.eval(`MPTheme.save(MPTheme.AUTO); MPTheme.applyChosen()`);
  await navTo(r, 'Accounts');
  await r.eval(`document.getElementById('look-h').scrollIntoView({ block: 'start' })`);
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.callout('sweep', { css: '#looks' }, { dur: 3, pad: 4 });
  await r.hover({ css: '#looks .look', index: 3 }, { after: 600 });

  await r.beat('pick');
  await r.click({ text: 'Glass', sel: '#look-filters button' }, { after: 900 });
  await r.callout('ring', { text: 'Liquid Glass', sel: '#looks .look strong', up: '.look' }, { dur: 1.6 });
  await r.click({ text: 'Liquid Glass', sel: '#looks .look strong' }, { after: 1600 });
  await r.at(0.6);
  await r.click({ text: 'Novelty', sel: '#look-filters button' }, { after: 700 });
  await r.click({ text: 'Notebook', sel: '#looks .look strong' }, { after: 1400 });

  await r.beat('tour');
  await r.click({ text: 'Emberfall', sel: '#here button' }, { after: 900 });
  for (const [id, name] of TOUR) {
    await r.eval(`MPTheme.save(${JSON.stringify(id)}); MPTheme.applyChosen()`);
    await r.callout('tag', { css: 'h1' }, { label: name, dur: 1.05 });
    await r.park(1000 + Math.round(Math.random() * 200), 420, 1100);
  }

  await r.beat('auto');
  await navTo(r, 'Accounts');
  await r.click({ text: 'All', sel: '#look-filters button' }, { after: 600 });
  await r.eval(`document.getElementById('look-h').scrollIntoView({ block: 'start' })`);
  await r.callout('ring', { text: 'Automatic', sel: '#looks .look strong', up: '.look' }, { dur: 2 });
  await r.click({ text: 'Automatic', sel: '#looks .look strong' }, { after: 1600 });
  await r.callout('tag', { text: 'Automatic', sel: '#looks .look strong', up: '.look' }, { label: 'Light and dark, by itself', dur: 2.8 });

  await r.beat('web');
  await webReady(r, '/look');
  await r.callout('sweep', { css: '.looks, main', index: 0 }, { dur: 2.6, pad: 4 });
  await r.park(900, 500, 900);
  await r.end(1.8);
  // Off camera: the desktop goes back to the look it had.
  await r.open('http://127.0.0.1:4176/?daemon=18529', 1200);
  await r.eval(`MPTheme.save(${JSON.stringify(before || 'auto')}); MPTheme.applyChosen()`);
}
scene.options = { view: 1200 };
