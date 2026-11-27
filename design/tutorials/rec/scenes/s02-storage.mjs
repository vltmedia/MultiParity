import { rowMenu, webReady } from './web.mjs';
import { ENV, TERM, cmd, cd, edit, line, replaceValue, typeAt } from './term.mjs';

const MORE = ENV + `

# Several places at once: a NAS for active projects, a bucket for finished ones.
# MP_STORAGE_LOCATIONS=`;

export default async function scene(r) {
  await r.open(TERM, 800);
  await cd(r, '~/MultiParity');
  await r.park(1100, 640, 10);
  await r.record();

  await r.beat('hello');
  await cmd(r, 'nano .env', 12);
  await edit(r, '.env', MORE);

  await r.beat('local');
  await r.callout('code', line(17), { label: 'The default', dur: 3 });

  await r.beat('mount');
  await replaceValue(r, 17, 'mount', 12);
  await r.eval(`T.setLine(19, 'MP_BLOB_DIR=')`);
  await typeAt(r, 19, '/mnt/nas/multiparity', 14);
  await r.callout('code', line(19), { label: 'Your NAS or second drive', dur: 3.4 });

  await r.beat('s3');
  await replaceValue(r, 17, 's3', 12);
  await r.callout('code', line(17), { label: 'B2, R2, Wasabi, AWS, MinIO', dur: 3.8 });

  await r.beat('move');
  await r.eval(`T.setLine(22, 'MP_STORAGE_LOCATIONS=')`);
  await typeAt(r, 22, '"nas=mount:/data/nas archive=s3:bucket=mp-archive;..."', 22);
  await r.callout('code', line(22), { label: 'More than one place', dur: 2.6 });
  await webReady(r, '/admin/projects');
  await rowMenu(r, 'Emberfall', 'Change storage…', { after: 1200 });
  await r.callout('spot', { css: 'dialog, [role=dialog]' }, { label: 'Live, no downtime', dur: 3.6, pad: 0 });
  await r.park(900, 450, 900);
  await r.end(1.6);
  await r.press('Escape').catch(() => {});
}
scene.options = { view: 1440 };
