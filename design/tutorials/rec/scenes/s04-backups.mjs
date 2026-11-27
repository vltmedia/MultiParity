import { webReady } from './web.mjs';
import { TERM, cd, cmd, edit, line, replaceValue } from './term.mjs';

const ENV = `# Nightly backups of the database and all file data (docs/BACKUP.md).
MP_BACKUP_TARGET=local:/data/backups
# MP_BACKUP_AT=03:00          # UTC
# MP_BACKUP_KEEP_DAILY=7
# MP_BACKUP_KEEP_WEEKLY=4`;
const DOCS = 'https://vltmedia.github.io/MultiParity/docs/BACKUP#restoring';

export default async function scene(r) {
  await webReady(r, '/admin/fixing');
  await r.scroll(700);
  await r.park(900, 500, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ text: 'Backups', sel: 'h2' }, { ms: 1000 });

  await r.beat('nightly');
  await r.callout('sweep', { css: 'table.t', index: 0 }, { dur: 2.6 }).catch(() => {});
  await r.hover({ text: 'New that night', sel: 'th' }, { after: 400 }).catch(() => {});

  await r.beat('target');
  await r.open(TERM, 700);
  await cd(r, '~/MultiParity');
  await cmd(r, 'nano .env', 14);
  await edit(r, '.env', ENV);
  await r.callout('code', line(2), { label: 'Same disk: not enough', dur: 2.8 });
  await r.at(0.55);
  await replaceValue(r, 2, 'mount:/data/backup-drive', 14);
  await r.callout('code', line(2), { label: 'Somewhere else!', dur: 2.6 });

  await r.beat('now');
  await webReady(r, '/admin/fixing');
  await r.scroll(700);
  await r.callout('ring', { text: 'Back up now', sel: 'button' }, { dur: 2 });
  await r.click({ text: 'Back up now', sel: 'button' }, { after: 2500 });
  await r.callout('sweep', { css: 'table.t', index: 0 }, { dur: 2.4 }).catch(() => {});

  await r.beat('restore');
  await r.open(DOCS, 2500);
  await r.callout('spot', { css: '.vp-doc ol', index: 0 }, { label: 'Tested end to end', dur: 4, pad: 6 }).catch(() => {});
  await r.park(900, 520, 900);
  await r.end(1.8);
}
scene.options = { view: 1440 };
