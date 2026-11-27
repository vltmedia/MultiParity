import fs from 'node:fs';
import { DESK, SECRETS, daemon, stopDaemon } from './lib.mjs';

export default async function scene(r) {
  stopDaemon(r);
  fs.rmSync('D:/Work/Emberfall', { recursive: true, force: true, maxRetries: 5, retryDelay: 300 });
  daemon(r, 'cfg-you', { fresh: true });
  await r.open(DESK, 2000);
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: '.main .head h1' }, { ms: 900 });
  await r.at(0.6);
  await r.park(1000, 520, 900);

  await r.beat('signin');
  await r.callout('ring', { css: 'button[data-view=accounts]' }, { dur: 2 });
  await r.click({ css: 'button[data-view=accounts]' }, { after: 700 });
  await r.callout('tag', { css: 'input[name=server]' }, { label: "Your studio's server", dur: 3, delay: 0.3 });
  await r.click({ css: 'input[name=server]' }, { after: 200 });
  await r.type('mp.cinderworks.studio');
  await r.click({ css: 'input[name=email]' }, { after: 200 });
  await r.type('alice@cinderworks.studio', { cps: 26 });
  await r.click({ css: 'input[name=password]' }, { after: 200 });
  await r.type(SECRETS.ADMIN_PW, { cps: 22 });
  await r.callout('ring', { css: '#login button[type=submit]' }, { dur: 1.6 });
  await r.click({ css: '#login button[type=submit]' }, { after: 1500 });

  await r.beat('project');
  await r.click({ css: 'button[data-view=projects]' }, { after: 900 });
  await r.callout('spot', { css: 'article.proj .row', index: 0 }, { dur: 3.2, pad: 4 });
  await r.hover({ css: 'article.proj .row strong' }, { after: 600 });
  await r.at(0.55);
  await r.click({ css: 'article.proj .row strong' }, { after: 700 });
  await r.callout('ring', { css: '[data-act=connect]' }, { dur: 2.2 });
  await r.click({ css: '[data-act=connect]' }, { after: 900 });

  await r.beat('folder');
  await r.callout('spot', { css: '#dlg' }, { dur: 3.4, pad: 0 });
  await r.click({ css: '#cfolder' }, { after: 200 });
  await r.press('ctrl+a');
  await r.type('D:\\Work\\Emberfall', { cps: 16 });
  await r.callout('tag', { css: '#cfolder' }, { label: 'Any folder, any drive', dur: 2.6 });
  await r.at(0.8);
  await r.click({ text: 'Sync this folder', sel: 'button' }, { after: 600 });

  await r.beat('done');
  await r.park(1050, 600, 900);
  await r.callout('sweep', { css: 'article.proj .row', index: 0 }, { dur: 2.2 });
  await r.at(0.7);
  await r.hover({ css: '#here' }, { after: 400 });

  await r.beat('more');
  await r.click({ css: 'button[data-view=accounts]' }, { after: 800 });
  await r.callout('spot', { css: '#login' }, { dur: 3, pad: 4 });
  await r.hover({ text: 'Add an account', sel: 'h2' }, { after: 600 });
  await r.at(0.6);
  await r.click({ css: 'button[data-view=projects]' }, { after: 600 });
  await r.park(1100, 560, 900);
  await r.end(1.8);
}
scene.options = { view: 1200 };
