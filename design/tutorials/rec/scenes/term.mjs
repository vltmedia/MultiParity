// Helpers for the self-hosting scenes: a terminal and editor page (../term.html) driven from here.
import path from 'node:path';
import { pathToFileURL } from 'node:url';

export const TERM = pathToFileURL(path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Z]:)/, '$1')), '..', 'term.html')).href;
export const js = (v) => JSON.stringify(v);
export const cmd = (r, text, cps = 17) => r.eval(`T.cmd(${js(text)}, ${cps})`);
export const out = (r, lines, gap = 60) => r.eval(`T.out(${js(lines)}, ${gap})`);
export const ready = (r) => r.eval('T.ready()');
export const cd = (r, dir) => r.eval(`T.cd(${js(dir)})`);
export const edit = (r, name, text) => r.eval(`T.edit(${js(name)}, ${js(text)})`);
export const replaceValue = (r, n, text, cps = 16) => r.eval(`T.replaceValue(${n}, ${js(text)}, ${cps})`);
export const typeAt = (r, n, text, cps = 16) => r.eval(`T.typeAt(${n}, ${js(text)}, ${cps})`);
export const closeEditor = (r) => r.eval('T.close()');
export const line = (n) => ({ css: `#L${n}` });
export const lastCmd = { css: '#term .cmd', last: true };

// The top of .env.example, as people see it after copying it.
export const ENV = `# Copy to .env and edit. Everything except the two passwords has a working default.

# The first server admin is created from these on first start (password: 10+ characters).
MP_ADMIN_EMAIL=admin@studio.example
MP_ADMIN_PASSWORD=
# Name of the organization created with the first admin.
MP_DEFAULT_ORG="My Studio"

# Database password (used by both the server and Postgres).
POSTGRES_PASSWORD=

# The address people use to reach this server; used in invite links.
MP_PUBLIC_URL=http://localhost:18500
MP_PORT=18500

# Where file data lives: local (Docker volume), mount (host folder or network share), or s3.
MP_STORAGE=local
# For MP_STORAGE=mount: the host folder to use, e.g. /mnt/nas/multiparity
# MP_BLOB_DIR=/mnt/nas/multiparity`;

export const COMPOSE_UP = [
  ['[+] Building 214.6s (24/24) FINISHED', 'dim'],
  ['[+] Running 7/7', 'o'],
  [' ✔ Network multiparity_default      Created', 'ok'],
  [' ✔ Volume "multiparity_db"          Created', 'ok'],
  [' ✔ Volume "multiparity_blobs"       Created', 'ok'],
  [' ✔ Volume "multiparity_backups"     Created', 'ok'],
  [' ✔ Volume "multiparity_desktop"     Created', 'ok'],
  [' ✔ Container multiparity-db-1       Healthy', 'ok'],
  [' ✔ Container multiparity-server-1   Started', 'ok'],
];
