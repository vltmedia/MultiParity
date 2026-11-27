<script setup lang="ts">
import { withBase } from 'vitepress';

const RELEASES = 'https://github.com/vltmedia/MultiParity/releases/latest';
const REPO = 'https://github.com/vltmedia/MultiParity';

const rows = [
  { name: 'hero_rig.blend', dir: 'Characters/Hero', st: 'theirs', label: 'Claimed by Mara', size: '412 MB', thumb: 'rig' },
  { name: 'hero_diffuse.png', dir: 'Characters/Hero', st: 'edited', label: 'Edited, yours', size: '64 MB', thumb: 'tex', ticked: true },
  { name: 'forge_interior.blend', dir: 'Environments/Forge', st: 'edited', label: 'Edited, yours', size: '1.9 GB', thumb: 'env', ticked: true },
  { name: 'wolf_howl_03.wav', dir: 'Audio/Creatures', st: 'behind', label: 'Newer from Dev', size: '8.2 MB', thumb: 'wav' },
  { name: 'forge_door.fbx', dir: 'Environments/Forge', st: 'ok', label: 'Up to date', size: '23 MB', thumb: 'mesh' },
];

const words = [
  ['Claim', 'Reserve a file so nobody else can publish it', 'Check out'],
  ['Publish', 'Send your edits to everyone as one numbered change', 'Submit'],
  ['Get latest', 'Bring your folder up to date', 'Sync'],
  ['Checkpoint', 'A private save point on your computer', '(a Git commit)'],
  ['Save draft', 'Unfinished work on the server, only for you', 'Shelve'],
  ['Rewind', 'Put files back as they were at a checkpoint', 'Revert'],
];

const day = [
  ['Get latest', 'The app tells you when someone publishes to your branch. One click brings your folder up to date, and only the parts of files that changed come down.'],
  ['Start editing', 'Open the .blend and change it. MultiParity claims it the moment it changes, so nobody else can publish over your work. If Mara has it, you see that before you start, and can ask her for it.'],
  ['Save checkpoints', 'Before the risky retopo, save a checkpoint. It is private, costs almost nothing for big files, and you can rewind to it whenever you like.'],
  ['Publish', 'Tick the files, say what you changed, and it goes to everyone as one change. Your claims are handed back. Every earlier version stays one right-click away.'],
];

const pieces = [
  ['Server', 'One Docker Compose file on a studio machine or in the cloud. File data on its disks, a NAS or any S3 storage, nightly backups, email, webhooks. Unlimited people.', withBase('/guide/server')],
  ['Desktop app', 'Windows and macOS. Every project from every account, the folders you choose, claims on first edit, checkpoints, branches, drafts and notifications. Updates itself.', RELEASES],
  ['Web app', 'Files with real thumbnails of images and Blender scenes, history, older versions, branches and administration, from any browser.', withBase('/guide/using#the-web-app')],
  ['Command line', '`mp` for build machines and scripts, with Perforce words as aliases, JSON output, and `mp watch` to run a script whenever something happens.', withBase('/guide/using#the-command-line')],
  ['Blender add-on', 'The open file’s state in the sidebar: claim, publish, checkpoint, get latest and older versions without leaving Blender.', withBase('/docs/AUTOMATION#plugins-blender')],
  ['AI assistants', '`mp mcp` lets Claude and other assistants find files, read the history and see who has what claimed.', withBase('/docs/AUTOMATION#ai-assistants-mp-mcp')],
];
</script>

