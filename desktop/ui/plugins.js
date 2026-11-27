// MultiParity desktop: plugins. The Plugins page (install from GitHub after seeing what a plugin
// does, update, turn off, remove), plugin operations in file menus and the project's Plugins
// button, and plugin views: web pages in a sandboxed frame, as a project tab or a page of their
// own. A view talks to the app only through the bridge below (plugin-sdk.js on its side).
// Uses helpers from app.js, work.js and project.js.
"use strict";

var plugins = { list: [], loaded: false, appView: null, tab: "installed", catalog: null, catalogError: "", q: "", sources: null };

async function loadPlugins() {
  try { plugins.list = await api("/plugins"); } catch (_) { plugins.list = []; }
  plugins.loaded = true;
  renderPluginNav();
  if ($("#v-plugins").classList.contains("on")) renderPlugins();
}

const enabledPlugins = () => plugins.list.filter((p) => p.enabled);

/** Plugin views in a place ("project" or "app"): [{plugin, view, key}]. */
function pluginViews(place) {
  return enabledPlugins().flatMap((p) => (p.manifest.views || []).filter((v) => (v.place || "project") === place).map((v) => ({ p, v, key: `plug:${p.id}:${v.id}` })));
}

/** Does a glob (*, **, ?) match a path or its file name? Case-blind, like the daemon. */
function globMatch(glob, path) {
  const re = new RegExp("^" + glob.split("**").map((part) => part.split("*").map((x) => x.replace(/[.+^${}()|[\]\\?]/g, (c) => (c === "?" ? "." : "\\" + c))).join("[^/]*")).join(".*") + "$", "i");
  return re.test(path) || re.test(path.split("/").pop());
}

/** Menu items for plugin operations that work on all of these files. */
function pluginFileItems(paths) {
  const out = [];
  for (const p of enabledPlugins()) for (const o of p.manifest.operations || []) {
    if (!(o.files || []).length || !paths.every((x) => o.files.some((g) => globMatch(g, x)))) continue;
    out.push([`${o.title} (${p.manifest.name})`, () => runPluginOp(p, o, paths)]);
  }
  return out;
}

/** Plugin operations on a whole project (no file patterns), for the project's Plugins… button. */
function projectOps() {
  return enabledPlugins().flatMap((p) => (p.manifest.operations || []).filter((o) => !(o.files || []).length).map((o) => ({ p, o })));
}

async function runPluginOp(p, o, paths, wsId) {
  const ws = wsId || work.wsId;
  if (!ws) return toast("Open a synced project first.", true);
  if (o.confirm && !(await confirmBox(o.title, o.confirm))) return;
  try {
    const r = await api(`/workspaces/${ws}/plugins/${encodeURIComponent(p.id)}/run`, { method: "POST", body: JSON.stringify({ operation: o.id, paths }) });
    toast((r && r.message) || `${o.title}: done.`, r && r.ok === false);
    if (r && r.open && tauri) invoke("open_path", { path: r.open });
    if (window.work && work.ws) refreshWork(true);
    return r;
  } catch (e) { toast(e.message, true); throw e; }
}

/** A yes/no question in the app's dialog. */
function confirmBox(title, text) {
  return new Promise((resolve) => {
    dialog(`<h2>${esc(title)}</h2><p class="note">${esc(text)}</p><div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn pri" value="go">Go ahead</button></div>`,
      async (what, body, dlg) => { dlg.close(); resolve(what === "go"); });
    $("#dlg").addEventListener("close", () => resolve(false), { once: true });
  });
}

// ------------------------------------------------------------------ views (sandboxed frames)

