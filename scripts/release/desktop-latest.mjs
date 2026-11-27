#!/usr/bin/env node
// Collect a desktop build into a folder a MultiParity server hands out (MP_DESKTOP_DIR), and
// write or update its latest.json. Run once per build machine (Windows, Mac) into the same
// folder; platforms already there are kept.
//
//   node scripts/release/desktop-latest.mjs --out <folder> [--bundle desktop/src-tauri/target/release/bundle]
//        [--notes "What changed"] [--arch x86_64|aarch64] [--base-url <url>]
//
// --base-url makes the addresses in latest.json absolute, for files hosted elsewhere, e.g. a
// GitHub release: --base-url https://github.com/vltmedia/MultiParity/releases/download/v0.2.0
//
// The build must be signed for updates: set TAURI_SIGNING_PRIVATE_KEY (or _PATH) when running
// `pnpm tauri build` so each update bundle gets a .sig next to it (docs/DESKTOP.md).
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';

const args = Object.fromEntries(process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith('--') ? [...acc, [a.slice(2), all[i + 1] && !all[i + 1].startsWith('--') ? all[i + 1] : true]] : acc), []));
const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')), '../..');
const bundle = path.resolve(args.bundle || path.join(root, 'desktop/src-tauri/target/release/bundle'));
const out = args.out && path.resolve(args.out);
if (!out) {
  console.error('Usage: desktop-latest.mjs --out <folder> [--bundle <dir>] [--notes <text>] [--arch x86_64|aarch64]');
  process.exit(2);
}
const conf = JSON.parse(fs.readFileSync(path.join(root, 'desktop/src-tauri/tauri.conf.json'), 'utf8'));
const version = conf.version;
const arch = args.arch || (os.arch() === 'arm64' ? 'aarch64' : 'x86_64');
const list = (dir) => (fs.existsSync(dir) ? fs.readdirSync(dir).map((f) => path.join(dir, f)) : []);
const find = (dir, test) => list(path.join(bundle, dir)).filter((f) => test(path.basename(f)) && path.basename(f).includes(version));

// [platform key, update bundle, first-install download]
const found = [];
const nsis = find('nsis', (f) => f.endsWith('-setup.exe'));
if (nsis.length) found.push([`windows-${arch}`, nsis[0], nsis[0]]);
else {
  const msi = find('msi', (f) => f.endsWith('.msi'));
  if (msi.length) found.push([`windows-${arch}`, msi[0], msi[0]]);
}
const appTar = list(path.join(bundle, 'macos')).filter((f) => f.endsWith('.app.tar.gz'));
const dmg = find('dmg', (f) => f.endsWith('.dmg'));
if (appTar.length) {
  // A universal build serves both kinds of Mac.
  for (const a of arch === 'universal' ? ['aarch64', 'x86_64'] : [arch]) found.push([`darwin-${a}`, appTar[0], dmg[0] || null]);
}
const appimage = find('appimage', (f) => f.endsWith('.AppImage'));
if (appimage.length) found.push([`linux-${arch}`, appimage[0], appimage[0]]);
if (!found.length) {
  console.error(`No ${version} bundles found under ${bundle}. Build first: cd desktop && pnpm tauri build`);
  process.exit(1);
}

fs.mkdirSync(out, { recursive: true });
const latestPath = path.join(out, 'latest.json');
let latest = fs.existsSync(latestPath) ? JSON.parse(fs.readFileSync(latestPath, 'utf8')) : {};
if (latest.version !== version) latest = { platforms: {}, downloads: {} }; // a new version starts clean
latest.version = version;
latest.notes = args.notes || latest.notes || `MultiParity ${version}`;
latest.pub_date = new Date().toISOString();
latest.platforms ||= {};
latest.downloads ||= {};
latest.sizes ||= {};
const base = typeof args['base-url'] === 'string' ? args['base-url'].replace(/\/+$/, '') : '';
const at = (name) => (base ? `${base}/${name}` : name);

const copy = (from, name) => { fs.copyFileSync(from, path.join(out, name)); return name; };
for (const [platform, upd, dl] of found) {
  const sigFile = `${upd}.sig`;
  if (!fs.existsSync(sigFile)) {
    console.error(`${path.basename(upd)} has no .sig: build with TAURI_SIGNING_PRIVATE_KEY set so updates can be verified.`);
    process.exit(1);
  }
  // macOS names the update bundle without the version: give it one, and the architecture.
  const name = path.basename(upd).endsWith('.app.tar.gz') ? `MultiParity_${version}_${arch}.app.tar.gz` : path.basename(upd);
  latest.platforms[platform] = { url: at(copy(upd, name)), signature: fs.readFileSync(sigFile, 'utf8').trim() };
  latest.sizes[name] = fs.statSync(upd).size;
  if (dl) {
    const dlName = dl === upd ? name : copy(dl, path.basename(dl));
    latest.downloads[platform] = at(dlName);
    latest.sizes[dlName] = fs.statSync(dl).size;
  }
  console.log(`${platform}: ${name}${dl && dl !== upd ? ` (download ${path.basename(dl)})` : ''}`);
}
fs.writeFileSync(latestPath, JSON.stringify(latest, null, 2) + '\n');
// Files of older versions are no longer named; remove them.
const keep = new Set(['latest.json', ...[...Object.values(latest.platforms).map((p) => p.url), ...Object.values(latest.downloads)].map((u) => u.split('/').pop())]);
for (const f of fs.readdirSync(out)) if (!keep.has(f) && fs.statSync(path.join(out, f)).isFile()) fs.unlinkSync(path.join(out, f));
console.log(`Wrote ${latestPath} for ${version}: ${Object.keys(latest.platforms).join(', ')}`);
