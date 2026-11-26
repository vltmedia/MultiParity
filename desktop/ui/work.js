// MultiParity desktop: Your work (what is unpublished in one synced folder, and publishing it)
// and Checkpoints (private save points, and rewinding to one). Uses helpers from app.js.
"use strict";

var work = {
  wsId: null,
  ws: null,
  project: null,
  account: null,
  status: null,
  left: new Set(), // unticked: stays out of this change
  message: "",
  keepMine: new Set(), // stale files the person chose to publish over the server's newer version
  conflict: null,
  busy: null,
  timer: null,
  tab: "work", // work, files, history, branches, drafts (project.js)
  close() {
    clearInterval(this.timer);
    this.timer = null;
  },
};

// ------------------------------------------------------------------ small helpers

const baseName = (p) => p.split("/").pop();
const dirName = (p) => p.split("/").slice(0, -1).join("/");
const plural = (n, word, many) => `${n} ${n === 1 ? word : many || word + "s"}`;

function ago(iso) {
  const t = new Date(iso);
  const s = (Date.now() - t.getTime()) / 1000;
  if (s < 50) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  if (s < 86400 && t.getDate() === new Date().getDate()) return `today ${t.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
  if (s < 7 * 86400) return t.toLocaleDateString([], { weekday: "long" }) + " " + t.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  return t.toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" });
}

function toast(text, bad) {
  const el = $("#toast");
  el.textContent = text;
  el.classList.toggle("bad", !!bad);
  el.hidden = false;
  clearTimeout(toast.t);
  toast.t = setTimeout(() => (el.hidden = true), bad ? 7000 : 4000);
}

function files() {
  const st = work.status;
  if (!st) return [];
  return [
    ...st.edited.map((f) => ({ ...f, kind: "edited" })),
    ...st.new.map((f) => ({ ...f, kind: "new" })),
    ...st.deleted.map((f) => ({ ...f, kind: "deleted" })),
  ];
}

const ticked = () => files().filter((f) => !work.left.has(f.path));

// ------------------------------------------------------------------ opening

async function openWork(wsId) {
  work.close();
  if (work.wsId !== wsId) {
    work.left = new Set();
    work.message = "";
    work.keepMine = new Set();
    work.conflict = null;
    work.status = null;
    work.tab = "work";
    proj.reset();
  }
  work.wsId = wsId;
  for (const g of state.groups) for (const p of g.projects) for (const w of p.workspaces) if (w.id === wsId) Object.assign(work, { ws: w, project: p.project, account: g.account });
  showView("work");
  renderWork();
  await refreshWork();
  work.timer = setInterval(() => { if (!document.hidden && !$("#dlg").open && $("#ctx").hidden && !work.busy) refreshWork(false); }, 10000);
}

async function refreshWork(force = true) {
  const before = JSON.stringify([work.status, work.error]);
  try {
    work.status = await api(`/workspaces/${work.wsId}/status`);
    state.statuses[work.wsId] = work.status;
    const present = new Set(files().map((f) => f.path));
    for (const p of [...work.left]) if (!present.has(p)) work.left.delete(p);
    work.error = null;
  } catch (e) {
    work.error = e.message;
  }
  // The timed refresh redraws only when something changed, so typing is not interrupted.
  if (force || JSON.stringify([work.status, work.error]) !== before) renderWork();
  renderHere();
}

// ------------------------------------------------------------------ rendering

function renderWork() {
  const v = $("#v-work");
  if (!work.ws) { v.innerHTML = ""; return; }
  // Keep what is being typed and where the cursor is.
  const msg = $("#w-msg");
  if (msg) work.message = msg.value;
  const focused = document.activeElement && document.activeElement.id === "w-msg";
  const q = $("#p-q");
  const qFocused = q && document.activeElement === q;
  const qText = q ? q.value : null;
  const st = work.status;
  const all = files();
  const on = ticked();
  const bytes = on.reduce((a, f) => a + (f.kind === "deleted" ? 0 : f.size), 0);

  v.innerHTML = `
    <header class="head">
      <div>
        <button class="back" data-w="back">Projects</button>
        <h1>${esc(work.project.name)}</h1>
        <p>${esc(work.project.org.name)} · branch ${esc(work.ws.branch_name)} · <span class="path">${esc(work.ws.root)}</span></p>
      </div>
      <div class="head-acts">
        <button class="btn ${st && st.behind ? "pri" : ""}" data-w="latest" ${work.busy ? "disabled" : ""}>${work.busy === "latest" ? "Getting latest…" : st && st.behind ? `Get latest (${st.behind} newer)` : "Get latest"}</button>
        <button class="btn" data-w="web">Open on the web</button>
        <button class="btn" data-w="move-folder">Move folder…</button>
      </div>
    </header>
    ${work.error ? `<div class="notice bad">${esc(work.error)}</div>` : ""}
    ${st && st.checkpoint_notice ? `<div class="notice hold">${esc(st.checkpoint_notice)}</div>` : ""}
    ${tabsHtml()}
    ${work.tab !== "work" ? `<div class="ptab">${tabHtml()}</div>` : `
    <div class="work">
      <section class="changes" aria-label="Your changes">
        <div class="bar">
          <h2>Your changes</h2>
          <span>${st ? (all.length ? `${plural(all.length, "file")}${st.ignored ? `, ${st.ignored} ignored` : ""}` : "") : "Reading your folder…"}</span>
          ${all.length > 1 ? `<button class="link" data-w="all">${work.left.size ? "Tick all" : "Untick all"}</button>` : ""}
        </div>
        ${st && !all.length ? `<div class="empty"><b>Nothing to publish.</b>Edits, new files and deletions in ${esc(work.ws.root)} show up here by themselves.</div>` : ""}
        <ul class="files">${all.map(fileRow).join("")}</ul>
        ${all.length ? `<p class="note">Right-click a file for more: discard it, or leave it out of checkpoints.</p>` : ""}
      </section>
      <aside class="publish" aria-label="Publish">
        <h2>Publish</h2>
        <p class="note">${!all.length ? "Nothing to send yet." : on.length ? `${plural(on.length, "file")} going to ${esc(work.ws.branch_name)}, ${human(bytes)} to send.` : "Nothing ticked. Tick the files you want to publish."}</p>
        <label class="sr" for="w-msg">Say what you changed</label>
        <textarea id="w-msg" placeholder="Say what you changed" ${all.length ? "" : "disabled"}>${esc(work.message)}</textarea>
        ${conflictHtml()}
        <button class="btn pri" data-w="publish" ${!on.length || work.busy ? "disabled" : ""}>${work.busy === "publish" ? "Publishing…" : on.length ? `Publish ${plural(on.length, "file")}` : "Publish"}</button>
        <div class="two">
          <button class="btn" data-w="checkpoint" ${!all.length || work.busy ? "disabled" : ""}>Save checkpoint</button>
          <button class="btn" data-w="discard" ${!on.length || work.busy ? "disabled" : ""}>Discard…</button>
        </div>
        <p class="note">Publishing sends the ticked files to everyone as one change and releases your claims on them. Unticked files stay as they are.</p>
      </aside>
    </div>
    ${checkpointsHtml()}`}`;

  const m = $("#w-msg");
  if (m && focused) { m.focus(); m.setSelectionRange(m.value.length, m.value.length); }
  const q2 = $("#p-q");
  if (q2 && qText !== null) { q2.value = qText; if (qFocused) { q2.focus(); q2.setSelectionRange(qText.length, qText.length); } }
}

function fileRow(f) {
  const off = work.left.has(f.path);
  const label = f.kind === "new" ? "New" : f.kind === "deleted" ? "Deleted" : "Edited";
  const stale = work.conflict && work.conflict.stale.includes(f.path);
  return `<li class="${off ? "off" : ""} ${stale ? "stale" : ""}" data-path="${esc(f.path)}" data-kind="${f.kind}" tabindex="0">
    <input type="checkbox" data-w="tick" aria-label="Publish ${esc(baseName(f.path))}" ${off ? "" : "checked"}>
    <span class="tag ${f.kind}">${label}</span>
    <div class="name"><strong>${esc(baseName(f.path))}</strong><small>${esc(dirName(f.path) || "Top folder")}</small></div>
    <span class="meta">${f.claimed ? `<em>Claimed</em>` : ""}${f.kind === "deleted" ? "" : human(f.size)}</span>
    <button class="more" data-w="menu" aria-label="More for ${esc(baseName(f.path))}" aria-haspopup="menu">···</button>
  </li>`;
}

function conflictHtml() {
  const c = work.conflict;
  if (!c) return "";
  const why = c.claimed.length ? "someone else has a file claimed" : c.stale.length ? "a file changed on the server" : c.message;
  let html = `<div class="notice bad conflict"><b>Nothing was published: ${esc(why)}.</b>`;
  if (c.claimed.length) {
    html += `<ul>${c.claimed.map((x) => `<li><span>${esc(baseName(x.path))}</span> is claimed by ${esc((x.user && x.user.name) || "someone")}. Untick it, or ask them to release it.</li>`).join("")}</ul>`;
  }
  if (c.stale.length) {
    html += `<ul>${c.stale.map((p) => `<li><span>${esc(baseName(p))}</span> changed on the server since you got it.</li>`).join("")}</ul>`;
    html += c.gotLatest
      ? `<p>You now have the latest list. Publish yours over theirs, or discard yours to keep theirs.</p>
         <div class="two"><button class="btn sm" data-w="keep-mine">Publish mine over theirs</button><button class="btn sm" data-w="discard-stale">Keep theirs</button></div>`
      : `<p>Get latest first: your edited copies are kept, nothing is overwritten.</p><button class="btn sm" data-w="latest">Get latest</button>`;
  }
  return html + `</div>`;
}

function checkpointsHtml() {
  const st = work.status;
  if (!st) return "";
  const where = st.checkpoint_storage === "server" ? "on the server, where only you can see them" : "on this computer";
  const cps = st.checkpoints || [];
  return `<section class="cps" aria-label="Checkpoints">
    <div class="bar"><h2>Checkpoints</h2><button class="link" data-w="space">Space used</button></div>
    <p class="note">Private save points of your unpublished work, kept ${where}. ${st.auto_checkpoints ? "Taken automatically while you work, and when you save one." : "Taken when you save one, and before discarding or rewinding so those can be undone."} Render output and caches are left out.</p>
    ${cps.length ? `<ul class="cplist">${cps.map(cpRow).join("")}</ul>` : `<div class="empty"><b>No checkpoints yet.</b>Save one before trying something risky, and you can come back to it.</div>`}
  </section>`;
}

function cpRow(c) {
  const title = c.note || (c.auto ? "Automatic checkpoint" : "Checkpoint");
  const tags = [c.auto ? "Automatic" : "", c.system ? "Undo point" : "", c.on_server ? "On the server" : ""].filter(Boolean);
  return `<li>
    <div class="when"><strong>${esc(ago(c.created_at))}</strong><small>${esc(new Date(c.created_at).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }))}</small></div>
    <div><strong>${esc(title)}</strong><small>${plural(c.files, "file")}${tags.length ? " · " + tags.map(esc).join(" · ") : ""}</small></div>
    <button class="btn sm" data-w="rewind" data-cp="${esc(c.id)}" ${work.busy ? "disabled" : ""}>Rewind…</button>
  </li>`;
}

// ------------------------------------------------------------------ actions

async function run(name, fn) {
  work.busy = name;
  renderWork();
  try { return await fn(); }
  finally { work.busy = null; await refreshWork(); }
}

async function publish(retryKeep) {
  const msg = ($("#w-msg").value || "").trim();
  work.message = $("#w-msg").value;
  if (!msg) { toast("Say what you changed first, so the team knows what this change is.", true); $("#w-msg").focus(); return; }
  const paths = ticked().map((f) => f.path);
  const keep = retryKeep ? [...work.keepMine] : [];
  await run("publish", async () => {
    try {
      const r = await api(`/workspaces/${work.wsId}/publish`, { method: "POST", body: JSON.stringify({ message: msg, paths, keep_mine: keep }) });
      toast(`Published change ${r.change.number}.`);
      work.message = "";
      $("#w-msg").value = "";
      work.left = new Set();
      work.conflict = null;
      work.keepMine = new Set();
    } catch (e) {
      if (e.status === 409 && e.details) {
        work.conflict = { message: e.message, stale: e.details.stale || [], claimed: e.details.claimed || [], gotLatest: false };
      } else toast(e.message, true);
    }
  });
}

async function getLatest() {
  await run("latest", async () => {
    try {
      const r = await api(`/workspaces/${work.wsId}/get-latest`, { method: "POST", body: "{}" });
      const kept = (r.kept_yours || []).length;
      toast(r.downloaded ? `Got latest: ${plural(r.downloaded, "file")}, ${human(r.bytes)}.${kept ? ` Kept your version of ${plural(kept, "file")}.` : ""}` : kept ? `Already up to date. Kept your version of ${plural(kept, "file")}.` : "Already up to date.");
      if (work.conflict && work.conflict.stale.length) work.conflict.gotLatest = true;
    } catch (e) { toast(e.message, true); }
  });
}

function saveCheckpoint() {
  dialog(`<h2>Save a checkpoint</h2>
    <p class="note">A private save point of your ${plural(files().length, "unpublished file")}, kept ${work.status.checkpoint_storage === "server" ? "on the server, where only you can see it" : "on this computer"}. You can rewind to it later.</p>
    <label class="field">Note<input id="cp-note" placeholder="before retopo" autofocus></label>
    <label class="check"><input type="checkbox" id="cp-all"> Include render output and caches too</label>
    <p class="err" id="cp-err" role="alert"></p>
    <div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn pri" value="save">Save checkpoint</button></div>`,
    async (_, body, dlg) => {
      const note = $("#cp-note", body).value.trim();
      const all = $("#cp-all", body).checked;
      try {
        const c = await api(`/workspaces/${work.wsId}/checkpoint`, { method: "POST", body: JSON.stringify({ note: note || null, all }) });
        dlg.close();
        toast(`Checkpoint saved${note ? `: ${note}` : ""}.${c.left_out ? ` Left out ${plural(c.left_out, "render or cache file")}.` : ""}`);
        refreshWork();
      } catch (e) { $("#cp-err", body).textContent = e.message; }
    });
  // Enter in the note saves (the form's first button is Cancel).
  $("#cp-note").addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); $('#dlg-body button[value="save"]').click(); } });
}