function frameHtml(p, v, wsId) {
  const src = `${DAEMON}/plugins/${encodeURIComponent(p.id)}/ui/${v.page.split("/").map(encodeURIComponent).join("/")}${wsId ? `?ws=${encodeURIComponent(wsId)}` : ""}`;
  // No allow-same-origin: the page cannot reach this app's API or storage, only the bridge.
  return `<iframe class="plugframe" title="${esc(v.title)}" sandbox="allow-scripts allow-forms allow-popups allow-downloads allow-modals" data-plugin="${esc(p.id)}" data-view="${esc(v.id)}" data-ws="${esc(wsId || "")}" src="${esc(src)}"></iframe>`;
}

/** Show (or hide) the project tab's frame. It lives outside the work view, which redraws often. */
function syncPluginTab() {
  const host = $("#plug-host");
  const on = $("#v-work").classList.contains("on") && window.work && work.ws && String(work.tab).startsWith("plug:");
  if (!on) { host.hidden = true; host.innerHTML = ""; host.dataset.key = ""; return; }
  const key = `${work.tab}@${work.wsId}`;
  if (host.dataset.key !== key) {
    const [, id, vid] = work.tab.split(":");
    const p = plugins.list.find((x) => x.id === id);
    const v = p && p.manifest.views.find((x) => x.id === vid);
    host.innerHTML = v ? frameHtml(p, v, work.wsId) : `<div class="notice bad">That plugin view is gone. It may have been turned off or removed.</div>`;
    host.dataset.key = key;
  }
  host.hidden = false;
}

function renderPluginNav() {
  const nav = $("#plugnav");
  const views = pluginViews("app");
  nav.hidden = !views.length;
  nav.innerHTML = views.length ? `<p class="lbl">From plugins</p>${views.map(({ p, v, key }) =>
    `<button data-plugview="${esc(key)}" title="${esc(p.manifest.name)}" ${plugins.appView === key ? 'aria-current="page"' : ""}>${icon("puzzle")}${esc(v.title)}</button>`).join("")}` : "";
}

function openPluginView(key) {
  const [, id, vid] = key.split(":");
  const p = plugins.list.find((x) => x.id === id);
  const v = p && p.manifest.views.find((x) => x.id === vid);
  if (!v) return;
  plugins.appView = key;
  showView("plugin");
  $("#v-plugin").innerHTML = `<header class="head"><div><h1>${esc(v.title)}</h1><p>From the ${esc(p.manifest.name)} plugin.</p></div></header>${frameHtml(p, v, "")}`;
  renderPluginNav();
}

