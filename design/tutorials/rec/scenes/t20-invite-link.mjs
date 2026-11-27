import crypto from 'node:crypto';
import { WEB } from './lib.mjs';
import { scopeCinder, webApi, webReady } from './web.mjs';

// The person who joins on camera. Each take removes the account the last take made, so the
// same name and address can be used again.
const NAME = 'Lena Ortiz';
const EMAIL = 'lena.ortiz@cinderworks.studio';

async function tidy(r) {
  const users = await webApi(r, 'GET', '/admin/users');
  for (const u of (Array.isArray(users) ? users : []).filter((u) => u.email === EMAIL)) {
    await webApi(r, 'DELETE', `/admin/users/${u.id}`, { confirm: u.name });
  }
}

export default async function scene(r) {
  await webReady(r, '/admin');
  const org = await scopeCinder(r);
  await tidy(r);
  // Earlier takes' unused links, so the list under the form starts clean.
  for (const i of (await webApi(r, 'GET', `/admin/orgs/${org}/invites`)) || []) if (!i.email && !i.accepted_at) await webApi(r, 'DELETE', `/admin/invites/${i.id}`);
  await r.open(`${WEB}/admin/invites`, 1800);
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.callout('spot', { css: '.inv .panel', index: 0 }, { dur: 3, pad: 4 });
  await r.hover({ css: 'h1' }, { ms: 900 });

  await r.beat('make');
  await r.callout('ring', { text: 'By link', sel: 'button' }, { dur: 1.6 });
  await r.click({ text: 'By link', sel: 'button' }, { after: 800 });
  await r.callout('ring', { css: '.inv .panel select', index: 0 }, { dur: 1.8 });
  await r.choose({ css: '.inv .panel select', index: 0 }, 'Member');
  await r.at(0.55);
  const last = await r.eval(`document.querySelectorAll('.inv .panel select').length - 1`);
  await r.callout('tag', { css: '.inv .panel select', index: last }, { label: 'How long it works', dur: 2.4 });
  await r.choose({ css: '.inv .panel select', index: last }, '7 days');
  await r.at(0.85);
  await r.click({ text: 'Make invite link', sel: 'button' }, { after: 1400 });

  await r.beat('copy');
  await r.callout('spot', { css: '.result' }, { label: 'One person, once', dur: 3.2, pad: 6 });
  await r.click({ text: 'Copy', sel: '.result button' }, { after: 900 });
  await r.at(0.6);
  await r.scroll(500);
  await r.callout('ring', { text: 'Revoke', sel: 'button' }, { dur: 2.2 });
  await r.hover({ text: 'Revoke', sel: 'button' }, { after: 600 });

  await r.beat('join');
  // What the person sees: the same link, opened while signed out.
  // The link carries the server's public address; the recorder reaches the same server locally.
  const url = (await r.eval(`document.querySelector('.result code').textContent.trim()`)).replace(/^https?:\/\/[^/]+/, WEB);
  await r.eval(`localStorage.removeItem('mp.token')`);
  await r.open(url, 1600);
  await r.click({ css: '.auth input', index: 0 }, { after: 200 });
  await r.type(NAME, { cps: 14 });
  await r.click({ css: '.auth input[type=email]' }, { after: 200 });
  await r.type(EMAIL, { cps: 20 });
  const pw = crypto.randomBytes(9).toString('base64url');
  await r.click({ css: '.auth input[type=password]', index: 0 }, { after: 150 });
  await r.type(pw, { cps: 24 });
  await r.click({ css: '.auth input[type=password]', index: 1 }, { after: 150 });
  await r.type(pw, { cps: 24 });
  await r.callout('ring', { text: 'Create account and join', sel: 'button' }, { dur: 1.6 });
  await r.click({ text: 'Create account and join', sel: 'button' }, { after: 2000 });

  await r.beat('done');
  await r.callout('tag', { css: 'h1' }, { label: 'In, as a member', dur: 2.8 });
  await r.park(1000, 500, 900);
  await r.end(1.8);
  // Off camera: back to the admin, and the take's account goes again.
  await webReady(r, '/admin');
  await tidy(r);
}
scene.options = { view: 1440 };
