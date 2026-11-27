import { SECRETS, WEB } from './lib.mjs';
import { COMPOSE_UP, ENV, TERM, cd, closeEditor, cmd, edit, lastCmd, line, out, replaceValue } from './term.mjs';

export default async function scene(r) {
  await r.open(TERM, 800);
  await r.park(1100, 640, 10);
  await r.record();

  await r.beat('hello');
  await r.park(1000, 600, 1200);

  await r.beat('clone');
  await cmd(r, 'git clone https://github.com/vltmedia/MultiParity.git');
  await out(r, ["Cloning into 'MultiParity'...", 'remote: Total 857 (delta 415), reused 693 (delta 251), pack-reused 0 (from 0)', 'Receiving objects: 100% (857/857), 4.95 MiB | 23.92 MiB/s, done.', 'Resolving deltas: 100% (415/415), done.'], 120);
  await cmd(r, 'cd MultiParity'); await cd(r, '~/MultiParity');
  await cmd(r, 'cp .env.example .env');
  await r.callout('code', lastCmd, { label: 'Your settings file', dur: 2.6 });

  await r.beat('env');
  await cmd(r, 'nano .env', 12);
  await edit(r, '.env', ENV);
  await r.sleep(500);
  await r.callout('code', line(4), { label: 'You', dur: 3.2 });
  await replaceValue(r, 4, 'alice@cinderworks.studio');
  await replaceValue(r, 5, 'correct-horse-battery-42');
  await r.at(0.42);
  await r.callout('code', line(10), { label: 'Long and random', dur: 3.2 });
  await replaceValue(r, 10, 'p9X!c2#Wq8z@Lm4%Tr7v^Kd1', 22);
  await r.at(0.72);
  await r.callout('code', line(13), { label: 'The address people use', dur: 3.6 });
  await replaceValue(r, 13, 'https://mp.cinderworks.studio', 14);

  await r.beat('up');
  await closeEditor(r);
  await cmd(r, 'docker compose up -d', 12);
  await out(r, COMPOSE_UP, 260);
  await r.callout('code', lastCmd, { label: 'Server and database, running', dur: 3 });

  await r.beat('signin');
  await r.open(`${WEB}/sign-in`, 1500);
  await r.click({ css: 'input[type=email], input[name=email]' }, { after: 200 });
  await r.type('alice@cinderworks.studio', { cps: 24 });
  await r.click({ css: 'input[type=password]' }, { after: 200 });
  await r.type(SECRETS.ADMIN_PW, { cps: 22 });
  await r.press('Enter');
  await r.sleep(1800);
  await r.callout('ring', { text: 'Admin', sel: 'a' }, { dur: 2.4 });
  await r.hover({ text: 'Admin', sel: 'a' }, { after: 400 });
  await r.end(2);
}
scene.options = { view: 1440 };
