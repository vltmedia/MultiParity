// MultiParity desktop: the project tabs beside Your work. Files (claims, older versions),
// History, Branches and Drafts. Server calls go through the daemon with the folder's account
// (/local/workspaces/{id}/server/...), so no sign-in token is handled here. Uses helpers from
// app.js and work.js.
"use strict";

const TABS = [["work", "Your work"], ["files", "Files"], ["history", "History"], ["branches", "Branches"], ["drafts", "Drafts"]];

var proj = {
  prefix: "",
  q: "",
  files: null,
  history: null,
  historyMore: true,
  open: {}, // change number -> detail, for expanded history rows
  branches: null,
  drafts: null,
  error: null,
  loading: false,
  reset() {
    Object.assign(this, { prefix: "", q: "", files: null, history: null, historyMore: true, open: {}, branches: null, drafts: null, error: null, loading: false });
  },
};

const srv = (path, opts) => api(`/workspaces/${work.wsId}/server/${path}`, opts);
const srvPost = (path, body) => srv(path, { method: "POST", body: JSON.stringify(body || {}) });
const isMe = (u) => !!u && u.email === work.ws.account_email;
const firstName = (n) => String(n || "someone").split(" ")[0];
const sep = () => (work.ws.root.includes("\\") ? "\\" : "/");
const absPath = (rel) => work.ws.root + sep() + rel.split("/").join(sep());

// ------------------------------------------------------------------ tabs

function tabsHtml() {
  const unpublished = files().length;
  const drafts = proj.drafts ? proj.drafts.length : 0;
  const badge = (k) => (k === "work" && unpublished ? `<b>${unpublished}</b>` : k === "drafts" && drafts ? `<b>${drafts}</b>` : "");
  return `<nav class="tabs" role="tablist" aria-label="${esc(work.project.name)}">${TABS.map(([k, label]) =>
    `<button role="tab" data-tab="${k}" aria-selected="${work.tab === k}">${label}${badge(k)}</button>`).join("")}</nav>`;
}

function tabHtml() {
  if (proj.error) return `<div class="notice bad">${esc(proj.error)} <button class="link" data-p="reload">Try again</button></div>`;
  switch (work.tab) {
    case "files": return filesHtml();
    case "history": return historyHtml();
    case "branches": return branchesHtml();
    case "drafts": return draftsHtml();
  }
  return "";
}

async function loadTab() {
  proj.error = null;
  proj.loading = true;
  renderWork();
  try {
    if (work.tab === "files") {
      const q = new URLSearchParams();
      if (proj.q) q.set("q", proj.q);
      else if (proj.prefix) q.set("prefix", proj.prefix + "/");
      proj.files = await srv(`branches/${work.ws.branch_id}/files?${q}`);
    } else if (work.tab === "history") {
      const list = await srv(`changes?branch=${work.ws.branch_id}&limit=40`);
      proj.history = list;
      proj.historyMore = list.length === 40;
      proj.open = {};
    } else if (work.tab === "branches") {
      proj.branches = await srv("branches");
    } else if (work.tab === "drafts") {
      proj.drafts = await srv("drafts");
      if (!proj.branches) proj.branches = await srv("branches").catch(() => null);
    }
  } catch (e) {
    proj.error = e.message;
  }
  proj.loading = false;
  renderWork();
}

function switchTab(t) {
  if (work.tab === t) return;
  work.tab = t;
  if (t === "work") { renderWork(); refreshWork(); return; }
  loadTab();
  refreshWork(false);
}

// ------------------------------------------------------------------ files

function fileState(f) {
  const st = work.status;
  if (st && st.deleted.some((x) => x.path === f.path)) return ["deleted", "Deleted here"];
  if (st && st.edited.some((x) => x.path === f.path)) return ["edited", "Edited here"];
  if (f.claim && isMe(f.claim.user)) return ["mine", "Claimed by you"];
  if (f.claim) return ["theirs", `Claimed by ${firstName(f.claim.user && f.claim.user.name)}`];
  if (f.change > work.ws.synced_change) return ["behind", "Newer available"];
  return ["ok", "Up to date"];
}