<template>
  <div class="mp">
    <section class="hero">
      <div class="hero-copy">
        <h1>Big files.<br />Clear claims.<br /><span>Nothing overwritten.</span></h1>
        <p class="lede">
          Self-hosted version control for game and art teams. A friendlier Perforce: every version of every scene, texture and
          render, and apps your artists will actually use.
        </p>
        <div class="ctas">
          <a class="btn pri" :href="RELEASES">Download the desktop app</a>
          <a class="btn" :href="withBase('/guide/server')">Run a server</a>
        </div>
        <p class="fine">Open source. Windows and macOS apps, a web app, and a server you run with Docker.</p>
      </div>

      <figure class="panel-wrap" aria-label="The MultiParity desktop app's list of changed files">
      <div class="panel">
        <div class="panel-head">
          <b>Emberfall</b>
          <span>Main, up to change 4812</span>
        </div>
        <ul class="files">
          <li v-for="r in rows" :key="r.name" :class="r.st">
            <span class="tick" :class="{ on: r.ticked }" aria-hidden="true"></span>
            <span class="thumb" :class="r.thumb" aria-hidden="true"></span>
            <span class="fname"><strong>{{ r.name }}</strong><small>{{ r.dir }}</small></span>
            <span class="st">{{ r.label }}</span>
            <span class="size">{{ r.size }}</span>
          </li>
        </ul>
        <div class="panel-foot">
          <span class="msg">Hero diffuse pass and forge lighting</span>
          <span class="pub">Publish 2 files</span>
        </div>
      </div>
        <div class="toast" role="presentation">
          <b>Dev published change 4813</b>
          <span>wolf_howl_03.wav: louder tail</span>
        </div>
      </figure>
    </section>

    <section class="words">
      <div class="sec-head">
        <h2>Plain words for the same ideas</h2>
        <p>Coming from Perforce? Nothing to unlearn, just friendlier names.</p>
      </div>
      <table>
        <thead><tr><th>MultiParity</th><th>What it does</th><th>Perforce</th></tr></thead>
        <tbody>
          <tr v-for="w in words" :key="w[0]"><td><strong>{{ w[0] }}</strong></td><td>{{ w[1] }}</td><td class="p4">{{ w[2] }}</td></tr>
        </tbody>
      </table>
    </section>

    <section class="day">
      <div class="sec-head">
        <h2>A working day</h2>
        <p>What an artist does, start to finish.</p>
      </div>
      <ol>
        <li v-for="(d, i) in day" :key="d[0]">
          <span class="n">{{ i + 1 }}</span>
          <h3>{{ d[0] }}</h3>
          <p>{{ d[1] }}</p>
        </li>
      </ol>
    </section>

    <section class="pieces">
      <div class="sec-head">
        <h2>Everything a studio needs</h2>
        <p>One server, and the tools people already reach for.</p>
      </div>
      <dl>
        <a v-for="p in pieces" :key="p[0]" :href="p[2]" class="piece">
          <dt>{{ p[0] }}</dt>
          <dd v-html="p[1].replace(/`([^`]+)`/g, '<code>$1</code>')"></dd>
        </a>
      </dl>
    </section>

    <section class="big">
      <div class="big-copy">
        <h2>Built for files measured in gigabytes</h2>
        <p>
          Files are cut into parts by their content and stored once. Change a corner of a 4 GB scene and only the parts that
          changed travel and get stored. Checkpoints keep only what the server does not already have, and render output is
          left out of them.
        </p>
        <p>
          Keep different projects on different storage: the server's disks, a NAS, or any S3 bucket, and move a project between
          them while people work. Nightly backups copy only what is new.
        </p>
      </div>
      <div class="start">
        <h3>Run your own in minutes</h3>
        <pre><code>git clone {{ REPO }}.git
cd MultiParity
cp .env.example .env  <i># then edit it</i>
docker compose up -d</code></pre>
        <a class="btn pri" :href="withBase('/guide/server')">Read the server guide</a>
      </div>
    </section>
  </div>
</template>

<style scoped>
.mp {
  --cut: polygon(var(--mp-cut) 0, 100% 0, 100% calc(100% - var(--mp-cut)), calc(100% - var(--mp-cut)) 100%, 0 100%, 0 var(--mp-cut));
  max-width: 1160px;
  margin: 0 auto;
  overflow-x: clip;
  padding: 0 24px 96px;
  color: var(--mp-ink);
}
h1, h2, h3 { font-family: 'Oxanium', var(--vp-font-family-base); color: var(--mp-ink); }

.btn {
  display: inline-block; padding: 11px 20px; font-weight: 600; font-size: 15px; text-decoration: none;
  border: 1px solid var(--mp-acc); color: var(--mp-ink); background: var(--mp-card); clip-path: var(--cut);
  transition: background 0.15s;
}
.btn:hover { background: var(--vp-c-brand-soft); }
.btn.pri { background: var(--mp-acc); color: var(--mp-acc-ink); }
.btn.pri:hover { filter: brightness(1.08); background: var(--mp-acc); }
.btn:focus-visible { outline: 2px solid var(--mp-hold); outline-offset: 3px; }

