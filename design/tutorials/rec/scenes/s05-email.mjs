import { webReady } from './web.mjs';
import { TERM, cd, cmd, edit, line, replaceValue } from './term.mjs';

const ENV = `# Email: invites, "forgot password" links, requests to release, failed backups.
MP_EMAIL_FROM=
# SMTP:
MP_SMTP_HOST=
MP_SMTP_PORT=1025
MP_SMTP_SECURITY=none
MP_SMTP_USER=
MP_SMTP_PASSWORD=
# or SendGrid:
# MP_SENDGRID_API_KEY=`;

export default async function scene(r) {
  await r.open(TERM, 700);
  await cd(r, '~/MultiParity');
  await r.park(1100, 640, 10);
  await r.record();

  await r.beat('hello');
  await cmd(r, 'nano .env', 12);
  await edit(r, '.env', ENV);

  await r.beat('why');
  await r.callout('spot', { css: '#code' }, { dur: 3, pad: 0 });
  await r.park(900, 500, 1200);

  await r.beat('set');
  await replaceValue(r, 2, '"MultiParity <multiparity@cinderworks.studio>"', 22);
  await replaceValue(r, 4, 'smtp.cinderworks.studio', 16);
  await replaceValue(r, 7, 'multiparity@cinderworks.studio', 22);
  await r.callout('code', line(8), { label: 'An app password', dur: 3 });
  await replaceValue(r, 8, 'abcd efgh ijkl mnop', 16);
  await r.callout('code', line(10), { label: 'Or SendGrid', dur: 2.4 });

  await r.beat('test');
  await cmd(r, 'docker compose up -d', 14);
  await webReady(r, '/admin/fixing');
  await r.click({ text: 'Send me a test email', sel: 'button' }, { ms: 10, before: 0, after: 0 }).catch(() => {});
  await r.scroll(1400);
  await r.callout('ring', { text: 'Send me a test email', sel: 'button' }, { dur: 2.2 });
  await r.click({ text: 'Send me a test email', sel: 'button' }, { after: 2500 });
  await r.park(900, 500, 900);
  await r.end(1.8);
}
scene.options = { view: 1440 };
