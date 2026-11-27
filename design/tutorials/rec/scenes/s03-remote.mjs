import { TERM, cmd, cd, edit, line, out, replaceValue, lastCmd } from './term.mjs';

const DOCS = 'https://vltmedia.github.io/MultiParity/docs/REMOTE';
const ENV = `# Reaching the server from outside the office (docs/REMOTE.md).
MP_DOMAIN=
MP_TLS=
MP_PUBLIC_URL=http://localhost:18500
MP_CLIENT_IP_HEADER=
MP_BIND=0.0.0.0`;

export default async function scene(r) {
  await r.open(DOCS, 2500);
  await r.park(900, 500, 10);
  await r.record();

  await r.beat('hello');
  await r.scroll(260);
  const table = { css: '.vp-doc table', index: 0 };
  await r.callout('spot', table, { dur: 3.4, pad: 6 });

  await r.beat('tailscale');
  await r.callout('sweep', { text: 'Tailscale', sel: 'td', up: 'tr' }, { dur: 2.6 });
  await r.hover({ text: 'Tailscale', sel: 'td' }, { after: 400 });

  await r.beat('cloudflare');
  await r.callout('sweep', { text: 'Cloudflare tunnel', sel: 'td', up: 'tr' }, { dur: 2.6 });
  await r.hover({ text: 'Cloudflare tunnel', sel: 'td' }, { after: 400 });

  await r.beat('caddy');
  await r.callout('sweep', { text: 'Caddy (built in)', sel: 'td', up: 'tr' }, { dur: 1.8 });
  await r.open(TERM, 700);
  await cd(r, '~/MultiParity');
  await cmd(r, 'nano .env', 14);
  await edit(r, '.env', ENV);
  await replaceValue(r, 2, 'mp.cinderworks.studio', 16);
  await replaceValue(r, 3, 'alice@cinderworks.studio', 18);
  await r.callout('code', line(2), { label: 'Your domain', dur: 2.4 });
  await replaceValue(r, 4, 'https://mp.cinderworks.studio', 20);
  await replaceValue(r, 5, 'X-Forwarded-For', 22);
  await replaceValue(r, 6, '127.0.0.1', 22);
  await r.eval('T.close()');
  await cmd(r, 'docker compose --profile https up -d', 16);
  await out(r, [['[+] Running 3/3', 'o'], [' ✔ Container multiparity-db-1      Healthy', 'ok'], [' ✔ Container multiparity-server-1  Started', 'ok'], [' ✔ Container multiparity-caddy-1   Started', 'ok']], 220);
  await r.callout('code', lastCmd, { label: 'Certificates handled', dur: 2.6 });

  await r.beat('size');
  await r.park(1000, 560, 900);
  await r.end(1.6);
}
scene.options = { view: 1440 };