/* ---------- hero ---------- */
.hero {
  display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1.05fr); gap: 56px; align-items: center;
  padding: 72px 0 88px; position: relative; isolation: isolate;
}
.hero::before {
  content: ''; position: absolute; inset: 0 -200px; z-index: -1;
  background-image: linear-gradient(var(--mp-grid) 1px, transparent 1px), linear-gradient(90deg, var(--mp-grid) 1px, transparent 1px);
  background-size: 16px 16px;
  mask-image: linear-gradient(to bottom, #000 70%, transparent);
}
.hero h1 { font-size: clamp(40px, 6vw, 68px); line-height: 1.02; font-weight: 800; letter-spacing: -0.025em; margin: 0; }
.hero h1 span { color: var(--mp-acc); }
.lede { font-size: 19px; line-height: 1.55; color: var(--vp-c-text-2); margin: 24px 0 30px; max-width: 34em; }
.ctas { display: flex; gap: 12px; flex-wrap: wrap; }
.fine { margin-top: 18px; font-size: 13.5px; color: var(--mp-mute); }

/* The file panel: the desktop app's Your work, drawn in its own look. */
.panel-wrap { margin: 0; position: relative; padding-top: 30px; }
.panel {
  position: relative; background: var(--mp-card); border: 1px solid var(--mp-acc);
  box-shadow: 0 0 0 5px var(--vp-c-brand-soft), 0 30px 60px -30px rgba(11, 42, 64, 0.35);
  clip-path: polygon(14px 0, 100% 0, 100% calc(100% - 14px), calc(100% - 14px) 100%, 0 100%, 0 14px);
}
.panel-head { display: flex; justify-content: space-between; align-items: baseline; padding: 14px 18px; border-bottom: 1px solid var(--mp-line); }
.panel-head b { font-family: 'Oxanium', sans-serif; font-size: 17px; }
.panel-head span, .size, .fname small { font-family: var(--vp-font-family-mono); font-size: 11px; color: var(--mp-mute); }
.files { list-style: none; margin: 0; padding: 0; }
.files li { display: grid; grid-template-columns: 16px 34px minmax(0, 1fr) auto 64px; gap: 12px; align-items: center; padding: 10px 18px; border-top: 1px solid var(--mp-line); }
.files li:first-child { border-top: 0; }
.tick { width: 14px; height: 14px; border: 1px solid var(--mp-mute); }
.tick.on { background: var(--mp-acc); border-color: var(--mp-acc); box-shadow: inset 0 0 0 3px var(--mp-card); }
.thumb { width: 34px; height: 34px; border: 1px solid var(--mp-line); }
.thumb.rig { background: radial-gradient(circle at 50% 36%, #c9d6e2 0 14%, transparent 15%), linear-gradient(#6a7f95, #2e4052); }
.thumb.tex { background: conic-gradient(from 30deg, #b34a2c, #e2a45a, #8a3a22, #d9825a, #b34a2c); }
.thumb.env { background: linear-gradient(160deg, #ffb35c 0 30%, #803a1c 30% 55%, #2a1a14 55%); }
.thumb.wav { background: repeating-linear-gradient(90deg, var(--mp-acc) 0 2px, transparent 2px 5px) center / 100% 60% no-repeat, var(--mp-card); }
.thumb.mesh { background: linear-gradient(135deg, transparent 45%, var(--mp-mute) 45% 50%, transparent 50%), linear-gradient(45deg, transparent 45%, var(--mp-mute) 45% 50%, transparent 50%), var(--vp-c-bg-soft); }
.fname { min-width: 0; display: flex; flex-direction: column; }
.fname strong { font-size: 14px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.st { font-size: 12.5px; font-weight: 600; padding: 2px 8px; border: 1px solid currentColor; white-space: nowrap; }
.theirs .st { color: var(--mp-hold); background: var(--mp-hold-bg); }
.edited .st { color: var(--mp-acc); }
.behind .st { color: var(--mp-new); }
.ok .st { color: var(--mp-mute); border-color: transparent; }
.size { text-align: right; }
.panel-foot { display: flex; gap: 10px; padding: 14px 18px; border-top: 1px solid var(--mp-line); background: var(--vp-c-bg-soft); }
.msg { flex: 1; padding: 9px 12px; border: 1px solid var(--mp-line); background: var(--mp-card); font-size: 14px; }
.pub { padding: 9px 16px; background: var(--mp-acc); color: var(--mp-acc-ink); font-weight: 600; font-size: 14px; clip-path: var(--cut); }
/* The one moment of motion: a notification arrives once. */
.toast {
  position: absolute; right: -16px; top: 0; z-index: 2; display: flex; flex-direction: column; gap: 2px;
  padding: 10px 14px; background: var(--mp-ink); color: var(--mp-paper); font-size: 13px; min-width: 230px;
  box-shadow: 0 14px 30px -12px rgba(0, 0, 0, 0.45);
  animation: arrive 0.5s 1.4s cubic-bezier(0.2, 0.8, 0.2, 1) both;
}
.toast b { font-weight: 600; }
.toast span { opacity: 0.75; font-size: 12px; }
@keyframes arrive { from { opacity: 0; transform: translateY(-10px); } to { opacity: 1; transform: none; } }
@media (prefers-reduced-motion: reduce) { .toast { animation: none; } }

/* ---------- sections ---------- */
section + section { margin-top: 96px; }
.sec-head { margin-bottom: 28px; }
.sec-head h2 { font-size: clamp(26px, 3.2vw, 34px); font-weight: 700; margin: 0 0 8px; letter-spacing: -0.015em; }
.sec-head p { color: var(--vp-c-text-2); margin: 0; font-size: 17px; }

.words table { width: 100%; border-collapse: collapse; display: table; font-size: 15.5px; }
.words th { text-align: left; font-family: 'Oxanium', sans-serif; font-weight: 600; font-size: 13px; color: var(--mp-mute); padding: 0 16px 10px 0; border-bottom: 1px solid var(--mp-acc); }
.words td { padding: 14px 16px 14px 0; border-bottom: 1px solid var(--mp-line); vertical-align: top; }
.words td strong { font-family: 'Oxanium', sans-serif; font-weight: 700; font-size: 17px; }
.words td.p4 { color: var(--mp-mute); font-family: var(--vp-font-family-mono); font-size: 13px; }

.day ol { list-style: none; padding: 0; margin: 0; display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); border-top: 1px solid var(--mp-acc); }
.day li { padding: 22px 22px 0 0; position: relative; }
.day li + li { padding-left: 22px; border-left: 1px solid var(--mp-line); }
.day .n { font-family: 'Oxanium', sans-serif; font-weight: 800; font-size: 44px; line-height: 1; color: var(--mp-acc); }
.day h3 { font-size: 19px; margin: 12px 0 8px; font-weight: 700; }
.day p { color: var(--vp-c-text-2); font-size: 15px; line-height: 1.6; margin: 0; }

.pieces dl { margin: 0; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); border-top: 1px solid var(--mp-line); }
.piece { display: grid; grid-template-columns: 170px minmax(0, 1fr); gap: 20px; padding: 20px 20px 20px 0; border-bottom: 1px solid var(--mp-line); text-decoration: none; color: inherit; }
.piece:nth-child(odd) { border-right: 1px solid var(--mp-line); }
.piece:nth-child(even) { padding-left: 20px; }
.piece dt { font-family: 'Oxanium', sans-serif; font-weight: 700; font-size: 18px; color: var(--mp-ink); }
.piece dd { margin: 0; color: var(--vp-c-text-2); font-size: 15px; line-height: 1.6; }
.piece:hover dt { color: var(--mp-acc); }
.piece :deep(code) { font-size: 0.86em; padding: 1px 5px; background: var(--vp-c-bg-soft); border: 1px solid var(--mp-line); }

.big { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 48px; align-items: start; }
.big h2 { font-size: clamp(26px, 3.2vw, 34px); font-weight: 700; margin: 0 0 16px; }
.big p { color: var(--vp-c-text-2); font-size: 16.5px; line-height: 1.65; }
.start { border: 1px solid var(--mp-acc); background: var(--mp-card); padding: 24px; clip-path: polygon(14px 0, 100% 0, 100% calc(100% - 14px), calc(100% - 14px) 100%, 0 100%, 0 14px); }
.start h3 { margin: 0 0 14px; font-size: 19px; font-weight: 700; }
.start pre { margin: 0 0 18px; padding: 16px; background: var(--vp-c-bg-soft); border: 1px solid var(--mp-line); overflow-x: auto; }
.start code { font-family: var(--vp-font-family-mono); font-size: 12px; line-height: 1.75; color: var(--mp-ink); }
.start i { font-style: normal; color: var(--mp-mute); }

@media (max-width: 960px) {
  .hero { grid-template-columns: 1fr; gap: 40px; padding: 48px 0 64px; }
  .day ol { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .day li:nth-child(3) { padding-left: 0; border-left: 0; }
  .day li { padding-bottom: 24px; }
  .big { grid-template-columns: 1fr; }
}
@media (max-width: 640px) {
  .mp { padding: 0 16px 72px; }
  .files li { grid-template-columns: 30px minmax(0, 1fr) auto; }
  .files .tick, .files .size { display: none; }
  .toast { right: 8px; left: auto; min-width: 0; }
  .day ol { grid-template-columns: 1fr; }
  .day li, .day li + li { padding: 20px 0; border-left: 0; border-bottom: 1px solid var(--mp-line); }
  .pieces dl { grid-template-columns: 1fr; }
  .piece { grid-template-columns: 1fr; gap: 6px; }
  .piece:nth-child(odd) { border-right: 0; }
  .piece:nth-child(even) { padding-left: 0; }
  .words td.p4, .words th:last-child { display: none; }
  section + section { margin-top: 72px; }
}
</style>