function filesHtml() {
  if (!proj.files) return `<div class="empty">${proj.loading ? "Loading files…" : ""}</div>`;
  const base = proj.prefix ? proj.prefix + "/" : "";
  const folders = new Map();
  const rows = [];
  for (const f of proj.files) {
    const rest = f.path.startsWith(base) ? f.path.slice(base.length) : f.path;
    const i = rest.indexOf("/");
    if (i >= 0 && !proj.q) {
      const name = rest.slice(0, i);
      const g = folders.get(name) || { name, files: 0, size: 0, claims: 0 };
      g.files++; g.size += f.size; if (f.claim) g.claims++;
      folders.set(name, g);
    } else rows.push(f);
  }
  rows.sort((a, b) => baseName(a.path).localeCompare(baseName(b.path)));
  const crumbs = proj.prefix ? proj.prefix.split("/") : [];
  const mineCount = proj.files.filter((f) => f.claim && isMe(f.claim.user)).length;
  return `
    <div class="ftools">
      <div class="crumbs">
        <button class="link" data-p="crumb" data-to="">${esc(work.project.name)}</button>
        ${crumbs.map((c, i) => `<span>/</span>${i === crumbs.length - 1 ? `<b>${esc(c)}</b>` : `<button class="link" data-p="crumb" data-to="${esc(crumbs.slice(0, i + 1).join("/"))}">${esc(c)}</button>`}`).join("")}
      </div>
      <label class="search sm"><span class="sr">Find a file</span><input id="p-q" type="search" placeholder="Find a file by name" value="${esc(proj.q)}"></label>
    </div>
    <p class="note">${proj.q ? `${plural(rows.length, "file")} matching “${esc(proj.q)}”` : `${plural(proj.files.length, "file")} on ${esc(work.ws.branch_name)}`}${mineCount ? `, ${plural(mineCount, "claim")} yours` : ""}. Right-click a file to claim it, release it, or see its older versions.</p>
    ${!folders.size && !rows.length ? `<div class="empty"><b>${proj.q ? "No files match." : "This folder is empty."}</b>${proj.q ? "Try another name." : `Save files into ${esc(work.ws.root)} and publish them.`}</div>` : ""}
    <ul class="flist" role="list">
      ${[...folders.values()].sort((a, b) => a.name.localeCompare(b.name)).map((g) => `
        <li class="folder" data-p="open-folder" data-name="${esc(g.name)}" tabindex="0">
          <span class="ficon" aria-hidden="true"></span>
          <div class="name"><strong>${esc(g.name)}</strong><small>${plural(g.files, "file")}${g.claims ? `, ${plural(g.claims, "claim")}` : ""}</small></div>
          <span class="meta">${human(g.size)}</span><span></span><span></span>
        </li>`).join("")}
      ${rows.map((f) => {
        const [cls, label] = fileState(f);
        return `<li data-path="${esc(f.path)}" tabindex="0">
          <span class="ficon file" aria-hidden="true"></span>
          <div class="name"><strong>${esc(proj.q ? f.path : baseName(f.path))}</strong><small>Change ${f.change}${f.tags && f.tags.length ? " · " + f.tags.map((t) => esc(t.name)).join(", ") : ""}</small></div>
          <span class="meta">${human(f.size)}</span>
          <span class="st ${cls}">${esc(label)}</span>
          <button class="more" data-p="file-menu" aria-label="More for ${esc(baseName(f.path))}" aria-haspopup="menu">···</button>
        </li>`;
      }).join("")}
    </ul>`;
}

function fileMenu(path, x, y, li) {
  const f = (proj.files || []).find((z) => z.path === path);
  if (!f) return;
  const items = [];
  if (!f.claim) items.push(["Claim", () => claimFiles([path])]);
  else if (isMe(f.claim.user)) items.push(["Release", () => releaseFiles([path])]);
  else items.push([`Ask ${firstName(f.claim.user && f.claim.user.name)} to release`, () => nudge(f)]);
  items.push(["Older versions…", () => versionsDialog(path)]);
  if (tauri) {
    items.push(["Open", () => invoke("open_path", { path: absPath(path) })]);
    items.push(["Show in folder", () => invoke("reveal_path", { path: absPath(path) })]);
  }
  showMenu(items, x, y, li);
}

async function claimFiles(paths) {
  try {
    const r = await srvPost("claims", { branch: work.ws.branch_id, paths, device: "Desktop app" });
    if (r.held && r.held.length) toast(`${baseName(r.held[0].path)} is claimed by ${firstName(r.held[0].user && r.held[0].user.name)}.`, true);
    else toast(`Claimed ${paths.length === 1 ? baseName(paths[0]) : plural(paths.length, "file")}. Nobody else can publish ${paths.length === 1 ? "it" : "them"} until you publish or release.`);
  } catch (e) { toast(e.message, true); }
  loadTab();
}