// The bridge: requests from plugin pages, answered with the plugin's own context.
window.addEventListener("message", async (e) => {
  const m = e.data;
  if (!m || m.mp !== 1 || !m.method) return;
  const frame = [...document.querySelectorAll("iframe.plugframe")].find((f) => f.contentWindow === e.source);
  if (!frame) return;
  const p = plugins.list.find((x) => x.id === frame.dataset.plugin && x.enabled);
  const reply = (result, error) => e.source.postMessage({ mp: 1, id: m.id, result, error }, "*");
  if (!p) return reply(null, "This plugin is turned off.");
  const wsId = frame.dataset.ws || (m.params && m.params.workspace) || "";
  const can = (perm) => (p.manifest.permissions || []).includes(perm);
  try {
    switch (m.method) {
      case "context": {
        const here = (state.groups || []).flatMap((g) => g.projects).flatMap((x) => x.workspaces.map((w) => ({ id: w.id, root: w.root, project: x.project.name, branch: w.branch_name })));
        const ws = here.find((w) => w.id === wsId) || null;
        const selection = wsId && window.work && work.wsId === wsId ? ticked().map((f) => f.path) : [];
        return reply({ plugin: { id: p.id, name: p.manifest.name, version: p.manifest.version }, view: frame.dataset.view, workspace: ws, workspaces: here, selection, theme: document.documentElement.dataset.mode || "" });
      }
      case "run": {
        const o = (p.manifest.operations || []).find((x) => x.id === m.params.operation);
        if (!o) return reply(null, `${p.manifest.name} has no operation ${m.params.operation}.`);
        return reply(await runPluginOp(p, o, m.params.paths || [], wsId));
      }
      case "local": {
        // Read-only, and only about the page's own folder.
        if (!can("files:read")) return reply(null, `${p.manifest.name} did not ask to read your files.`);
        if (!wsId) return reply(null, "This page is not about a synced folder.");
        const path = String(m.params.path || "").replace(/^\/+/, "");
        if (!/^(status|tree|checkpoints|versions)(\?|$)/.test(path)) return reply(null, "Plugins can read status, tree, checkpoints and versions.");
        return reply(await api(`/workspaces/${wsId}/${path}`));
      }
      case "server": {
        if (!can("server")) return reply(null, `${p.manifest.name} did not ask to read from the server.`);
        if (!wsId) return reply(null, "This page is not about a synced folder.");
        return reply(await api(`/workspaces/${wsId}/server/${String(m.params.path || "").replace(/^\/+/, "")}`));
      }
      case "toast": toast(`${p.manifest.name}: ${String(m.params.text || "").slice(0, 300)}`, !!m.params.bad); return reply(true);
      case "open":
      case "reveal": {
        if (!tauri) return reply(null, "Only in the desktop app.");
        const ws = (state.groups || []).flatMap((g) => g.projects).flatMap((x) => x.workspaces).find((w) => w.id === wsId);
        const rel = String(m.params.path || "");
        if (!ws || rel.includes("..")) return reply(null, "Only files in this project's folder.");
        const sepc = ws.root.includes("\\") ? "\\" : "/";
        await invoke(m.method === "open" ? "open_path" : "reveal_path", { path: ws.root + sepc + rel.split("/").join(sepc) });
        return reply(true);
      }
    }
    reply(null, `Unknown request ${m.method}.`);
  } catch (err) { reply(null, err.message || String(err)); }
});

// ------------------------------------------------------------------ the Plugins page

function renderPlugins() {
  const v = $("#v-plugins");
  const rows = plugins.list.map((p) => {
    const m = p.manifest;
    const what = [
      (m.hooks || []).length ? `Runs ${m.hooks.map((h) => h.replace(/_/g, " ")).join(", ")}` : "",
      (m.operations || []).length ? `${m.operations.length} operation${m.operations.length === 1 ? "" : "s"}: ${m.operations.map((o) => o.title).join(", ")}` : "",
      (m.views || []).length ? `Views: ${m.views.map((x) => x.title).join(", ")}` : "",
    ].filter(Boolean);
    return `<li class="plug ${p.enabled ? "" : "off"}">
      <div class="pl-main">
        <strong>${icon("puzzle")}${esc(m.name)} <small>${esc(m.version || "")}</small></strong>
        ${m.description ? `<p>${esc(m.description)}</p>` : ""}
        <ul class="pl-what">${what.map((x) => `<li>${esc(x)}</li>`).join("")}</ul>
        <small class="pl-src">${p.linked ? `Linked folder ${esc(p.dir)}` : p.source.startsWith("web:") ? `From ${esc(p.source.slice(4).split("#")[0])}` : `${esc(p.source.replace(/^github:/, ""))}${p.git_ref ? ` (${esc(p.git_ref)})` : ""} at ${esc(p.commit.slice(0, 12))}`}${p.required_by ? ` · required by ${esc(p.required_by)}` : ""}</small>
      </div>
      <div class="pl-acts">
        <label class="switch" data-pl="toggle" data-id="${esc(p.id)}"><span>${p.enabled ? "Enabled" : "Disabled"}</span><span role="switch" tabindex="0" aria-checked="${p.enabled}" aria-label="Use ${esc(m.name)}"></span></label>
        ${p.linked ? "" : `<button class="btn sm" data-pl="update" data-id="${esc(p.id)}">Check for update</button>`}
        ${p.required_by ? "" : `<button class="btn sm" data-pl="remove" data-id="${esc(p.id)}">Remove</button>`}
      </div>
    </li>`;
  }).join("");
  const tabs = [["installed", `Installed${plugins.list.length ? ` <b>${plugins.list.length}</b>` : ""}`], ["browse", "Browse"], ["sources", "Sources"]];
  const head = `
    <header class="head"><div><h1>Plugins</h1><p>Add steps around publishing, actions on files and new views, from plugins on GitHub or the web. A plugin runs on this computer as you, so install only ones you trust.</p></div></header>
    <nav class="tabs" role="tablist" aria-label="Plugins">${tabs.map(([k, l]) => `<button role="tab" data-pl="tab" data-t="${k}" aria-selected="${plugins.tab === k}">${l}</button>`).join("")}</nav>
    <div id="pl-preview"></div>`;
  if (plugins.tab === "browse") { v.innerHTML = head + browseHtml(); return; }
  if (plugins.tab === "sources") { v.innerHTML = head + sourcesHtml(); return; }
  v.innerHTML = head + `
    <div class="panel">
      <h2>Install from GitHub</h2>
      <div class="pathrow"><input id="pl-spec" placeholder="owner/repo, owner/repo/folder or owner/repo@v1.2" aria-label="GitHub repository"><button class="btn pri" data-pl="look">Look it up</button></div>
      <p class="note">You see what it does and what it can reach before anything is installed. Or <button class="link" data-pl="tab" data-t="browse">browse your sources</button>. Making one? <button class="link" data-pl="link">Use a folder on this computer</button>.</p>
    </div>
    <div class="panel">
      <h2>Installed</h2>
      ${plugins.list.length ? `<ul class="plugs">${rows}</ul>` : `<div class="empty"><b>No plugins yet.</b>Install one above.</div>`}
    </div>`;
}

