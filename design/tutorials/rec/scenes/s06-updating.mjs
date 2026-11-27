import { webReady } from './web.mjs';
import { TERM, cd, cmd, out, lastCmd } from './term.mjs';

export default async function scene(r) {
  await webReady(r, '/admin/fixing');
  await r.scroll(700);
  await r.park(900, 500, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ text: 'Backups', sel: 'h2' }, { ms: 1000 });

  await r.beat('backup');
  await r.callout('ring', { text: 'Back up now', sel: 'button' }, { dur: 2 });
  await r.click({ text: 'Back up now', sel: 'button' }, { after: 2200 });

  await r.beat('pull');
  await r.open(TERM, 700);
  await cd(r, '~/MultiParity');
  await cmd(r, 'git pull', 12);
  await out(r, ['Updating b45a0b4..e1c07d2', 'Fast-forward', ' 14 files changed, 412 insertions(+), 96 deletions(-)'], 120);
  await cmd(r, 'docker compose up -d --build', 14);
  await out(r, [['[+] Building 96.2s (24/24) FINISHED', 'dim'], ['[+] Running 2/2', 'o'], [' ✔ Container multiparity-db-1      Healthy', 'ok'], [' ✔ Container multiparity-server-1  Started', 'ok']], 260);
  await r.callout('code', lastCmd, { label: 'The database updates itself', dur: 3 });

  await r.beat('apps');
  // The desktop app's own notification wording when an update is downloaded.
  await r.toast('MultiParity update ready', 'Version 0.1.3 is downloaded. Restart MultiParity when it suits you.', 5200);
  await r.callout('tag', { rect: { x: r.W - 18 - 340, y: r.H - 18 - 96, w: 340, h: 96 } }, { label: 'Signed updates', dur: 3, delay: 0.5 });
  await r.park(900, 500, 900);
  await r.end(1.8);
}
scene.options = { view: 1440 };