async function releaseFiles(paths) {
  try {
    await srvPost("claims/release", { branch: work.ws.branch_id, paths });
    toast(`Released ${paths.length === 1 ? baseName(paths[0]) : plural(paths.length, "file")}.`);
  } catch (e) { toast(e.message, true); }
  loadTab();
}

async function nudge(f) {
  try {
    await srvPost("claims/nudge", { branch: work.ws.branch_id, path: f.path });
    toast(`Asked ${firstName(f.claim.user && f.claim.user.name)} to release ${baseName(f.path)}.`);
  } catch (e) { toast(e.message, true); }
}

// ------------------------------------------------------------------ history

function historyHtml() {
  if (!proj.history) return `<div class="empty">${proj.loading ? "Loading history…" : ""}</div>`;
  if (!proj.history.length) return `<div class="empty"><b>Nothing published on ${esc(work.ws.branch_name)} yet.</b>Publish your first files from Your work.</div>`;
  return `<ul class="hist">${proj.history.map((c) => {
    const d = proj.open[c.number];
    return `<li class="${d ? "open" : ""}">
      <button class="hrow" data-p="change" data-n="${c.number}" aria-expanded="${!!d}">
        <span class="num">${c.number}</span>
        <span class="what"><strong>${esc(c.message)}</strong><small>${esc((c.author && c.author.name) || "Someone")} · ${esc(ago(c.created_at))} · ${plural(c.file_count, "file")}, ${human(c.size)}${c.kind !== "publish" ? ` · ${c.kind === "merge" ? "merge" : "update from parent"}` : ""}</small></span>
      </button>
      ${d ? `<ul class="hfiles">${(d.files || []).map((f) => `<li><span class="tag ${f.op === "add" ? "new" : f.op === "delete" ? "deleted" : "edited"}">${f.op === "add" ? "Added" : f.op === "delete" ? "Deleted" : "Edited"}</span>
        <span class="hpath">${esc(f.path)}</span><span class="meta">${f.op === "delete" ? "" : human(f.size)}</span>
        ${f.op === "delete" ? "<span></span>" : `<button class="link" data-p="versions" data-path="${esc(f.path)}">Older versions</button>`}</li>`).join("")}</ul>` : ""}
    </li>`;
  }).join("")}</ul>
  ${proj.historyMore ? `<button class="btn sm more-hist" data-p="more-history">Show older changes</button>` : ""}`;
}

async function toggleChange(n) {
  if (proj.open[n]) { delete proj.open[n]; renderWork(); return; }
  try { proj.open[n] = await srv(`changes/${n}`); } catch (e) { toast(e.message, true); }
  renderWork();
}

async function moreHistory() {
  const last = proj.history[proj.history.length - 1];
  try {
    const list = await srv(`changes?branch=${work.ws.branch_id}&limit=40&before=${last.number}`);
    proj.history = proj.history.concat(list);
    proj.historyMore = list.length === 40;
  } catch (e) { toast(e.message, true); }
  renderWork();
}

// ------------------------------------------------------------------ branches