// ------------------------------------------------------------------ Browse and Sources

async function loadCatalog(refresh) {
  if (plugins.catalogLoading) return;
  plugins.catalogLoading = true;
  plugins.catalogError = "";
  plugins.catalog = null;
  if (plugins.tab === "browse") renderPlugins();
  try { plugins.catalog = await api(`/plugin-catalog${refresh ? "?refresh=1" : ""}`); }
  catch (e) { plugins.catalogError = e.message; plugins.catalog = []; }
  plugins.catalogLoading = false;
  if (plugins.tab === "browse") renderPlugins();
}

async function loadSources() {
  try { plugins.sources = await api("/plugin-sources"); } catch (e) { plugins.sources = []; toast(e.message, true); }
  if (plugins.tab === "sources") renderPlugins();
}

function browseHtml() {
  if (!plugins.catalog) { if (!plugins.catalogLoading) loadCatalog(false); return `<div class="panel"><p class="note">Reading your sources…</p></div>`; }
  const q = plugins.q.trim().toLowerCase();
  const groups = plugins.catalog.map((g) => {
    const items = g.plugins.filter((e) => !q || `${e.name} ${e.description} ${e.author} ${e.id}`.toLowerCase().includes(q));
    const cards = items.map((e) => {
      const have = e.installed;
      const act = e.problem && !e.manifest && e.kind === "github"
        ? `<span class="note">${esc(e.problem)}</span>`
        : `<button class="btn sm ${have ? "" : "pri"}" data-pl="pick" data-key="${esc(e.key)}">${have ? "Other versions…" : "Install…"}</button>`;
      return `<li class="cat">
        <div class="pl-main">
          <strong>${icon("puzzle")}${esc(e.name)} <small>${esc(e.version || "")}</small>${have ? ` <span class="chip">Installed ${esc(have.version || "")}</span>` : ""}</strong>
          ${e.description ? `<p>${esc(e.description)}</p>` : ""}
          <small class="pl-src">${e.author ? `${esc(e.author)} · ` : ""}${e.spec ? esc(e.spec) : "Download from the web"}${e.homepage ? ` · <a href="${esc(e.homepage)}" target="_blank" rel="noopener">Page</a>` : ""}</small>
        </div>
        <div class="pl-acts">${act}</div>
      </li>`;
    }).join("");
    return `<div class="panel"><h2>${esc(g.source.name || g.source.location)}</h2>
      ${g.problem ? `<p class="err">${esc(g.problem)}</p>` : ""}
      ${cards ? `<ul class="plugs">${cards}</ul>` : g.problem ? "" : `<p class="note">${q ? "Nothing here matches." : "No plugins listed."}</p>`}</div>`;
  }).join("");
  return `<div class="toolbar-row">
      <input id="pl-q" type="search" placeholder="Find a plugin" value="${esc(plugins.q)}" aria-label="Find a plugin">
      <button class="btn sm" data-pl="refresh">Refresh</button>
    </div>
    ${plugins.catalogError ? `<p class="err">${esc(plugins.catalogError)}</p>` : ""}
    ${groups || `<div class="panel"><div class="empty"><b>No sources.</b>Add a GitHub repository or a web address under Sources.</div></div>`}`;
}

