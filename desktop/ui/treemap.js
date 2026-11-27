// MultiParity desktop: the size map of a project folder (like WinDirStat or GrandPerspective).
// Every file is a block whose area is its size; folders are framed and labelled around their
// files. Laid out with the squarified treemap algorithm in a 1600 x 900 space and drawn as
// absolutely placed blocks in percentages, so text stays sharp at any window size.
"use strict";

const TM_W = 1600, TM_H = 900;
const TM_TYPES = {
  image: ["Images", "#4f9dde"], "3d": ["3D scenes", "#e0884a"], audio: ["Audio", "#6fbf5a"], video: ["Video", "#b862c9"],
  code: ["Code", "#d9b843"], text: ["Text", "#8aa0b8"], other: ["Other", "#6d7c8e"],
};

/** A tree of folders and files under `base` (a path ending in "/", or ""). */
function tmTree(list, base) {
  const root = { name: "", path: base.replace(/\/$/, ""), size: 0, kids: new Map() };
  for (const f of list) {
    if (!f.path.startsWith(base) || !(f.size > 0)) continue;
    const parts = f.path.slice(base.length).split("/");
    let n = root;
    n.size += f.size;
    for (let i = 0; i < parts.length - 1; i++) {
      const p = (n.path ? n.path + "/" : "") + parts[i];
      if (!n.kids.has(parts[i])) n.kids.set(parts[i], { name: parts[i], path: p, size: 0, kids: new Map() });
      n = n.kids.get(parts[i]);
      n.size += f.size;
    }
    n.kids.set("\u0000" + parts.at(-1), { name: parts.at(-1), path: f.path, size: f.size, file: f });
  }
  return root;
}

/** Squarified layout of `items` (each with .size, biggest first) in a rectangle. */
function tmSquarify(items, x, y, w, h) {
  const total = items.reduce((a, it) => a + it.size, 0);
  if (!total || w <= 0 || h <= 0) return [];
  const scale = (w * h) / total;
  const out = [];
  let r = { x, y, w, h };
  let row = [];
  const worst = (areas, side) => {
    const s = areas.reduce((a, b) => a + b, 0);
    let max = 0, min = Infinity;
    for (const a of areas) { if (a > max) max = a; if (a < min) min = a; }
    return Math.max((side * side * max) / (s * s), (s * s) / (side * side * min));
  };
  const place = () => {
    const s = row.reduce((a, it) => a + it.area, 0);
    if (r.w >= r.h) {
      const cw = s / r.h;
      let yy = r.y;
      for (const it of row) { const hh = it.area / cw; out.push({ it: it.it, x: r.x, y: yy, w: cw, h: hh }); yy += hh; }
      r = { x: r.x + cw, y: r.y, w: r.w - cw, h: r.h };
    } else {
      const rh = s / r.w;
      let xx = r.x;
      for (const it of row) { const ww = it.area / rh; out.push({ it: it.it, x: xx, y: r.y, w: ww, h: rh }); xx += ww; }
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

/** Blocks to draw for a tree: folder frames and file tiles, in the 1600 x 900 space. */
function tmLayout(root) {
  const blocks = [];
  let budget = 5000; // tiles drawn; the rest of a crowded folder is one "smaller files" block
  const walk = (node, x, y, w, h, depth) => {
    const kids = [...node.kids.values()].sort((a, b) => b.size - a.size);
    let items = kids;
    // Tiny things merge, so the map stays readable and quick.
    const minArea = 18;
    const scale = (w * h) / (node.size || 1);
    const big = kids.filter((k) => k.size * scale >= minArea);
    if (big.length < kids.length || big.length > budget) {
      const keep = big.slice(0, Math.max(0, budget));
      const rest = kids.filter((k) => !keep.includes(k));
      items = [...keep, ...(rest.length ? [{ name: `${rest.length} smaller`, path: node.path, size: rest.reduce((a, k) => a + k.size, 0), more: rest.length }] : [])];
    }
    for (const b of tmSquarify(items, x, y, w, h)) {
      const k = b.it;
      if (k.kids) {
        const label = b.w > 90 && b.h > 46;
        blocks.push({ kind: "folder", node: k, x: b.x, y: b.y, w: b.w, h: b.h, label, depth });
        const pad = b.w > 12 && b.h > 12 ? 3 : 0;
        const head = label ? 22 : 0;
        if (b.w - 2 * pad > 2 && b.h - 2 * pad - head > 2) walk(k, b.x + pad, b.y + pad + head, b.w - 2 * pad, b.h - 2 * pad - head, depth + 1);
      } else {
        budget--;
        blocks.push({ kind: k.more ? "more" : "file", node: k, x: b.x, y: b.y, w: b.w, h: b.h });
      }
    }
  };
  walk(root, 0, 0, TM_W, TM_H, 0);
  return blocks;
}

function tmTypeOf(path) {
  return { "file-image": "image", "file-box": "3d", "file-audio": "audio", "file-video": "video", "file-code": "code", "file-text": "text" }[kindIcon(path)] || "other";
}

/** The size map's HTML for the files under `base`. */
function treemapHtml(list, base) {
  const root = tmTree(list, base);
  if (!root.size) return `<div class="empty"><b>Nothing to measure here.</b>Files of zero bytes take no space.</div>`;
  const pct = (v, t) => `${((v / t) * 100).toFixed(3)}%`;
  const byType = {};
  for (const f of list) if (f.path.startsWith(base)) byType[tmTypeOf(f.path)] = (byType[tmTypeOf(f.path)] || 0) + (f.size || 0);
  const blocks = tmLayout(root).map((b) => {
    const style = `left:${pct(b.x, TM_W)};top:${pct(b.y, TM_H)};width:${pct(b.w, TM_W)};height:${pct(b.h, TM_H)}`;
    const share = `${((b.node.size / root.size) * 100).toFixed(b.node.size / root.size < 0.01 ? 2 : 1)}%`;
    if (b.kind === "folder") {
      return `<div class="tm-dir d${Math.min(b.depth, 3)}" style="${style}" data-p="tm-dir" data-to="${esc(b.node.path)}" title="${esc(b.node.path)}/ · ${human(b.node.size)} · ${share}">${b.label ? `<span>${icon("folder")}${esc(b.node.name)} <i>${human(b.node.size)}</i></span>` : ""}</div>`;
    }
    if (b.kind === "more") return `<div class="tm-file more" style="${style}" title="${b.node.more} smaller files · ${human(b.node.size)}"></div>`;
    const t = tmTypeOf(b.node.path);
    const big = b.w > 110 && b.h > 34;
    return `<div class="tm-file" style="${style};--c:${TM_TYPES[t][1]}" data-p="tm-file" data-path="${esc(b.node.path)}" tabindex="-1" title="${esc(b.node.path)} · ${human(b.node.size)} · ${share}">${big ? `<span>${esc(b.node.name)}<i>${human(b.node.size)}</i></span>` : ""}</div>`;
  }).join("");
  const legend = Object.entries(byType).sort((a, b) => b[1] - a[1]).map(([t, s]) =>
    `<li><i style="--c:${TM_TYPES[t][1]}"></i>${TM_TYPES[t][0]} <b>${human(s)}</b> <small>${((s / root.size) * 100).toFixed(0)}%</small></li>`).join("");
  return `<div class="tmap" role="img" aria-label="Size map of ${esc(root.path || "the project")}: ${human(root.size)}">${blocks}</div>
    <ul class="tm-legend">${legend}</ul>`;
}