function discardDialog(paths) {
  const list = files().filter((f) => paths.includes(f.path));
  const newOnes = list.filter((f) => f.kind === "new").length;
  dialog(`<h2>Discard ${plural(list.length, "file")}?</h2>
    <p>Your edits go away and these files go back to the latest version this computer has from the server.${newOnes ? ` ${plural(newOnes, "new file")} ${newOnes === 1 ? "is" : "are"} moved to the folder's safety area instead.` : ""}</p>
    <p class="note">Your current versions are saved in a checkpoint first, so you can rewind to undo this.</p>
    <ul class="mini">${list.slice(0, 8).map((f) => `<li>${esc(f.path)}</li>`).join("")}${list.length > 8 ? `<li>and ${list.length - 8} more</li>` : ""}</ul>
    <p class="err" id="d-err" role="alert"></p>
    <div class="dlg-acts"><button class="btn" value="cancel">Keep my edits</button><button class="btn pri" value="go">Discard ${plural(list.length, "file")}</button></div>`,
    async (_, body, dlg) => {
      try {
        const r = await api(`/workspaces/${work.wsId}/discard`, { method: "POST", body: JSON.stringify({ paths }) });
        dlg.close();
        for (const p of paths) work.left.delete(p);
        if (work.conflict) { work.conflict.stale = work.conflict.stale.filter((p) => !paths.includes(p)); if (!work.conflict.stale.length && !work.conflict.claimed.length) work.conflict = null; }
        toast(`Discarded ${plural(r.files, "file")}.${r.undo ? " Rewind to the newest checkpoint to undo." : ""}`);
        refreshWork();
      } catch (e) { $("#d-err", body).textContent = e.message; }
    });
}

async function rewindDialog(cpId) {
  let c;
  try { c = await api(`/workspaces/${work.wsId}/checkpoints/${encodeURIComponent(cpId)}`); }
  catch (e) { toast(e.message, true); return; }
  const title = c.note || (c.auto ? "the automatic checkpoint" : "this checkpoint");
  dialog(`<h2>Rewind to ${esc(title)}</h2>
    <p class="note">Saved ${esc(ago(c.created_at))}. Ticked files go back to how they were then. Your current versions are saved in a checkpoint first, so you can undo this.</p>
    <ul class="pick">${c.files.map((f) => `<li><label><input type="checkbox" value="${esc(f.path)}" checked>
      <span>${esc(f.path)}</span><small>${f.deleted ? "Deleted then" : human(f.size)}</small></label></li>`).join("")}</ul>
    <p class="err" id="r-err" role="alert"></p>
    <div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn pri" value="go">Rewind</button></div>`,
    async (_, body, dlg) => {
      const boxes = [...body.querySelectorAll(".pick input")];
      const chosen = boxes.filter((b) => b.checked).map((b) => b.value);
      if (!chosen.length) { $("#r-err", body).textContent = "Tick at least one file."; return; }
      const paths = chosen.length === boxes.length ? [] : chosen;
      try {
        const r = await api(`/workspaces/${work.wsId}/rewind`, { method: "POST", body: JSON.stringify({ checkpoint: cpId, paths }) });
        dlg.close();
        toast(`Rewound ${plural(r.files, "file")}.${r.undo ? " To undo, rewind to the newest checkpoint." : ""}`);
        refreshWork();
      } catch (e) { $("#r-err", body).textContent = e.message; }
    });
}