function sourcesHtml() {
  if (!plugins.sources) { if (!plugins.sourcesLoading) { plugins.sourcesLoading = true; loadSources().finally(() => { plugins.sourcesLoading = false; }); } return `<div class="panel"><p class="note">Loading…</p></div>`; }
  return `<div class="panel">
      <h2>Add a source</h2>
      <div class="pathrow"><input id="src-loc" placeholder="owner/repo, or https://…/multiparity-plugins.json" aria-label="Source"><button class="btn pri" data-pl="src-add">Add</button></div>
      <p class="note">A GitHub repository that is a plugin, has plugins in its folders, or lists plugins (its own or other repositories') in a <code>multiparity-plugins.json</code>; or the web address of such a list.</p>
    </div>
    <div class="panel">
      <h2>Your sources</h2>
      ${plugins.sources.length ? `<ul class="plugs">${plugins.sources.map((x) => `<li class="plug">
        <div class="pl-main"><strong>${esc(x.name || x.location)}</strong><small class="pl-src">${esc(x.location.replace(/^github:/, "github.com/"))}</small></div>
        <div class="pl-acts"><button class="btn sm" data-pl="src-remove" data-id="${esc(x.id)}">Remove</button></div></li>`).join("")}</ul>` : `<div class="empty"><b>No sources.</b>Add one above.</div>`}
    </div>`;
}

/** Choose a version of a catalog plugin, then see what it does before installing. */
async function pickVersion(key) {
  let list;
  try { list = await api(`/plugin-catalog/versions?key=${encodeURIComponent(key)}`); }
  catch (e) { return toast(e.message, true); }
  const entry = (plugins.catalog || []).flatMap((g) => g.plugins).find((e) => e.key === key) || { name: "the plugin" };
  const when = (t) => (t ? ` · ${new Date(t).toLocaleDateString()}` : "");
  dialog(`<h2>Install ${esc(entry.name)}</h2>
    <p class="note">Choose what to install. A release is a version its makers published; the branch is whatever is newest there, released or not. Updates follow what you choose.</p>
    <ul class="choices">${list.map((x, i) => `<li><label><input type="radio" name="pv" value="${i}" ${x.recommended ? "checked" : ""}>
      <span><strong>${esc(x.label)}</strong>${x.notes ? `<small class="pv-notes">${esc(x.notes)}</small>` : ""}</span>
      <small>${x.kind === "prerelease" ? "Pre-release" : x.kind === "branch" ? "Branch" : x.recommended ? "Newest release" : "Release"}${when(x.published_at)}</small></label></li>`).join("")}</ul>
    <p class="err" id="pv-err" role="alert"></p>
    <div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn pri" value="go">Continue</button></div>`,
    async (what, body, dlg) => {
      const picked = body.querySelector('input[name="pv"]:checked');
      if (!picked) { $("#pv-err", body).textContent = "Choose a version."; return; }
      const go = body.querySelector('button[value="go"]');
      go.disabled = true; go.textContent = "Downloading…";
      try {
        const st = await api("/plugin-catalog/preview", { method: "POST", body: JSON.stringify({ key, version: list[+picked.value].git_ref }) });
        dlg.close();
        $("#pl-preview").innerHTML = previewHtml(st, st.replaces ? "Install this version" : "Install");
        $("#pl-preview").scrollIntoView({ block: "nearest" });
      } catch (e) { $("#pv-err", body).textContent = e.message; go.disabled = false; go.textContent = "Continue"; }
    });
}

