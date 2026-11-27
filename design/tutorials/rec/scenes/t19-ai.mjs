import { WEB, webSignIn } from './lib.mjs';
import { webApi } from './web.mjs';

// The token made on camera is for the local demo server, and is revoked after the take.
const NAME = 'Claude on my laptop';
const tidy = async (r) => {
  for (const t of (await webApi(r, 'GET', '/me/tokens')).filter((t) => t.label === NAME)) await webApi(r, 'DELETE', `/me/tokens/${t.id}`);
};

export default async function scene(r) {
  await webSignIn(r);
  await tidy(r);
  await r.open(`${WEB}/`, 2000);
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.hover({ css: 'h1' }, { ms: 1000 });

  await r.beat('token');
  await r.click({ css: '.acct button' }, { after: 700 });
  await r.callout('ring', { text: 'Access tokens', sel: 'a' }, { dur: 1.6 });
  await r.click({ text: 'Access tokens', sel: 'a' }, { after: 1300 });
  await r.click({ css: 'input[aria-label="Token name"]' }, { after: 200 });
  await r.type(NAME, { cps: 14 });
  await r.click({ text: 'Make token', sel: 'button' }, { after: 1400 });

  await r.beat('setup');
  await r.callout('spot', { css: '.token-made .tok-row', index: 1 }, { dur: 3, pad: 6 });
  await r.hover({ css: '.token-made .tok-row', index: 1 }, { after: 600 });
  await r.at(0.55);
  await r.callout('ring', { text: 'Copy', sel: '.token-made .tok-row button', index: 0 }, { dur: 1.8 });
  await r.hover({ text: 'Copy', sel: '.token-made .tok-row button', index: 0 }, { after: 800 });

  await r.beat('ask');
  await r.hover({ css: '.token-made pre' }, { after: 600 });
  await r.callout('tag', { css: '.token-made pre' }, { label: 'Any MCP assistant', dur: 2.6 });

  await r.beat('safe');
  await r.scroll(500);
  await r.callout('ring', { text: 'Revoke', sel: 'button' }, { dur: 1.8 });
  await r.hover({ text: 'Revoke', sel: 'button' }, { after: 900 });
  await r.end(1.8);
  await tidy(r);
}
scene.options = { view: 1440 };