/** Ignore or leave out of checkpoints, with choices built from the file's name and folder. */
async function patternDialog(path, kind) {
  let choices;
  try { choices = await api(`/workspaces/${work.wsId}/ignore/suggest?path=${encodeURIComponent(path)}`); }
  catch (e) { toast(e.message, true); return; }
  const ignore = kind === "ignore";
  dialog(`<h2>${ignore ? "Ignore" : "Leave out of checkpoints"}: ${esc(baseName(path))}</h2>
    <p class="note">${ignore
      ? "Ignored files stay on your computer and are never published. Choose how wide the rule is."
      : "Checkpoints copy your unpublished files so you can go back. Files that can be made again, like renders and caches, are better left out. Publishing is not affected."}</p>
    <ul class="choices">${choices.map((c, i) => `<li><label><input type="radio" name="pat" value="${esc(c.pattern)}" ${i === 0 ? "checked" : ""}>
      <span><strong>${esc(c.label)}</strong><code>${esc(c.pattern)}</code></span><small>${plural(c.matches, "file")}, ${human(c.bytes)}${ignore && c.tracked ? `, ${c.tracked} already published (ignoring does not remove them)` : ""}</small></label></li>`).join("")}</ul>
    <label class="check"><input type="checkbox" id="pat-personal"> Only on this computer, not for the whole project</label>
    <p class="err" id="p-err" role="alert"></p>
    <div class="dlg-acts"><button class="btn" value="cancel">Cancel</button><button class="btn pri" value="go">${ignore ? "Ignore" : "Leave out"}</button></div>`,
    async (_, body, dlg) => {
      const pat = (body.querySelector('input[name="pat"]:checked') || {}).value;
      const personal = $("#pat-personal", body).checked;
      try {
        await api(`/workspaces/${work.wsId}/${ignore ? "ignore" : "no-checkpoint"}`, { method: "POST", body: JSON.stringify({ pattern: pat, personal }) });
        dlg.close();
        toast(`${ignore ? "Ignoring" : "Checkpoints now leave out"} ${pat}.${personal ? "" : " The rule is shared with the project when you publish."}`);
        refreshWork();
      } catch (e) { $("#p-err", body).textContent = e.message; }
    });
}

