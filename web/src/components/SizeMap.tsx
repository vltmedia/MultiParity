// The size map of a project folder (like WinDirStat or GrandPerspective): every file is a block
// whose area is its size, framed and labelled by its folders. Squarified treemap laid out in a
// 1600 x 900 space and drawn as blocks placed in percentages, so text stays sharp at any width.
// The desktop app has the same map (desktop/ui/treemap.js).
import type { FileEntry } from '../api/types';
import { size as human } from '../lib/format';

const W = 1600;
const H = 900;
const TYPES: Record<string, [string, string]> = {
  image: ['Images', '#4f9dde'], '3d': ['3D scenes', '#e0884a'], audio: ['Audio', '#6fbf5a'], video: ['Video', '#b862c9'],
  code: ['Code', '#d9b843'], text: ['Text', '#8aa0b8'], other: ['Other', '#6d7c8e'],
};
const KINDS: Array<[RegExp, string]> = [
  [/\.(png|jpe?g|tga|tiff?|exr|hdr|psd|kra|webp|gif|bmp|dds|ktx2?)$/i, 'image'],
  [/\.(wav|mp3|ogg|flac|aiff?|bank|wem)$/i, 'audio'],
  [/\.(mp4|mov|avi|mkv|webm|prores)$/i, 'video'],
  [/\.(blend1?|fbx|obj|abc|usd[acz]?|gltf|glb|ma|mb|max|c4d|hip|ztl|spp|uasset|umap|prefab|unity)$/i, '3d'],
  [/\.(cs|cpp|c|h|hpp|py|js|ts|rs|lua|gd|json|ya?ml|toml|ini|cfg|shader|hlsl|glsl)$/i, 'code'],
  [/\.(md|txt|pdf|docx?|rtf|csv)$/i, 'text'],
];
export const typeOf = (path: string) => KINDS.find(([re]) => re.test(path))?.[1] || 'other';

type Node = { name: string; path: string; size: number; kids?: Map<string, Node>; file?: FileEntry; more?: number };
type Block = { kind: 'folder' | 'file' | 'more'; node: Node; x: number; y: number; w: number; h: number; label?: boolean; depth?: number };

function tree(list: FileEntry[], base: string): Node {
  const root: Node = { name: '', path: base.replace(/\/$/, ''), size: 0, kids: new Map() };
  for (const f of list) {
    if (!f.path.startsWith(base) || !(f.size > 0)) continue;
    const parts = f.path.slice(base.length).split('/');
    let n = root;
    n.size += f.size;
    for (let i = 0; i < parts.length - 1; i++) {
      const p = (n.path ? n.path + '/' : '') + parts[i];
      if (!n.kids!.has(parts[i])) n.kids!.set(parts[i], { name: parts[i], path: p, size: 0, kids: new Map() });
      n = n.kids!.get(parts[i])!;
      n.size += f.size;
    }
    n.kids!.set('\u0000' + parts[parts.length - 1], { name: parts[parts.length - 1], path: f.path, size: f.size, file: f });
  }
  return root;
}

function squarify(items: Node[], x: number, y: number, w: number, h: number) {
  const total = items.reduce((a, it) => a + it.size, 0);
  const out: Array<{ it: Node; x: number; y: number; w: number; h: number }> = [];
  if (!total || w <= 0 || h <= 0) return out;
  const scale = (w * h) / total;
  let r = { x, y, w, h };
  let row: Array<{ it: Node; area: number }> = [];
  const worst = (areas: number[], side: number) => {
    const s = areas.reduce((a, b) => a + b, 0);
    return Math.max((side * side * Math.max(...areas)) / (s * s), (s * s) / (side * side * Math.min(...areas)));
  };
  const place = () => {
    const s = row.reduce((a, q) => a + q.area, 0);
    if (r.w >= r.h) {
      const cw = s / r.h;
      let yy = r.y;
      for (const q of row) { const hh = q.area / cw; out.push({ it: q.it, x: r.x, y: yy, w: cw, h: hh }); yy += hh; }
      r = { x: r.x + cw, y: r.y, w: r.w - cw, h: r.h };
    } else {
      const rh = s / r.w;
      let xx = r.x;
      for (const q of row) { const ww = q.area / rh; out.push({ it: q.it, x: xx, y: r.y, w: ww, h: rh }); xx += ww; }
      r = { x: r.x, y: r.y + rh, w: r.w, h: r.h - rh };
    }
    row = [];
  };
  for (const it of items) {
    const a = { it, area: it.size * scale };
    const side = Math.min(r.w, r.h);
    if (!row.length || worst([...row.map((q) => q.area), a.area], side) <= worst(row.map((q) => q.area), side)) row.push(a);
    else { place(); row.push(a); }
  }
  if (row.length) place();
  return out;
}

