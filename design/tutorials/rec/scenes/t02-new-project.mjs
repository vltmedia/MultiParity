import fs from 'node:fs';
import { DESK, daemon } from './lib.mjs';
import { webReady } from './web.mjs';

// The folder that has been sitting on someone's disk: work, plus renders and a backup that
// should not go up.
function folder(r) {
  const F = 'D:/Work/Frostline';
  fs.rmSync(F, { recursive: true, force: true });
  const put = (p, mb, fill) => { fs.mkdirSync(`${F}/${p}`.replace(/\/[^/]+$/, ''), { recursive: true }); fs.writeFileSync(`${F}/${p}`, Buffer.alloc(Math.round(mb * 1048576), fill)); };
  r.sh(`python "${process.env.MP_DEMO || 'P:/temp/Claude/claude/7df7f3888d1632d5/a22ff03a-1f55-4c9c-8b8f-318966a068cc/scratchpad/demo'}/paint.py" "${F}/Textures/ice_cliff_albedo.png" ice 201`);
  r.sh(`python "${process.env.MP_DEMO || 'P:/temp/Claude/claude/7df7f3888d1632d5/a22ff03a-1f55-4c9c-8b8f-318966a068cc/scratchpad/demo'}/paint.py" "${F}/Textures/snow_ground.png" stone 202`);
  fs.mkdirSync(`${F}/Scenes`, { recursive: true });
  fs.copyFileSync('P:/temp/Claude/claude/7df7f3888d1632d5/a22ff03a-1f55-4c9c-8b8f-318966a068cc/scratchpad/Blender45_0.blend', `${F}/Scenes/frostline_main.blend`);
  put('Scenes/frostline_main.blend1', 0.4, 1);
  put('Audio/wind_loop.wav', 1.2, 2);
  put('Docs/pitch.md', 0.001, 65);
  for (let i = 1; i <= 6; i++) put(`Renders/turntable_${String(i).padStart(4, '0')}.png`, 0.3, 10 + i);
}

export default async function scene(r) {
  folder(r);
  daemon(r);
  await webReady(r, '/admin/projects');
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: 'h1' }, { ms: 1000 });

  await r.beat('make');
  await r.callout('ring', { text: 'New project', sel: 'button' }, { dur: 2 });
  await r.click({ text: 'New project', sel: 'button' }, { after: 900 });
  await r.click({ css: 'input[placeholder="Emberfall"]' }, { after: 200 });
  await r.type('Frostline', { cps: 12 });
  await r.choose({ text: 'Choose an organization', sel: 'select', up: 'select' }, 'Cinder Works');
  await r.callout('spot', { css: 'dialog p.fine, [role=dialog] p.fine, .dialog p.fine' }, { label: 'Junk rules included', dur: 3.6, pad: 6 });
  await r.hover({ css: 'dialog p.fine, [role=dialog] p.fine, .dialog p.fine' }, { after: 600 });
  await r.at(0.8);
  await r.click({ text: 'Make project', sel: 'button' }, { after: 1500 });

  await r.beat('sync');
  await r.open(DESK, 1800);
  await r.callout('spot', { text: 'Frostline', sel: 'article.proj .row strong', up: '.row' }, { dur: 3, pad: 4 });
  await r.click({ text: 'Frostline', sel: 'article.proj .row strong' }, { after: 800 });
  await r.callout('ring', { css: 'article.proj.open [data-act=connect]' }, { dur: 2 });
  await r.click({ css: 'article.proj.open [data-act=connect]' }, { after: 900 });
  await r.click({ css: '#cfolder' }, { after: 200 });
  await r.press('ctrl+a');
  await r.type('D:\\Work\\Frostline', { cps: 16 });

  await r.beat('check');
  await r.callout('tag', { css: '#dlg .note' }, { label: 'Nothing gets deleted', dur: 3 });
  await r.callout('ring', { text: 'Check the folder', sel: 'button' }, { dur: 2, delay: 1.4 });
  await r.at(0.45);
  await r.click({ text: 'Check the folder', sel: 'button' }, { after: 1400 });
  await r.callout('sweep', { css: '#cprev' }, { dur: 2.4 });
  await r.at(0.92);
  await r.click({ text: 'Sync this folder', sel: 'button' }, { after: 1800 });
  await r.click({ text: 'Your work and checkpoints', sel: 'button' }, { after: 1500 });

  await r.beat('ignore');
  await r.callout('sweep', { css: '.changes .files' }, { dur: 2.4, pad: 2 });
  await r.at(0.35);
  await r.click({ text: 'turntable_0001.png', sel: '.changes li strong' }, { right: true, after: 700 });
  await r.click({ text: 'Ignore…', sel: '#ctx li' }, { after: 1200 });
  await r.callout('spot', { css: '#dlg' }, { label: 'Keep junk out', dur: 3.6, pad: 0 });
  await r.click({ text: 'Everything in this folder', sel: '.choices strong' }, { after: 700 });
  await r.at(0.9);
  await r.click({ css: '#dlg-body button[value=go]' }, { after: 1500 });

  await r.beat('publish');
  await r.click({ css: '#w-msg' }, { after: 200 });
  await r.type('First version', { cps: 14 });
  await r.callout('ring', { css: '[data-w=publish]' }, { dur: 1.8 });
  await r.click({ css: '[data-w=publish]' }, { after: 2200 });
  await r.park(1000, 420, 900);
  await r.end(1.8);
}
scene.options = { view: 1200 };