async function spaceDialog() {
  let r;
  try { r = await api(`/workspaces/${work.wsId}/space`); } catch (e) { toast(e.message, true); return; }
  dialog(`<h2>Space used by ${esc(work.project.name)}</h2>
    <ul class="adopt">
      <li><b>${human(r.kept_bytes)}</b><div><strong>Checkpoints on this computer</strong><small>Limit ${human(r.limit_bytes)}${r.limit_is_automatic ? ", a tenth of the free disk" : ""}. ${r.storage === "server" ? `${plural(r.on_server, "checkpoint")} kept on the server use no space here.` : ""}</small></div></li>
      <li><b>${human(r.safety_bytes)}</b><div><strong>Files moved aside</strong><small>Instead of being overwritten. Kept 30 days.</small></div></li>
      <li style="--k:${r.low_disk ? "var(--warn)" : "var(--new)"}"><b>${human(r.free_disk_bytes)}</b><div><strong>Free on this disk</strong><small>${r.low_disk ? "Nearly full." : "Plenty."}</small></div></li>
    </ul>
    ${r.freed_bytes || r.safety_freed_bytes ? `<p class="ok">Freed ${human(r.freed_bytes + r.safety_freed_bytes)} just now.</p>` : ""}
    <div class="dlg-acts"><button class="btn pri" value="cancel">Done</button></div>`, async () => {});
}