function branchesHtml() {
  if (!proj.branches) return `<div class="empty">${proj.loading ? "Loading branches…" : ""}</div>`;
  const live = proj.branches.filter((b) => !b.archived);
  const byParent = new Map();
  for (const b of live) { const k = b.parent_id || ""; byParent.set(k, [...(byParent.get(k) || []), b]); }
  const order = [];
  (function walk(parent, depth) { for (const b of (byParent.get(parent) || []).sort((x, y) => x.name.localeCompare(y.name))) { order.push([b, depth]); walk(b.id, depth + 1); } })("", 0);
  const nameOf = (id) => (live.find((x) => x.id === id) || {}).name || "its parent";
  const dirty = files().length;
  return `
    <div class="ftools"><p class="note">Your folder is on <b>${esc(work.ws.branch_name)}</b>. Switching swaps the files in the folder to that branch's versions; publish or save a draft first.</p>
      <button class="btn sm" data-p="new-branch">New branch…</button></div>
    <ul class="blist">${order.map(([b, depth]) => {
      const here = b.id === work.ws.branch_id;
      return `<li class="${here ? "here" : ""}" style="--depth:${depth}">
        <div class="name"><strong>${esc(b.name)}${here ? ` <em>Your folder</em>` : ""}${b.protected ? ` <em class="prot">Protected</em>` : ""}</strong>
          <small>${b.parent_id ? `From ${esc(nameOf(b.parent_id))} · ${plural(b.ahead, "file")} to merge back · ${b.behind ? `${plural(b.behind, "new file")} on ${esc(nameOf(b.parent_id))}` : "up to date with it"}` : `Main line · change ${b.head_change}`}${b.shares_claims ? "" : " · own claims"}</small></div>
        <div class="acts">
          ${here ? `${b.parent_id && b.behind ? `<button class="btn sm" data-p="update" data-id="${b.id}">Bring in ${esc(nameOf(b.parent_id))}'s changes</button>` : ""}
                   ${b.parent_id ? `<button class="btn sm" data-p="merge" data-id="${b.id}">Merge into ${esc(nameOf(b.parent_id))}…</button>` : ""}`
                 : `<button class="btn sm" data-p="switch" data-id="${b.id}" ${dirty ? `disabled title="Publish or save a draft of your ${plural(dirty, "unpublished file")} first."` : ""}>Switch here</button>`}
        </div>
      </li>`;
    }).join("")}</ul>`;
}

async function switchBranch(id) {
  const b = proj.branches.find((x) => x.id === id);
  await run("switch", async () => {
    try {
      const r = await api(`/workspaces/${work.wsId}/branch`, { method: "POST", body: JSON.stringify({ branch_id: id }) });
      Object.assign(work.ws, { branch_id: r.workspace.branch_id, branch_name: r.workspace.branch_name, synced_change: r.workspace.synced_change });
      toast(`Your folder is on ${b.name} now. ${plural(r.downloaded, "file")} changed${r.removed ? `, ${r.removed} removed` : ""}.`);
      proj.files = null; proj.history = null;
      load();
    } catch (e) { toast(e.message, true); }
  });
  loadTab();
}

function newBranchDialog() {
  const live = proj.branches.filter((b) => !b.archived);
  dialog(`<h2>New branch</h2>
    <p class="note">A branch is a separate line of work: publish to it without touching the others, then merge it back.</p>
    <label class="field">Name<input id="nb-name" placeholder="hero-redesign" autofocus></label>
    <label class="field">Start from<select id="nb-from">${live.map((b) => `<option value="${b.id}" ${b.id === work.ws.branch_id ? "selected" : ""}>${esc(b.name)}</option>`).join("")}</select></label>
    <label class="check"><input type="checkbox" id="nb-own"> Its own claims (people can claim the same file here and on the parent)</label>
    <label class="check"><input type="checkbox" id="nb-switch" checked ${files().length ? "disabled" : ""}> Switch my folder to it${files().length ? " (publish or save a draft first)" : ""}</label>
    <p class="err" id="nb-err" role="alert"></p>
    <div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn pri" value="go">Make branch</button></div>`,
    async (_, body, dlg) => {
      const name = $("#nb-name", body).value.trim();
      if (!name) { $("#nb-err", body).textContent = "Give the branch a name."; return; }
      try {
        const b = await srvPost("branches", { name, from: $("#nb-from", body).value, shares_claims: !$("#nb-own", body).checked });
        dlg.close();
        toast(`Made the branch ${b.name}.`);
        proj.branches = await srv("branches");
        if ($("#nb-switch", body).checked && !files().length) return switchBranch(b.id);
        renderWork();
      } catch (e) { $("#nb-err", body).textContent = e.message; }
    });
}

async function updateBranch(id, resolutions) {
  let r;
  try { r = await srvPost(`branches/${id}/update`, resolutions ? { resolutions } : {}); }
  catch (e) { toast(e.message, true); return; }
  if (r.conflicts && r.conflicts.length) return conflictDialog(id, r);
  await getLatest();
  toast(r.change ? `Brought in ${plural(r.only_parent, "file")}${r.auto_merged.length ? `, ${r.auto_merged.length} merged automatically` : ""}, and got them.` : "Already up to date with the parent branch.");
  proj.branches = await srv("branches").catch(() => proj.branches);
  renderWork();
}