function layout(root: Node): Block[] {
  const blocks: Block[] = [];
  let budget = 5000;
  const walk = (node: Node, x: number, y: number, w: number, h: number, depth: number) => {
    const kids = [...node.kids!.values()].sort((a, b) => b.size - a.size);
    const scale = (w * h) / (node.size || 1);
    const big = kids.filter((k) => k.size * scale >= 18);
    let items = kids;
    if (big.length < kids.length || big.length > budget) {
      const keep = big.slice(0, Math.max(0, budget));
      const rest = kids.filter((k) => !keep.includes(k));
      items = [...keep, ...(rest.length ? [{ name: `${rest.length} smaller`, path: node.path, size: rest.reduce((a, k) => a + k.size, 0), more: rest.length }] : [])];
    }
    for (const b of squarify(items, x, y, w, h)) {
      const k = b.it;
      if (k.kids) {
        const label = b.w > 90 && b.h > 46;
        blocks.push({ kind: 'folder', node: k, x: b.x, y: b.y, w: b.w, h: b.h, label, depth });
        const pad = b.w > 12 && b.h > 12 ? 3 : 0;
        const head = label ? 22 : 0;
        if (b.w - 2 * pad > 2 && b.h - 2 * pad - head > 2) walk(k, b.x + pad, b.y + pad + head, b.w - 2 * pad, b.h - 2 * pad - head, depth + 1);
      } else {
        budget--;
        blocks.push({ kind: k.more ? 'more' : 'file', node: k, x: b.x, y: b.y, w: b.w, h: b.h });
      }
    }
  };
  walk(root, 0, 0, W, H, 0);
  return blocks;
}

export function SizeMap({ files, base, onFolder, onFile }: { files: FileEntry[]; base: string; onFolder: (path: string) => void; onFile: (f: FileEntry) => void }) {
  const root = tree(files, base);
  if (!root.size) return <div className="empty"><b>Nothing to measure here.</b> Files of zero bytes take no space.</div>;
  const pct = (v: number, t: number) => `${((v / t) * 100).toFixed(3)}%`;
  const share = (n: number) => `${((n / root.size) * 100).toFixed(n / root.size < 0.01 ? 2 : 1)}%`;
  const byType: Record<string, number> = {};
  for (const f of files) if (f.path.startsWith(base)) byType[typeOf(f.path)] = (byType[typeOf(f.path)] || 0) + (f.size || 0);
  return (
    <>
      <div className="tmap" role="img" aria-label={`Size map of ${root.path || 'the project'}: ${human(root.size)}`}>
        {layout(root).map((b, i) => {
          const style = { left: pct(b.x, W), top: pct(b.y, H), width: pct(b.w, W), height: pct(b.h, H) };
          if (b.kind === 'folder') {
            return (
              <div key={i} className={`tm-dir d${Math.min(b.depth || 0, 3)}`} style={style} title={`${b.node.path}/ · ${human(b.node.size)} · ${share(b.node.size)}`} onClick={() => onFolder(b.node.path)}>
                {b.label && <span>{b.node.name} <i>{human(b.node.size)}</i></span>}
              </div>
            );
          }
          if (b.kind === 'more') return <div key={i} className="tm-file more" style={style} title={`${b.node.more} smaller files · ${human(b.node.size)}`} />;
          const t = typeOf(b.node.path);
          return (
            <div key={i} className="tm-file" style={{ ...style, ['--c' as string]: TYPES[t][1] }} title={`${b.node.path} · ${human(b.node.size)} · ${share(b.node.size)}`} onClick={() => onFile(b.node.file!)}>
              {b.w > 110 && b.h > 34 && <span>{b.node.name}<i>{human(b.node.size)}</i></span>}
            </div>
          );
        })}
      </div>
      <ul className="tm-legend">
        {Object.entries(byType).sort((a, b) => b[1] - a[1]).map(([t, s]) => (
          <li key={t}><i style={{ ['--c' as string]: TYPES[t][1] }} />{TYPES[t][0]} <b>{human(s)}</b> <small>{((s / root.size) * 100).toFixed(0)}%</small></li>
        ))}
      </ul>
    </>
  );
}