/** Every published version of one file, with three ways to get one back. */
async function versionsDialog(path) {
  let r;
  try { r = await api(`/workspaces/${work.wsId}/versions?path=${encodeURIComponent(path)}`); }
  catch (e) { toast(e.message, true); return; }
  const list = r.versions || [];
  const edited = files().some((f) => f.path === path && f.kind !== "new");
  const row = (v) => {
    const who = (v.author && v.author.name) || "someone";
    const gone = v.op === "delete";
    const elsewhere = v.branch_id !== work.ws.branch_id;
    return `<li class="${v.current ? "cur" : ""}">
      <div class="when"><strong>Change ${v.change}</strong><small>${esc(ago(v.created_at))}</small></div>
      <div class="what"><strong>${esc(v.message)}</strong><small>${esc(who)} · ${gone ? "deleted it" : human(v.size)}${elsewhere ? ` · on ${esc(v.branch_name)}` : ""}${v.current ? " · the latest" : ""}</small></div>
      <div class="acts">${gone ? "" : `
        ${v.current && !edited ? "" : `<button class="btn sm" value="folder:${v.change}" title="Put this version in your folder in place of the file. Publish it to make it the latest for everyone.">Put back</button>`}
        <button class="btn sm" value="copy:${v.change}" title="Save this version beside the file, leaving the file alone.">Save a copy</button>
        ${v.current ? "" : `<button class="btn sm" value="latest:${v.change}" title="Make this the latest on the server now, as a new change, and get it.">Make latest</button>`}`}
      </div>
    </li>`;
  };
  dialog(`<h2>Older versions of ${esc(baseName(path))}</h2>
    <p class="note">${list.length ? `${plural(list.length, "published version")}, newest first. Nothing is lost by going back: the newer versions stay in the history.` : "This file has not been published yet, so there are no older versions."}
    ${edited ? " You have edits to this file: putting a version back saves them in a checkpoint first." : ""}</p>
    <ul class="versions">${list.map(row).join("")}</ul>
    <p class="err" id="v-err" role="alert"></p>
    <div class="dlg-acts"><button class="btn pri" value="cancel">Done</button></div>`,
    async (value, body, dlg) => {
      const [to, n] = value.split(":");
      const change = Number(n);
      if (to === "latest" && !confirm(`Make change ${change}'s version of ${baseName(path)} the latest for everyone on ${work.ws.branch_name}?`)) return;
      body.querySelectorAll("button").forEach((b) => (b.disabled = true));
      try {
        const res = await api(`/workspaces/${work.wsId}/versions`, { method: "POST", body: JSON.stringify({ path, change, to }) });
        dlg.close();
        if (to === "folder") toast(`Put back ${baseName(path)} as of change ${change}. Publish it to make it the latest.${res.undo ? " Your version is in a checkpoint." : ""}`);
        else if (to === "copy") toast(`Saved a copy: ${res.saved_to}`);
        else toast(`Change ${res.change.number}: ${baseName(path)} as of change ${change} is the latest again.${res.kept_yours ? " Your folder keeps your edited copy." : ""}`);
        refreshWork();
      } catch (e) {
        body.querySelectorAll("button").forEach((b) => (b.disabled = false));
        $("#v-err", body).textContent = e.message;
      }
    });
}

