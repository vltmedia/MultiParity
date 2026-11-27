import { webReady } from './web.mjs';
import { TERM, cd, cmd, out, lastCmd } from './term.mjs';

export default async function scene(r) {
  await webReady(r, '/admin/fixing');
  await r.choose({ css: 'aside select, nav select, select', index: 0 }, 'Cinder Works', { ms: 10 });
  await r.sleep(800);
  await r.park(900, 500, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ text: 'Webhooks', sel: 'h2' }, { ms: 1000 });

  await r.beat('hook');
  await r.click({ css: 'input[placeholder="https://hooks.example.com/multiparity"]' }, { after: 300 });
  await r.type('http://hooks.cinderworks.studio/multiparity', { cps: 22 });
  await r.choose({ text: 'Which events', sel: 'label span', up: 'label' }, '', { after: 300 }).catch(() => {});
  await r.callout('ring', { text: 'Add webhook', sel: 'button' }, { dur: 1.8 });
  await r.click({ text: 'Add webhook', sel: 'button' }, { after: 1500 });
  await r.at(0.75);
  await r.callout('ring', { text: 'Send a test', sel: 'button' }, { dur: 2 });
  await r.click({ text: 'Send a test', sel: 'button' }, { after: 1800 });

  await r.beat('signed');
  await r.callout('spot', { css: '.result.good' }, { label: 'Signed with this secret', dur: 3.6, pad: 6 }).catch(() => {});
  await r.park(900, 480, 900);

  await r.beat('watch');
  await r.open(TERM, 700);
  await cd(r, '~/build');
  await cmd(r, 'mp watch --project Emberfall --event change.published --exec ./on-publish.sh', 20);
  await r.callout('code', lastCmd, { label: 'Runs your script', dur: 3 });
  await out(r, [['Watching Emberfall for change.published. Ctrl+C to stop.', 'dim']], 100);
  await r.end(1.8);
}
scene.options = { view: 1440 };