function conflictDialog(id, r) {
  dialog(`<h2>${plural(r.conflicts.length, "file")} changed on both branches</h2>
    <p class="note">Choose which version to keep for each. Nothing has changed yet.</p>
    <ul class="pick">${r.conflicts.map((c, i) => `<li class="clash"><span>${esc(c.path)}</span>
      <label><input type="radio" name="c${i}" value="mine" checked> This branch's</label>
      <label><input type="radio" name="c${i}" value="parent"> The parent's</label></li>`).join("")}</ul>
    <p class="err" id="u-err" role="alert"></p>
    <div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn pri" value="go">Bring in the changes</button></div>`,
    async (_, body, dlg) => {
      const res = {};
      r.conflicts.forEach((c, i) => { res[c.path] = body.querySelector(`input[name="c${i}"]:checked`).value; });
      dlg.close();
      await updateBranch(id, res);
    });
}

async function mergeBranch(id) {
  const b = proj.branches.find((x) => x.id === id);
  const parent = proj.branches.find((x) => x.id === b.parent_id);
  let p;
  try { p = await srv(`branches/${id}/merge-preview`); } catch (e) { toast(e.message, true); return; }
  if (!p.up_to_date) { toast(`Bring in ${parent.name}'s changes first, so the merge cannot clash.`, true); return; }
  if (!p.only_here) { toast(`${b.name} has nothing ${parent.name} does not have.`); return; }
  dialog(`<h2>Merge ${esc(b.name)} into ${esc(parent.name)}</h2>
    <p class="note">${plural(p.only_here, "file")} arrive on ${esc(parent.name)} as one change.${parent.protected ? ` ${esc(parent.name)} is protected: only merges reach it.` : ""}</p>
    <label class="field">Say what this brings<input id="m-msg" value="${esc(`Merge ${b.name}`)}"></label>
    <p class="err" id="m-err" role="alert"></p>
    <div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn pri" value="go">Merge</button></div>`,
    async (_, body, dlg) => {
      try {
        const r = await srvPost(`branches/${id}/merge`, { message: $("#m-msg", body).value.trim() || `Merge ${b.name}` });
        dlg.close();
        toast(`Merged ${b.name} into ${parent.name} as change ${r.change.number}.`);
        proj.branches = await srv("branches");
        renderWork();
      } catch (e) { $("#m-err", body).textContent = e.message; }
    });
}

// ------------------------------------------------------------------ drafts

function draftsHtml() {
  if (!proj.drafts) return `<div class="empty">${proj.loading ? "Loading drafts…" : ""}</div>`;
  const nameOf = (id) => ((proj.branches || []).find((b) => b.id === id) || {}).name || "another branch";
  const n = files().length;
  return `
    <div class="ftools"><p class="note">Drafts keep unfinished work on the server where only you see it, so you can pick it up on another computer or switch branches without publishing.</p>
      <button class="btn sm" data-p="save-draft" ${n ? "" : "disabled"}>Save a draft${n ? ` of ${plural(n, "file")}` : ""}…</button></div>
    ${proj.drafts.length ? `<ul class="cplist">${proj.drafts.map((d) => `<li>
      <div class="when"><strong>${esc(ago(d.created_at))}</strong><small>${esc(new Date(d.created_at).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }))}</small></div>
      <div><strong>${esc(d.title)}</strong><small>${plural(d.file_count, "file")}, ${human(d.size)}${d.branch_id !== work.ws.branch_id ? ` · from ${esc(nameOf(d.branch_id))}` : ""}</small></div>
      <div class="acts"><button class="btn sm" data-p="pickup" data-id="${d.id}">Pick up…</button><button class="btn sm" data-p="drop-draft" data-id="${d.id}">Delete</button></div>
    </li>`).join("")}</ul>` : `<div class="empty"><b>No drafts.</b>Save one when you need to stop half-way.</div>`}`;
}