// ------------------------------------------------------------------ context menu

function openMenu(li, x, y) {
  const path = li.dataset.path;
  const kind = li.dataset.kind;
  const off = work.left.has(path);
  const items = [
    [off ? "Put back in this change" : "Leave out of this change", () => { off ? work.left.delete(path) : work.left.add(path); renderWork(); }],
    ["Discard…", () => discardDialog([path])],
  ];
  if (kind !== "new") items.push(["Older versions…", () => versionsDialog(path)]);
  if (kind !== "deleted") items.push(["Leave out of checkpoints…", () => patternDialog(path, "nocheckpoint")]);
  if (kind === "new") items.push(["Ignore…", () => patternDialog(path, "ignore")]);
  if (tauri && kind !== "deleted") items.push(["Show in folder", () => invoke("reveal_path", { path: absPath(path) })]);
  showMenu(items, x, y, li);
}

/** A right-click menu of [label, action] items at x, y; Escape gives focus back to `back`. */
function showMenu(items, x, y, back) {
  const ul = $("#ctx");
  ul.innerHTML = items.map(([t], i) => `<li role="menuitem" tabindex="-1" data-i="${i}">${esc(t)}</li>`).join("");
  ul.hidden = false;
  const w = ul.offsetWidth, h = ul.offsetHeight;
  ul.style.left = `${Math.min(x, innerWidth - w - 8)}px`;
  ul.style.top = `${Math.min(y, innerHeight - h - 8)}px`;
  ul.onclick = (e) => { const i = e.target.dataset.i; if (i !== undefined) { closeMenu(); items[+i][1](); } };
  ul.onkeydown = (e) => {
    const all = [...ul.children]; const at = all.indexOf(document.activeElement);
    if (e.key === "ArrowDown") { e.preventDefault(); all[(at + 1) % all.length].focus(); }
    if (e.key === "ArrowUp") { e.preventDefault(); all[(at - 1 + all.length) % all.length].focus(); }
    if (e.key === "Enter") { e.preventDefault(); document.activeElement.click(); }
    if (e.key === "Escape") { closeMenu(); if (back) back.focus(); }
  };
  ul.firstElementChild.focus();
}

