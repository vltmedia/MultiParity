import fs from 'node:fs';
import { DESK, as, daemon, paint } from './lib.mjs';
import { EMBER } from './desk.mjs';

// A day's work in Alice's Emberfall folder: textures repainted, new files, one deleted.
function dayOfWork(r) {
  daemon(r);
  if (!/Nothing to publish/.test(as(r, 'you', 'status', EMBER))) as(r, 'you', 'discard .', EMBER);
  if (!/ on Main,/.test(as(r, 'you', 'status', EMBER))) as(r, 'you', 'branch switch Main', EMBER);
  as(r, 'you', 'get-latest', EMBER);
  paint(r, `${EMBER}/Characters/Hero/hero_diffuse.png`, 'skin2', 12);
  paint(r, `${EMBER}/Characters/EmberWolf/ember_wolf_fur.png`, 'fur', 13);
  paint(r, `${EMBER}/Environments/Forge/forge_beams_wood.png`, 'wood', 14);
  paint(r, `${EMBER}/Environments/Forge/forge_floor_stone.png`, 'stone', 15);
  paint(r, `${EMBER}/Environments/Shore/frozen_sea.png`, 'ice', 16);
  fs.mkdirSync(`${EMBER}/Environments/Forge/Props`, { recursive: true });
  paint(r, `${EMBER}/Environments/Forge/Props/anvil_metal.png`, 'stone', 17);
  paint(r, `${EMBER}/Environments/Forge/Props/bellows_leather.png`, 'wood', 18);
  fs.copyFileSync(`${EMBER}/Environments/Forge/forge_door.fbx`, `${EMBER}/Environments/Forge/Props/anvil.fbx`);
  fs.appendFileSync(`${EMBER}/Docs/art_brief.md`, '\n- Forge props: anvil and bellows, weathered.\n');
  fs.rmSync(`${EMBER}/Environments/Forge/Bakes/forge_lightmap_02.exr`, { force: true });
}

export default async function scene(r) {
  dayOfWork(r);
  await r.open(DESK, 1800);
  await r.click({ text: 'Emberfall', sel: '#here button' }, { ms: 10, after: 2500 });
  await r.park(900, 480, 10);
  await r.record();

  await r.beat('hello');
  await r.callout('sweep', { css: '.changes .files' }, { dur: 2.6, pad: 2 });
  await r.hover({ css: '.changes h2' }, { ms: 900 });

  await r.beat('search');
  await r.click({ css: '#w-q' }, { after: 200 });
  await r.type('forge', { cps: 9 });
  await r.callout('tag', { css: '#w-q' }, { label: 'Find by name', dur: 2.2 });
  await r.at(0.42);
  await r.press('ctrl+a');
  await r.press('Backspace');
  await r.choose({ css: '#w-type' }, 'Images');
  await r.callout('ring', { css: '#w-type' }, { dur: 1.6 });
  await r.at(0.8);
  await r.choose({ css: '#w-sort' }, 'Sort: Size');

  await r.beat('pick');
  await r.callout('ring', { css: '[data-w=clear-vis]' }, { dur: 1.6 });
  await r.click({ css: '[data-w=clear-vis]' }, { after: 900 });
  await r.click({ css: '[data-w=sel-vis]' }, { after: 900 });
  await r.at(0.55);
  await r.click({ css: '[data-w=reset-view]' }, { after: 700 });
  await r.click({ css: '[data-w=clear-all]' }, { after: 600 });
  await r.click({ css: '.changes .files li [data-w=tick]', index: 0 }, { after: 400 });
  await r.click({ css: '.changes .files li [data-w=tick]', index: 3 }, { after: 600, shift: true });
  await r.callout('sweep', { css: '.changes .files li', index: 1 }, { dur: 1.8, pad: 2 });

  await r.beat('thumbs');
  await r.callout('ring', { css: '.changes .files .th', index: 0 }, { dur: 1.8 });
  await r.click({ css: '.changes .files .th', index: 0 }, { after: 2400 });
  await r.at(0.8);
  await r.press('Escape');

  await r.beat('hidden');
  await r.choose({ css: '#w-type' }, '3D scenes');
  await r.callout('spot', { css: '.publish .note' }, { dur: 3.4 });
  await r.hover({ css: '.publish .note' }, { after: 600 });
  await r.end(1.6);
}
scene.options = { view: 1200 };
