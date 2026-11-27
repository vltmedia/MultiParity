import { WEB } from './lib.mjs';
import { TERM, cd, cmd, edit, line } from './term.mjs';
import { rowButton, rowMenu, webReady } from './web.mjs';

// The server's email settings, filled in (the password is left for the person to type).
const SMTP_ENV = `# Email: invites, "forgot password" links, requests to release, failed backups.
MP_EMAIL_FROM="MultiParity <multiparity@cinderworks.studio>"
# SMTP:
MP_SMTP_HOST=smtp.cinderworks.studio
MP_SMTP_PORT=587
MP_SMTP_SECURITY=starttls
MP_SMTP_USER=multiparity@cinderworks.studio
MP_SMTP_PASSWORD=
# or SendGrid:
# MP_SENDGRID_API_KEY=`;

export default async function scene(r) {
  await webReady(r, '/admin');
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.callout('spot', { css: 'nav, aside', index: 0 }, { dur: 3, pad: 0 });
  await r.hover({ text: 'Overview', sel: 'a' }, { ms: 1000 });

  await r.beat('invite');
  await r.click({ text: 'Invites', sel: 'a' }, { after: 1200 });
  await r.click({ css: 'textarea' }, { after: 300 });
  await r.type('lena@cinderworks.studio', { cps: 18 });
  await r.callout('ring', { css: '.inv .panel select', index: 0 }, { dur: 2 });
  await r.choose({ css: '.inv .panel select', index: 0 }, 'Member');
  await r.callout('ring', { text: 'Make invites', sel: 'button' }, { dur: 1.8 });
  await r.click({ text: 'Make invites', sel: 'button' }, { after: 1500 });

  // Emailed invites need the server's email: a look at those lines in its settings.
  await r.beat('email');
  await r.open(TERM, 600);
  await cd(r, '~/MultiParity');
  await cmd(r, 'nano .env', 26);
  await edit(r, '.env', SMTP_ENV);
  await r.callout('spot', { css: '#code' }, { label: 'Needed for emailed invites', dur: 3.4, pad: 0 });
  await r.at(0.45);
  await r.callout('code', line(4), { label: 'Your SMTP server', dur: 2.4 });
  await r.at(0.72);
  await r.callout('code', line(10), { label: 'Or a SendGrid key', dur: 2.4 });

  // No email: an invite link instead.
  await r.beat('link');
  await r.open(`${WEB}/admin/invites`, 1600);
  await r.callout('ring', { text: 'By link', sel: 'button' }, { dur: 1.8 });
  await r.click({ text: 'By link', sel: 'button' }, { after: 900 });
  await r.callout('ring', { text: 'Make invite link', sel: 'button' }, { dur: 2.2 });
  await r.hover({ text: 'Make invite link', sel: 'button' }, { after: 600 });

  await r.beat('projects');
  await r.click({ text: 'Projects', sel: 'a' }, { after: 1200 });
  await r.callout('tag', { text: 'New project', sel: 'button' }, { label: 'Admins only', dur: 3 });
  await r.click({ text: 'New project', sel: 'button' }, { after: 1000 });
  await r.callout('spot', { text: 'Store file data in', sel: 'label span', up: 'label' }, { dur: 3.6, pad: 8 });
  await r.hover({ text: 'Store file data in', sel: 'label span' }, { after: 500 });
  await r.at(0.75);
  await r.click({ text: 'Cancel', sel: 'button' }, { after: 600 });
  // Change storage lives in each project's More menu: show it, then close the menu.
  const more = await rowButton(r, 'Emberfall', 'More');
  await r.click({ text: 'More', sel: 'button', index: more }, { after: 500 });
  await r.callout('ring', { text: 'Change storage…', sel: '.menu button' }, { dur: 2.2 });
  await r.hover({ text: 'Change storage…', sel: '.menu button' }, { after: 400 });
  await r.press('Escape');

  await r.beat('look');
  await rowMenu(r, 'Emberfall', 'Look…', { after: 1400 });
  await r.callout('spot', { css: 'dialog, [role=dialog]' }, { dur: 3.4, pad: 0 });
  await r.park(900, 450, 900);
  await r.at(0.85);
  await r.press('Escape');

  await r.beat('bye');
  await r.click({ text: 'All projects', sel: 'a' }, { after: 1200 });
  await r.park(900, 500, 900);
  await r.end(2);
}
scene.options = { view: 1440 };