function closeMenu() { $("#ctx").hidden = true; }

// Close on a left press outside the menu. (Not on "click": some systems send one with the
// right-click that opened the menu.)
document.addEventListener("mousedown", (e) => { if (e.button === 0 && !e.target.closest("#ctx")) closeMenu(); });
window.addEventListener("blur", closeMenu);

// ------------------------------------------------------------------ wiring

$("#v-work").addEventListener("input", (e) => { if (e.target.id === "w-msg") work.message = e.target.value; });

$("#v-work").addEventListener("click", (e) => {
  const el = e.target.closest("[data-w]");
  if (!el) return;
  const what = el.dataset.w;
  const li = el.closest("li[data-path]");
  if (what === "tick") { const p = li.dataset.path; el.checked ? work.left.delete(p) : work.left.add(p); renderWork(); return; }
  if (what === "menu") { e.stopPropagation(); const r = el.getBoundingClientRect(); openMenu(li, r.left, r.bottom + 4); return; }
  if (what === "all") { work.left.size ? (work.left = new Set()) : files().forEach((f) => work.left.add(f.path)); renderWork(); return; }
  if (what === "back") { showView("projects"); return; }
  if (what === "web") {
    const path = `/p/${work.project.id}/today`;
    if (tauri) invoke("open_server_page", { accountId: work.account.id, path });
    else window.open(work.account.server + path);
    return;
  }
  if (what === "latest") return getLatest();
  if (what === "publish") return publish(false);
  if (what === "keep-mine") { work.conflict.stale.forEach((p) => work.keepMine.add(p)); return publish(true); }
  if (what === "discard-stale") return discardDialog(work.conflict.stale);
  if (what === "checkpoint") return saveCheckpoint();
  if (what === "discard") return discardDialog(ticked().map((f) => f.path));
  if (what === "rewind") return rewindDialog(el.dataset.cp);
  if (what === "space") return spaceDialog();
  if (what === "move-folder") return moveFolderDialog(work.ws, { project: work.project });
});

$("#v-work").addEventListener("contextmenu", (e) => {
  const li = e.target.closest(".files li[data-path]");
  if (!li) return;
  e.preventDefault();
  openMenu(li, e.clientX, e.clientY);
});

$("#v-work").addEventListener("keydown", (e) => {
  const li = e.target.closest(".files li[data-path]");
  if (li && e.target === li && (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10"))) {
    e.preventDefault();
    const r = li.getBoundingClientRect();
    openMenu(li, r.left + 40, r.bottom);
  }
  if (li && e.target === li && e.key === " ") { e.preventDefault(); const p = li.dataset.path; work.left.has(p) ? work.left.delete(p) : work.left.add(p); renderWork(); }
  if (e.target.id === "w-msg" && e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); publish(false); }
});