function previewHtml(st, verb) {
  const m = st.manifest;
  return `<div class="pl-card">
    <strong>${esc(m.name)} ${esc(m.version || "")}</strong>${m.author ? ` <small>by ${esc(m.author)}</small>` : ""}
    ${m.description ? `<p>${esc(m.description)}</p>` : ""}
    <p class="note">${st.source.startsWith("web:")
      ? `From ${esc(st.source.slice(4).split("#")[0])}, download checksum ${esc(st.commit.slice(0, 12))}`
      : `From ${esc(st.source.replace(/^github:/, ""))}${st.git_ref ? ` (${esc(st.git_ref)})` : ""} at commit ${esc(st.commit.slice(0, 12))}`}${st.replaces ? `, replacing version ${esc(st.replaces.version)}` : ""}.</p>
    <ul class="pl-what">
      ${(m.hooks || []).map((h) => `<li>Runs ${esc(h.replace(/_/g, " "))}${h.startsWith("before") ? " and can stop it" : ""}</li>`).join("")}
      ${(m.operations || []).map((o) => `<li>Adds <b>${esc(o.title)}</b>${(o.files || []).length ? ` for ${esc(o.files.join(", "))}` : " for a project"}</li>`).join("")}
      ${(m.views || []).map((x) => `<li>Adds the view <b>${esc(x.title)}</b> (${x.place === "app" ? "a page of its own" : "a project tab"})</li>`).join("")}
      ${st.command ? `<li>Runs <code>${esc(st.command.join(" "))}</code> on this computer, as you</li>` : ""}
    </ul>
    ${st.permissions.length ? `<p class="note">It says it will:</p><ul class="pl-perm">${st.permissions.map((x) => `<li class="${x.new ? "new" : ""}">${esc(x.text)}${x.new ? " <b>(new)</b>" : ""}</li>`).join("")}</ul>` : ""}
    <div class="dlg-acts"><button class="btn" data-pl="cancel">Cancel</button><button class="btn pri" data-pl="install" data-stage="${esc(st.stage)}">${verb}</button></div>
  </div>`;
}

