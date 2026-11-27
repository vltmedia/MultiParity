// Copy the repository's docs/*.md into the site (site/docs), so the docs are written in one place
// and the site always shows what is in the repository. Each page gets a title for the sidebar
// from its first heading.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const from = path.resolve(here, '../../docs');
const to = path.resolve(here, '../docs');
fs.rmSync(to, { recursive: true, force: true });
fs.mkdirSync(to, { recursive: true });

const REPO = 'https://github.com/vltmedia/MultiParity/blob/main';
let n = 0;
for (const f of fs.readdirSync(from).filter((f) => f.endsWith('.md'))) {
  let s = fs.readFileSync(path.join(from, f), 'utf8');
  // Links to repository files outside docs/ (../README.md, ../scripts/...) go to GitHub.
  s = s.replace(/\]\((\.\.\/[^)#\s]+)(#[^)]*)?\)/g, (_, p, h = '') => `](${REPO}/${p.replace(/^\.\.\//, '')}${h})`);
  fs.writeFileSync(path.join(to, f), s);
  n++;
}
console.log(`Copied ${n} docs into site/docs.`);
