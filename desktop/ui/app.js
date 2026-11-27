// MultiParity desktop: the cross-account Projects page. Talks to the local daemon
// (embedded in the app) and to the app shell for folder pickers and server windows.
"use strict";

const tauri = window.__TAURI__ && window.__TAURI__.core;
// Opened in a browser for development or screenshots, ?daemon=<port> points at another daemon
// (the app itself always uses its own, and its security policy allows no other).
const DAEMON_PORT = (!tauri && /^\d{4,5}$/.test(new URLSearchParams(location.search).get("daemon") || "")) ? new URLSearchParams(location.search).get("daemon") : "18519";
const DAEMON = `http://127.0.0.1:${DAEMON_PORT}/local`;
const $ = (s, el = document) => el.querySelector(s);
const esc = (v) => String(v ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const human = (b) => {
  const u = ["B", "KB", "MB", "GB", "TB"]; let i = 0; let v = Number(b) || 0;
  while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
  return i === 0 ? `${v} B` : `${v >= 100 ? v.toFixed(0) : v.toFixed(1)} ${u[i]}`;
};

const state = { accounts: [], groups: [], filter: "all", q: "", open: null, trees: {}, checked: {}, statuses: {} };

async function api(path, opts = {}) {
  const res = await fetch(DAEMON + path, { ...opts, headers: { "content-type": "application/json", ...(opts.headers || {}) } });
  const body = await res.json().catch(() => ({}));
  if (!res.ok || body.error) {
    const err = new Error((body.error && body.error.message) || `The app answered ${res.status}.`);
    err.status = res.status;
    err.details = body.error && body.error.details;
    throw err;
  }
  return body;
}

async function invoke(cmd, args) {
  if (!tauri) return null;
  try { return await tauri.invoke(cmd, args); } catch (e) { alert(String(e)); return null; }
}

// ------------------------------------------------------------------ loading

async function load() {
  const conn = $("#conn");
  try {
    state.accounts = await api("/accounts");
    conn.textContent = state.accounts.length ? `Syncing on this computer` : "Not signed in yet";
    conn.classList.remove("bad");
  } catch (e) {
    conn.textContent = "The background service is starting…";
    conn.classList.add("bad");
    setTimeout(load, 1500);
    return;
  }
  if (!state.accounts.length) { showView("accounts"); renderAccounts(); renderProjects(); return; }
  try {
    state.groups = (await api("/projects")).accounts;
  } catch (e) {
    state.groups = [];
    $("#list").innerHTML = `<div class="empty"><b>Projects could not be loaded.</b>${esc(e.message)}</div>`;
  }
  renderAccounts();
  renderProjects();
  renderHere();
}

/** The sidebar list of projects synced to this computer, each opening its Your work view. */
function renderHere() {
  const items = [];
  for (const g of state.groups) for (const p of g.projects) for (const w of p.workspaces) {
    const st = state.statuses[w.id];
    const n = st ? st.new.length + st.edited.length + st.deleted.length : 0;
    items.push(`<button data-ws="${esc(w.id)}" ${window.work && work.wsId === w.id && $("#v-work").classList.contains("on") ? 'aria-current="page"' : ""}>
      <span>${esc(p.project.name)}</span>${n ? `<b title="${n} unpublished">${n}</b>` : ""}</button>`);
  }
  $("#here").innerHTML = items.length ? `<h3>On this computer</h3>${items.join("")}` : "";
}

// ------------------------------------------------------------------ projects

function rows() {
  const q = state.q.trim().toLowerCase();
  const out = [];
  for (const g of state.groups) {
    for (const p of g.projects) {
      const ws = p.workspaces[0] || null;
      const st = ws ? state.statuses[ws.id] : null;
      const unpublished = st ? st.new.length + st.edited.length + st.deleted.length : 0;
      const text = `${p.project.name} ${p.project.org.name} ${ws ? ws.root : ""}`.toLowerCase();
      if (q && !text.includes(q)) continue;
      if (state.filter === "here" && !ws) continue;
      if (state.filter === "not" && ws) continue;
      if (state.filter === "mine" && !(p.project.my_claims > 0 || unpublished > 0)) continue;
      out.push({ g, p, ws, st, unpublished });
    }
  }
  return out;
}

function renderProjects() {
  const list = $("#list");
  const all = rows();
  let local = 0, total = 0, here = 0;
  for (const g of state.groups) for (const p of g.projects) { const w = p.workspaces[0]; if (w) { here++; local += w.local_bytes; total += w.total_bytes; } }
  $("#totals").innerHTML = here ? `<strong>${human(local)}</strong>on this computer, from ${here} project${here > 1 ? "s" : ""}` : `<strong>Nothing yet</strong>Turn on a project to sync it here`;
  if (!state.accounts.length) {
    list.innerHTML = `<div class="empty"><b>Add an account to see your projects.</b>Sign in under Accounts with your studio's server address.</div>`;
    return;
  }
  if (!all.length) {
    list.innerHTML = `<div class="empty"><b>No projects match.</b>Try another filter, or ask an admin to add you to a project.</div>`;
    return;
  }
  // Group by account, then organization.
  const groups = new Map();
  for (const r of all) {
    const key = `${r.g.account.id}|${r.p.project.org.id}`;
    if (!groups.has(key)) groups.set(key, { r, items: [] });
    groups.get(key).items.push(r);
  }
  let html = "";
  for (const { r, items } of groups.values()) {
    const a = r.g.account;
    html += `<section class="group"><h2><b>${esc(r.p.project.org.name)}</b>${esc(a.label || a.email)} · ${esc(a.server.replace(/^https?:\/\//, ""))}</h2>`;
    for (const it of items) html += projectRow(it);
    html += `</section>`;
  }
  for (const g of state.groups) if (g.error) html += `<p class="acct-err">${esc(g.account.email)}: ${esc(g.error)}</p>`;
  list.innerHTML = html;
}

function projectRow({ g, p, ws, st, unpublished }) {
  const key = `${g.account.id}:${p.project.id}`;
  const open = state.open === key;
  const pct = ws && ws.total_bytes ? Math.round((ws.local_bytes / ws.total_bytes) * 100) : 0;
  const chips = [
    st && st.behind ? `<span class="chip newer">${st.behind} newer</span>` : "",
    unpublished ? `<span class="chip mine">${unpublished} unpublished</span>` : "",
    p.project.claims ? `<span class="chip claims">${p.project.claims} claimed</span>` : "",
  ].join("");
  return `<article class="proj ${open ? "open" : ""}" data-key="${esc(key)}">
    <div class="row" data-act="toggle" tabindex="0" role="button" aria-expanded="${open}">
      <div><strong>${esc(p.project.name)}</strong><small>Change ${p.project.head_change} · ${p.project.file_count} files · ${human(p.project.size)}</small></div>
      <div class="meter">${ws ? `<span>${human(ws.local_bytes)} of ${human(ws.total_bytes)} here</span><i><b style="width:${pct}%"></b></i>` : `<span>Not on this computer</span>`}</div>
      <div class="chips">${chips}</div>
      <label class="switch" data-act="switch"><span>${ws ? "Syncing" : "Off"}</span><span role="switch" tabindex="0" aria-checked="${!!ws}" aria-label="Sync ${esc(p.project.name)} to this computer"></span></label>
    </div>
    ${open ? detail(g, p, ws) : ""}
  </article>`;
}

function detail(g, p, ws) {
  if (!ws) {
    return `<div class="detail"><div><h3>Not on this computer</h3><p class="note">Turn it on to choose a folder. If the folder already has files from this project, they are kept and matched, never deleted.</p></div>
      <div class="side-acts"><button class="btn pri" data-act="connect">Sync to this computer</button><button class="btn" data-act="web">Open on the web</button></div></div>`;
  }
  const tree = state.trees[ws.id];
  return `<div class="detail">
    <div><h3>Folders on this computer</h3>${tree ? treeHtml(ws.id, tree) : `<p class="note">Loading folders…</p>`}
      <p class="note" id="sel-${esc(ws.id)}"></p></div>
    <div class="side-acts">
      <button class="btn pri" data-act="work">Your work and checkpoints</button>
      <button class="btn" data-act="save-folders">Save folder choices</button>
      <button class="btn" data-act="latest">Get latest</button>
      <button class="btn" data-act="web">Open project</button>
      <button class="btn" data-act="move">Move folder…</button>
      <button class="btn" data-act="disconnect">Stop syncing</button>
      <p>${esc(ws.root)}</p>
      <p>Branch ${esc(ws.branch_name)} · as ${esc(ws.account_email)}</p>
    </div></div>`;
}

// ------------------------------------------------------------------ folder tree (selective sync)

function flatten(node, depth, out) {
  for (const c of node.children) { out.push({ node: c, depth }); if (depth < 2) flatten(c, depth + 1, out); }
  return out;
}

function descendants(node) {
  const out = [];
  (function walk(n) { for (const c of n.children) { out.push(c.path); walk(c); } })(node);
  return out;
}

function stateOf(wsId, node) {
  const set = state.checked[wsId];
  const all = [node.path, ...descendants(node)];
  const on = all.filter((p) => set.has(p)).length;
  return on === 0 ? "none" : on === all.length ? "all" : "some";
}

function treeHtml(wsId, root) {
  const items = flatten(root, 0, []);
  if (!items.length) return `<p class="note">This project has no folders yet.</p>`;
  return `<ul class="tree">${items.map(({ node, depth }) => {
    const s = stateOf(wsId, node);
    return `<li class="${s === "none" ? "dim" : ""}" style="padding-left:${10 + depth * 20}px">
      <input class="tri" type="checkbox" data-path="${esc(node.path)}" aria-label="Keep ${esc(node.path)} on this computer" ${s === "all" ? "checked" : ""} data-ind="${s === "some"}">
      <span class="n">${esc(node.name)}</span><span class="sz">${human(node.size)}</span><span class="sz">${human(node.local_size)} here</span></li>`;
  }).join("")}</ul>`;
}

function initChecked(wsId, root) {
  const set = new Set();
  (function walk(n) {
    if (n.included === "all") { set.add(n.path); for (const d of descendants(n)) set.add(d); return; }
    for (const c of n.children) walk(c);
  })(root);
  state.checked[wsId] = set;
}

function selection(wsId, root) {
  const set = state.checked[wsId];
  const include = [];
  let any = false;
  (function walk(n) {
    for (const c of n.children) {
      const s = stateOf(wsId, c);
      if (s === "all") { include.push(c.path); any = true; }
      else if (s === "some") { any = true; walk(c); }
    }
  })(root);
  const everything = root.children.every((c) => stateOf(wsId, c) === "all");
  return { include: everything ? [] : include, any };
}

function updateSelectionNote(wsId) {
  const root = state.trees[wsId];
  const el = document.getElementById(`sel-${wsId}`);
  if (!root || !el) return;
  const set = state.checked[wsId];
  let bytes = 0;
  (function walk(n) { for (const c of n.children) { if (stateOf(wsId, c) === "all") bytes += c.size; else walk(c); } })(root);
  el.textContent = `After saving: ${human(bytes)} on this computer. Files you have edited stay, whatever you untick.`;
  document.querySelectorAll(`.tri`).forEach((cb) => { cb.indeterminate = cb.dataset.ind === "true"; });
  void set;
}

async function loadTree(ws) {
  try {
    const root = await api(`/workspaces/${ws.id}/folders`);
    state.trees[ws.id] = root;
    initChecked(ws.id, root);
  } catch (e) {
    state.trees[ws.id] = { children: [] };
  }
  renderProjects();
  updateSelectionNote(ws.id);
}

// ------------------------------------------------------------------ actions

function findRow(key) {
  for (const g of state.groups) for (const p of g.projects) if (`${g.account.id}:${p.project.id}` === key) return { g, p, ws: p.workspaces[0] || null };
  return null;
}

async function onAction(act, key, el) {
  const r = findRow(key);
  if (!r) return;
  const { g, p, ws } = r;
  if (act === "toggle") {
    state.open = state.open === key ? null : key;
    renderProjects();
    if (state.open && ws) { loadTree(ws); loadStatus(ws); }
    return;
  }
  if (act === "switch") return ws ? confirmDisconnect(ws, p) : connectDialog(g, p);
  if (act === "connect") return connectDialog(g, p);
  if (act === "work") return openWork(ws.id);
  if (act === "move") return moveFolderDialog(ws, p);
  if (act === "disconnect") return confirmDisconnect(ws, p);
  if (act === "web") return invoke("open_server_page", { accountId: g.account.id, path: `/p/${p.project.id}/today` }) ?? window.open(`${g.account.server}/p/${p.project.id}/today`);
  if (act === "latest") {
    el.disabled = true; el.textContent = "Getting latest…";
    try { const res = await api(`/workspaces/${ws.id}/get-latest`, { method: "POST", body: "{}" }); el.textContent = `Got ${res.downloaded} file${res.downloaded === 1 ? "" : "s"}`; }
    catch (e) { alert(e.message); el.textContent = "Get latest"; }
    el.disabled = false; setTimeout(load, 800);
    return;
  }
  if (act === "save-folders") {
    const root = state.trees[ws.id];
    const { include, any } = selection(ws.id, root);
    if (!any) { alert("Tick at least one folder, or stop syncing this project."); return; }
    el.disabled = true; el.textContent = "Saving…";
    try {
      const res = await api(`/workspaces/${ws.id}/sync`, { method: "PUT", body: JSON.stringify({ include, exclude: [] }) });
      const rep = res.report || {};
      el.textContent = "Saved";
      if (rep.kept && rep.kept.length) alert(`${rep.kept.length} file(s) with unpublished edits were kept in folders you unticked.`);
    } catch (e) { alert(e.message); el.textContent = "Save folder choices"; }
    el.disabled = false;
    delete state.trees[ws.id];
    await load();
    const again = findRow(key);
    if (again && again.ws) loadTree(again.ws);
  }
}

async function loadStatus(ws) {
  try { state.statuses[ws.id] = await api(`/workspaces/${ws.id}/status`); renderProjects(); renderHere(); updateSelectionNote(ws.id); } catch (_) {}
}

function dialog(html, onSubmit) {
  const dlg = $("#dlg"); const body = $("#dlg-body");
  body.innerHTML = html;
  body.onsubmit = async (e) => {
    const btn = e.submitter;
    if (!btn || btn.value === "cancel") return;
    e.preventDefault();
    await onSubmit(btn.value, body, dlg);
  };
  dlg.showModal();
}

async function connectDialog(g, p) {
  const base = (await invoke("default_root")) || "";
  const sep = base.includes("\\") ? "\\" : "/";
  const folder = base ? `${base}${sep}${p.project.org.name}${sep}${p.project.name}` : "";
  dialog(`<h2>Sync ${esc(p.project.name)} to this computer</h2>
    <p class="note">Choose a folder. If it already has files from this project, they are kept and matched; nothing in it is deleted or replaced.</p>
    <div class="pathrow"><input id="cfolder" value="${esc(folder)}" aria-label="Folder"><button class="btn" value="pick">Choose…</button></div>
    <div id="cprev"></div>
    <div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn" value="preview">Check the folder</button><button class="btn pri" value="connect">Sync this folder</button></div>`,
    async (what, body, dlg) => {
      const input = $("#cfolder", body);
      if (what === "pick") { const f = await invoke("pick_folder", { start: input.value || null }); if (f) input.value = f; return; }
      const payload = { account_id: g.account.id, project_id: p.project.id, folder: input.value.trim(), dry_run: what === "preview" };
      if (!payload.folder) { $("#cprev", body).innerHTML = `<p class="err">Choose a folder first.</p>`; return; }
      try {
        const res = await api("/connect", { method: "POST", body: JSON.stringify(payload) });
        const r = res.report;
        if (what === "preview") {
          $("#cprev", body).innerHTML = `<ul class="adopt">
            <li style="--k:var(--new)"><b>${r.matched}</b><div><strong>Already match the server</strong><small>Nothing to download for these.</small></div></li>
            <li style="--k:var(--acc)"><b>${r.different.length}</b><div><strong>Different from the server</strong><small>Kept as they are and shown as edited.</small></div></li>
            <li style="--k:var(--hold)"><b>${r.local_only}</b><div><strong>Only in this folder</strong><small>Kept and shown as new.</small></div></li>
            <li><b>${r.to_download}</b><div><strong>To download</strong><small>${human(r.download_bytes)}</small></div></li></ul>`;
          return;
        }
        dlg.close();
        await load();
      } catch (e) {
        $("#cprev", body).innerHTML = `<p class="err">${esc(e.message)}</p>`;
      }
    });
}

/** Move a synced folder elsewhere on this computer. Same drive: a rename. Another drive: a copy
 *  with times kept, then the old folder is removed. Nothing is downloaded again. */
async function moveFolderDialog(ws, p) {
  const sep = ws.root.includes("\\") ? "\\" : "/";
  const parent = ws.root.split(sep).slice(0, -1).join(sep);
  const size = human(ws.local_bytes);
  dialog(`<h2>Move ${esc(p.project.name)}</h2>
    <p class="note">Now at <b>${esc(ws.root)}</b> (${size}). Choose a new or empty folder. On the same drive this is instant; to another drive everything is copied, checked, and then the old folder is removed. Nothing is downloaded again.</p>
    <div class="pathrow"><input id="mfolder" value="${esc(ws.root)}" aria-label="New folder"><button class="btn" value="pick">Choose…</button></div>
    <p class="note">Close apps that have files from this folder open first.</p>
    <p class="err" id="merr" role="alert"></p>
    <div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn pri" value="move">Move folder</button></div>`,
    async (what, body, dlg) => {
      const input = $("#mfolder", body);
      if (what === "pick") {
        const f = await invoke("pick_folder", { start: parent || null });
        if (f) input.value = `${f}${sep}${p.project.name}`;
        return;
      }
      const to = input.value.trim();
      if (!to || to === ws.root) { $("#merr", body).textContent = "Choose a different folder."; return; }
      const btn = body.querySelector('button[value="move"]');
      btn.disabled = true; btn.textContent = "Moving…";
      try {
        const r = await api(`/workspaces/${ws.id}/move`, { method: "POST", body: JSON.stringify({ to }) });
        dlg.close();
        if (window.toast) toast(r.method === "renamed" ? `Moved to ${r.to}.` : `Copied ${human(r.bytes)} to ${r.to}.${r.old_folder_left ? " The old folder could not be fully removed." : ""}`, !!r.old_folder_left);
        if (r.old_folder_left) alert(r.old_folder_left);
        delete state.trees[ws.id];
        await load();
        if (window.work && work.wsId === ws.id && $("#v-work").classList.contains("on")) openWork(ws.id);
      } catch (e) {
        $("#merr", body).textContent = e.message;
        btn.disabled = false; btn.textContent = "Move folder";
      }
    });
}

function confirmDisconnect(ws, p) {
  dialog(`<h2>Stop syncing ${esc(p.project.name)}?</h2>
    <p>The folder <b>${esc(ws.root)}</b> and every file in it stay where they are. MultiParity stops watching it, and you can turn it back on at any time.</p>
    <div class="dlg-acts"><button class="btn" value="cancel">Keep syncing</button><button class="btn pri" value="go">Stop syncing</button></div>`,
    async (_, __, dlg) => { await api(`/workspaces/${ws.id}/disconnect`, { method: "POST", body: "{}" }); dlg.close(); load(); });
}

// ------------------------------------------------------------------ accounts

function renderAccounts() {
  loadNotifyPrefs();
  const ul = $("#accts");
  if (!state.accounts.length) { ul.innerHTML = `<li><div><strong>No accounts yet</strong><small>Add one with the form.</small></div></li>`; return; }
  ul.innerHTML = state.accounts.map((a) => `<li><div><strong>${esc(a.label ? `${a.name}, ${a.label}` : a.name)}</strong><small>${esc(a.email)} · ${esc(a.server)}</small></div>
    <button class="btn sm" data-signout="${esc(a.id)}">Sign out</button></li>`).join("");
}

// ------------------------------------------------------------------ updates (app only)

async function renderUpdate(force) {
  if (!tauri) return;
  const el = $("#upd");
  let st;
  try { st = await tauri.invoke(force ? "check_update" : "update_status"); }
  catch (e) { el.hidden = false; el.innerHTML = `<span>Version ${esc((await tauri.invoke("update_status")).current)}</span><small>${esc(String(e))}</small>`; return; }
  el.hidden = false;
  el.innerHTML = st.ready
    ? `<button class="btn sm pri" id="upd-go">Restart to update to ${esc(st.ready)}</button>`
    : `<span>Version ${esc(st.current)}</span><button class="link" id="upd-check" ${st.checking ? "disabled" : ""}>${st.checking ? "Checking…" : force ? "Up to date" : "Check for updates"}</button>`;
}

$("#upd").addEventListener("click", async (e) => {
  if (e.target.id === "upd-check") { e.target.textContent = "Checking…"; e.target.disabled = true; renderUpdate(true); }
  if (e.target.id === "upd-go") {
    e.target.disabled = true; e.target.textContent = "Updating…";
    try { await tauri.invoke("install_update"); } catch (err) { alert(String(err)); renderUpdate(); }
  }
});
renderUpdate();
setInterval(() => renderUpdate(), 10 * 60 * 1000);

async function loadNotifyPrefs() {
  try {
    const p = await api("/notifications/settings");
    for (const box of document.querySelectorAll("#notify-prefs input")) box.checked = !!p[box.dataset.pref];
  } catch (_) {}
}

$("#notify-prefs").addEventListener("change", async () => {
  const body = {};
  for (const box of document.querySelectorAll("#notify-prefs input")) body[box.dataset.pref] = box.checked;
  try { await api("/notifications/settings", { method: "PUT", body: JSON.stringify(body) }); }
  catch (e) { alert(e.message); loadNotifyPrefs(); }
});

$("#login").addEventListener("submit", async (e) => {
  e.preventDefault();
  const f = new FormData(e.target);
  const err = $("#login-err"); err.textContent = "";
  try {
    await api("/accounts/login", { method: "POST", body: JSON.stringify(Object.fromEntries(f)) });
    e.target.reset();
    await load();
    showView("projects");
  } catch (ex) { err.textContent = ex.message; }
});

$("#accts").addEventListener("click", async (e) => {
  const id = e.target.dataset.signout;
  if (!id) return;
  const a = state.accounts.find((x) => x.id === id);
  dialog(`<h2>Sign out of ${esc(a.email)}?</h2><p>Folders that use this account keep their files. They stop syncing until you sign in again.</p>
    <div class="dlg-acts"><button class="btn" value="cancel">Stay signed in</button><button class="btn pri" value="go">Sign out</button></div>`,
    async (_, __, dlg) => { await api(`/accounts/${id}`, { method: "DELETE" }); dlg.close(); load(); });
});

// ------------------------------------------------------------------ wiring

function showView(v) {
  document.querySelectorAll(".nav button").forEach((b) => b.dataset.view === v ? b.setAttribute("aria-current", "page") : b.removeAttribute("aria-current"));
  document.querySelectorAll(".view").forEach((s) => s.classList.toggle("on", s.id === `v-${v}`));
  if (v !== "work" && window.work) work.close();
  // A project's own look shows only inside it.
  if (v !== "work") MPTheme.applyChosen(null);
  renderHere();
}

$(".nav").addEventListener("click", (e) => { const v = e.target.dataset.view; if (v) showView(v); });
$("#here").addEventListener("click", (e) => { const b = e.target.closest("[data-ws]"); if (b) openWork(b.dataset.ws); });
$("#filters").addEventListener("click", (e) => {
  const f = e.target.dataset.f; if (!f) return;
  state.filter = f;
  document.querySelectorAll("#filters button").forEach((b) => b.setAttribute("aria-pressed", b.dataset.f === f));
  renderProjects();
});
$("#q").addEventListener("input", (e) => { state.q = e.target.value; renderProjects(); });
$("#list").addEventListener("click", (e) => {
  const tri = e.target.closest(".tri");
  if (tri) {
    const art = tri.closest(".proj"); const r = findRow(art.dataset.key);
    const wsId = r.ws.id; const set = state.checked[wsId];
    const node = (function find(n) { if (n.path === tri.dataset.path) return n; for (const c of n.children) { const x = find(c); if (x) return x; } return null; })(state.trees[wsId]);
    const on = stateOf(wsId, node) !== "all";
    for (const p of [node.path, ...descendants(node)]) on ? set.add(p) : set.delete(p);
    renderProjects(); updateSelectionNote(wsId);
    return;
  }
  const el = e.target.closest("[data-act]");
  if (!el) return;
  e.stopPropagation();
  const key = el.closest(".proj").dataset.key;
  onAction(el.dataset.act, key, el);
});
$("#list").addEventListener("keydown", (e) => {
  if ((e.key === "Enter" || e.key === " ") && e.target.matches('[data-act="toggle"], [role=switch]')) {
    e.preventDefault();
    const act = e.target.matches("[role=switch]") ? "switch" : "toggle";
    onAction(act, e.target.closest(".proj").dataset.key, e.target);
  }
});

load();
setInterval(() => { if (!document.hidden && !$("#dlg").open && !$("#v-work").classList.contains("on")) load(); }, 30000);
// Unpublished counts in the sidebar, for every synced folder.
async function loadAllStatuses() {
  for (const g of state.groups) for (const p of g.projects) for (const w of p.workspaces) {
    try { state.statuses[w.id] = await api(`/workspaces/${w.id}/status`); } catch (_) {}
  }
  renderHere();
}
setInterval(() => { if (!document.hidden) loadAllStatuses(); }, 60000);
setTimeout(loadAllStatuses, 1500);