$("#v-plugins").addEventListener("click", async (e) => {
  const el = e.target.closest("[data-pl]");
  if (!el) return;
  const what = el.dataset.pl;
  const box = $("#pl-preview");
  if (what === "tab") { plugins.tab = el.dataset.t; renderPlugins(); return; }
  if (what === "refresh") { loadCatalog(true); return; }
  if (what === "pick") { pickVersion(el.dataset.key); return; }
  if (what === "src-add") {
    const loc = $("#src-loc").value.trim();
    if (!loc) return toast("Type a GitHub repository or a web address first.", true);
    el.disabled = true; el.textContent = "Checking…";
    try {
      const r = await api("/plugin-sources", { method: "POST", body: JSON.stringify({ location: loc }) });
      toast(`Added ${r.source.name}: ${r.plugins} plugin${r.plugins === 1 ? "" : "s"}.`);
      plugins.catalog = null; await loadSources();
    } catch (err) { toast(err.message, true); el.disabled = false; el.textContent = "Add"; }
    return;
  }
  if (what === "src-remove") {
    try { await api(`/plugin-sources/${encodeURIComponent(el.dataset.id)}`, { method: "DELETE" }); plugins.catalog = null; await loadSources(); }
    catch (err) { toast(err.message, true); }
    return;
  }
  if (what === "look") {
    const spec = $("#pl-spec").value.trim();
    if (!spec) return toast("Type a GitHub repository first.", true);
    el.disabled = true; el.textContent = "Looking…";
    try { box.innerHTML = previewHtml(await api("/plugins/preview", { method: "POST", body: JSON.stringify({ spec }) }), "Install"); }
    catch (err) { box.innerHTML = `<p class="err">${esc(err.message)}</p>`; }
    el.disabled = false; el.textContent = "Look it up";
    return;
  }
  if (what === "cancel") { box.innerHTML = ""; return; }
  if (what === "install") {
    el.disabled = true;
    try {
      const p = await api("/plugins/install", { method: "POST", body: JSON.stringify({ stage: el.dataset.stage }) });
      toast(`Installed ${p.manifest.name} ${p.manifest.version || ""}.`);
      box.innerHTML = ""; if ($("#pl-spec")) $("#pl-spec").value = "";
      plugins.catalog = null; // re-read, so Installed marks are right
      await loadPlugins();
    } catch (err) { toast(err.message, true); el.disabled = false; }
    return;
  }
  if (what === "link") {
    const folder = tauri ? await invoke("pick_folder", { start: null }) : prompt("Folder with multiparity-plugin.json:");
    if (!folder) return;
    try { const p = await api("/plugins/link", { method: "POST", body: JSON.stringify({ folder }) }); toast(`Linked ${p.manifest.name}. Edits in ${folder} apply straight away.`); await loadPlugins(); }
    catch (err) { toast(err.message, true); }
    return;
  }
  const id = el.dataset.id;
  const p = plugins.list.find((x) => x.id === id);
  if (what === "toggle") {
    try { await api(`/plugins/${encodeURIComponent(id)}`, { method: "PATCH", body: JSON.stringify({ enabled: !p.enabled }) }); await loadPlugins(); }
    catch (err) { toast(err.message, true); }
    return;
  }
  if (what === "remove") {
    if (!(await confirmBox(`Remove ${p.manifest.name}?`, "Its hooks, operations and views go. Files it made stay where they are."))) return;
    try { await api(`/plugins/${encodeURIComponent(id)}`, { method: "DELETE" }); toast(`Removed ${p.manifest.name}.`); await loadPlugins(); }
    catch (err) { toast(err.message, true); }
    return;
  }
  if (what === "update") {
    el.disabled = true; el.textContent = "Checking…";
    try {
      const st = await api(`/plugins/${encodeURIComponent(id)}/update`, { method: "POST" });
      if (st.up_to_date) toast(`${p.manifest.name} is up to date.`);
      else { box.innerHTML = previewHtml(st, "Update"); box.scrollIntoView({ block: "nearest" }); }
    } catch (err) { toast(err.message, true); }
    el.disabled = false; el.textContent = "Check for update";
  }
});
$("#v-plugins").addEventListener("input", (e) => {
  if (e.target.id === "pl-q") {
    plugins.q = e.target.value;
    const at = e.target.selectionStart;
    renderPlugins();
    const q = $("#pl-q"); if (q) { q.focus(); q.setSelectionRange(at, at); }
  }
});
$("#v-plugins").addEventListener("keydown", (e) => {
  if (e.target.id === "src-loc" && e.key === "Enter") $('#v-plugins [data-pl="src-add"]').click();
  if (e.target.id === "pl-spec" && e.key === "Enter") $('#v-plugins [data-pl="look"]').click();
  if ((e.key === "Enter" || e.key === " ") && e.target.matches("[role=switch]")) { e.preventDefault(); e.target.closest("[data-pl]").click(); }
});
$("#plugnav").addEventListener("click", (e) => { const b = e.target.closest("[data-plugview]"); if (b) openPluginView(b.dataset.plugview); });

loadPlugins();