function saveDraftDialog() {
  dialog(`<h2>Save a draft</h2>
    <p class="note">Your ${plural(files().length, "unpublished file")} go to the server, visible only to you. They stay in your folder too.</p>
    <label class="field">Title<input id="dr-title" placeholder="Hero cape, half done" autofocus></label>
    <p class="err" id="dr-err" role="alert"></p>
    <div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn pri" value="go">Save draft</button></div>`,
    async (_, body, dlg) => {
      const title = $("#dr-title", body).value.trim();
      if (!title) { $("#dr-err", body).textContent = "Give the draft a title."; return; }
      body.querySelectorAll("button").forEach((b) => (b.disabled = true));
      try {
        await api(`/workspaces/${work.wsId}/draft`, { method: "POST", body: JSON.stringify({ title }) });
        dlg.close();
        toast(`Saved the draft “${title}”.`);
        loadTab();
      } catch (e) {
        body.querySelectorAll("button").forEach((b) => (b.disabled = false));
        $("#dr-err", body).textContent = e.message;
      }
    });
}

function pickupDialog(id) {
  const d = proj.drafts.find((x) => x.id === id);
  dialog(`<h2>Pick up “${esc(d.title)}”</h2>
    <p>Its ${plural(d.file_count, "file")} are written into your folder. Your current unpublished work is saved in a checkpoint first, so you can rewind if you need it back.</p>
    <p class="err" id="pk-err" role="alert"></p>
    <div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn pri" value="go">Pick up the draft</button></div>`,
    async (_, body, dlg) => {
      try {
        const r = await api(`/workspaces/${work.wsId}/drafts/${id}/restore`, { method: "POST", body: "{}" });
        dlg.close();
        toast(`Picked up ${plural(r.files, "file")} from “${d.title}”. They are in Your work.`);
        switchTab("work");
      } catch (e) { $("#pk-err", body).textContent = e.message; }
    });
}

async function dropDraft(id) {
  const d = proj.drafts.find((x) => x.id === id);
  if (!confirm(`Delete the draft “${d.title}”? Files in your folder are not touched.`)) return;
  try { await srv(`drafts/${id}`, { method: "DELETE" }); toast(`Deleted the draft “${d.title}”.`); }
  catch (e) { toast(e.message, true); }
  loadTab();
}

// ------------------------------------------------------------------ wiring

$("#v-work").addEventListener("click", (e) => {
  const tab = e.target.closest("[data-tab]");
  if (tab) return switchTab(tab.dataset.tab);
  const el = e.target.closest("[data-p]");
  if (!el) return;
  const what = el.dataset.p;
  if (what === "reload") return loadTab();
  if (what === "crumb") { proj.prefix = el.dataset.to; proj.q = ""; return loadTab(); }
  if (what === "open-folder") { proj.prefix = (proj.prefix ? proj.prefix + "/" : "") + el.dataset.name; return loadTab(); }
  if (what === "file-menu") { e.stopPropagation(); const li = el.closest("li[data-path]"); const r = el.getBoundingClientRect(); return fileMenu(li.dataset.path, r.left, r.bottom + 4, li); }
  if (what === "change") return toggleChange(Number(el.dataset.n));
  if (what === "versions") return versionsDialog(el.dataset.path);
  if (what === "more-history") return moreHistory();
  if (what === "new-branch") return newBranchDialog();
  if (what === "switch") return switchBranch(el.dataset.id);
  if (what === "update") return updateBranch(el.dataset.id);
  if (what === "merge") return mergeBranch(el.dataset.id);
  if (what === "save-draft") return saveDraftDialog();
  if (what === "pickup") return pickupDialog(el.dataset.id);
  if (what === "drop-draft") return dropDraft(el.dataset.id);
});

$("#v-work").addEventListener("keydown", (e) => {
  if (e.target.id === "p-q" && e.key === "Enter") { proj.q = e.target.value.trim(); loadTab(); return; }
  const folder = e.target.closest && e.target.closest("li.folder");
  if (folder && e.target === folder && e.key === "Enter") { proj.prefix = (proj.prefix ? proj.prefix + "/" : "") + folder.dataset.name; loadTab(); return; }
  const li = e.target.closest && e.target.closest(".flist li[data-path]");
  if (li && e.target === li && (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10"))) { e.preventDefault(); const r = li.getBoundingClientRect(); fileMenu(li.dataset.path, r.left + 40, r.bottom, li); }
});

$("#v-work").addEventListener("search", (e) => { if (e.target.id === "p-q" && !e.target.value) { proj.q = ""; loadTab(); } });

$("#v-work").addEventListener("contextmenu", (e) => {
  const li = e.target.closest(".flist li[data-path]");
  if (!li) return;
  e.preventDefault();
  fileMenu(li.dataset.path, e.clientX, e.clientY, li);
});
