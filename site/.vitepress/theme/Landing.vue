<script setup lang="ts">
import { computed, ref } from 'vue';
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

const shot = (name: string, scheme: 'light' | 'dark') => withBase(`/shots/${name}-${scheme}.webp`);

// The four things people notice first, each with the screen that shows it.
const features = [
  {
    shot: 'desk-work', app: 'Desktop app', w: 1360, h: 860,
    title: 'Everything you changed, in one list',
    body: 'Your work shows every file you edited, added or deleted, with a thumbnail and its size. Tick what is ready, say what you did, and publish it as one change. Checkpoints you saved along the way sit right below.',
    points: ['Claims taken for you on first edit', 'Checkpoints and rewind, private to you', 'Notified the moment someone publishes'],
  },
  {
    shot: 'web-files', app: 'Web app', w: 1600, h: 1000,
    title: 'See who has what before you open it',
    body: 'Every file shows its state: edited by you, claimed by someone else, new, or newer on the server. Blender scenes and images get real thumbnails, and the side bar shows who is working on what right now.',
    points: ['Claims you can see before you start', 'Tags like Approved or Needs review', 'Find any file by name'],
  },
  {
    shot: 'web-versions', app: 'Web app', w: 1600, h: 1000,
    title: 'Every version, one click back',
    body: 'Each file keeps every version ever published. Download an old one, put it back in your folder, or make it the latest again. Going back loses nothing: newer versions stay in the history.',
    points: ['Who changed it, when, and why', 'Download, Put back or Make latest', 'Stored once, however many versions'],
  },
  {
    shot: 'desk-projects', app: 'Desktop app', w: 1360, h: 860,
    title: 'Every project, every studio, one app',
    body: 'Sign in to as many servers as you work with. Each project lives in the folder you choose, and the app keeps them all up to date in the background from the tray.',
    points: ['Several accounts side by side', 'Folders anywhere on your disks', 'Updates itself'],
  },
];

// More screens, grouped by what they are for.
const gallery = [
  { key: 'today', label: 'Today', shot: 'web-today', w: 1600, h: 1000, cap: 'What happened today across the project: changes, claims and who is working on what.' },
  { key: 'history', label: 'History', shot: 'web-history', w: 1600, h: 1000, cap: 'Every change ever published, with its files and message, searchable.' },
  { key: 'branches', label: 'Branches', shot: 'desk-branches', w: 1360, h: 860, cap: 'Work on a lighting pass on its own branch, then bring it into Main when it is ready.' },
  { key: 'files', label: 'Files on disk', shot: 'desk-files', w: 1360, h: 860, cap: 'Browse the project folder from the desktop app, with state and thumbnails for every file.' },
  { key: 'admin', label: 'Administration', shot: 'web-admin', w: 1600, h: 1000, cap: 'People, roles, invites, storage and backups for the whole studio, from the browser.' },
  { key: 'look', label: 'Themes', shot: 'web-look', w: 1600, h: 1000, cap: 'Pick a look for yourself, or set one for an organization or a project.' },
];
const tab = ref(gallery[0].key);
const current = computed(() => gallery.find((g) => g.key === tab.value)!);
function onTabKey(e: KeyboardEvent, i: number) {
  const d = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
  if (!d) return;
  e.preventDefault();
  const next = gallery[(i + d + gallery.length) % gallery.length];
  tab.value = next.key;
  (document.getElementById(`tab-${next.key}`) as HTMLElement | null)?.focus();
}

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

    <figure class="studio">
      <picture>
        <source media="(max-width: 900px)" :srcset="withBase('/shots/studio-small.webp')" />
        <img :src="withBase('/shots/studio.webp')" width="2200" height="1228" loading="lazy"
          alt="An artist's desk at night: the MultiParity desktop app open on a laptop next to a drawing tablet" />
      </picture>
      <figcaption>
        <h2>Made for the people who make the files</h2>
        <p>Artists, animators and sound designers, not just the build engineer. No command line, no workspace specs, no locking rituals.</p>
      </figcaption>
    </figure>

    <section class="tour">
      <article v-for="(f, i) in features" :key="f.shot" class="feat" :class="{ flip: i % 2 === 1 }">
        <div class="feat-copy">
          <p class="app">{{ f.app }}</p>
          <h2>{{ f.title }}</h2>
          <p>{{ f.body }}</p>
          <ul><li v-for="p in f.points" :key="p">{{ p }}</li></ul>
        </div>
        <div class="frame" :class="f.app === 'Desktop app' ? 'desk' : 'web'">
          <div class="bar" aria-hidden="true"><i></i><i></i><i></i><span>{{ f.app === 'Desktop app' ? 'MultiParity' : 'mp.cinderworks.studio' }}</span></div>
          <img class="only-light" :src="shot(f.shot, 'light')" :width="f.w" :height="f.h" loading="lazy" :alt="`${f.app}: ${f.title}`" />
          <img class="only-dark" :src="shot(f.shot, 'dark')" :width="f.w" :height="f.h" loading="lazy" alt="" />
        </div>
      </article>
    </section>

    <section class="gallery">
      <div class="sec-head">
        <h2>More of the apps</h2>
        <p>Every screen comes in Ice Lab by day and Orbital HUD by night. Switch the site’s theme to see both.</p>
      </div>
      <div class="tabs" role="tablist" aria-label="Screens">
        <button v-for="(g, i) in gallery" :id="`tab-${g.key}`" :key="g.key" role="tab" type="button"
          :aria-selected="tab === g.key" :tabindex="tab === g.key ? 0 : -1" aria-controls="gallery-panel"
          @click="tab = g.key" @keydown="onTabKey($event, i)">{{ g.label }}</button>
      </div>
      <div id="gallery-panel" class="g-panel" role="tabpanel" :aria-labelledby="`tab-${current.key}`">
        <div class="frame" :class="current.shot.startsWith('desk') ? 'desk' : 'web'">
          <div class="bar" aria-hidden="true"><i></i><i></i><i></i><span>{{ current.shot.startsWith('desk') ? 'MultiParity' : 'mp.cinderworks.studio' }}</span></div>
          <img class="only-light" :src="shot(current.shot, 'light')" :width="current.w" :height="current.h" :alt="current.cap" />
          <img class="only-dark" :src="shot(current.shot, 'dark')" :width="current.w" :height="current.h" alt="" />
        </div>
        <p class="g-cap">{{ current.cap }}</p>
      </div>
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
  overflow-x: clip;
  padding: 0 24px 96px;
  color: var(--mp-ink);
}
.mp > section { max-width: 1112px; margin-left: auto; margin-right: auto; }
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

/* ---------- studio photo band ---------- */
.studio {
  position: relative; margin: 0 -24px; background: #050d14; overflow: hidden;
}
.studio img { display: block; width: 100%; height: clamp(360px, 52vw, 680px); object-fit: cover; object-position: 50% 40%; }
.studio figcaption {
  display: grid; grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr); gap: 48px; align-items: end;
  padding: 36px max(24px, calc(50vw - 556px)) 48px; background: #050d14;
}
.studio h2 { color: #eef6fb; font-size: clamp(26px, 3.2vw, 38px); font-weight: 700; line-height: 1.15; margin: 0; letter-spacing: -0.015em; }
.studio p { color: #b9cfdd; font-size: 17px; margin: 0; max-width: 34em; line-height: 1.55; }
.studio + section { margin-top: 112px; }

/* ---------- framed screenshots ---------- */
.frame {
  position: relative; background: var(--mp-card); border: 1px solid var(--mp-line);
  box-shadow: 0 1px 0 var(--mp-line), 0 40px 80px -40px rgba(11, 42, 64, 0.45);
  overflow: hidden;
}
.frame .bar { display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 12px; border-bottom: 1px solid var(--mp-line); background: var(--vp-c-bg-soft); }
.frame .bar i { width: 9px; height: 9px; border-radius: 50%; background: var(--mp-line); }
.frame .bar span { margin-left: 10px; font-family: var(--vp-font-family-mono); font-size: 11px; color: var(--mp-mute); }
.frame img { display: block; width: 100%; height: auto; }

/* ---------- feature tour ---------- */
.tour { display: flex; flex-direction: column; gap: 120px; }
.feat { display: grid; grid-template-columns: minmax(0, 0.78fr) minmax(0, 1.22fr); gap: 56px; align-items: center; }
.feat.flip .feat-copy { order: 2; }
.feat-copy .app { margin: 0 0 10px; font-family: 'Oxanium', sans-serif; font-weight: 600; font-size: 14px; color: var(--mp-acc); }
.feat-copy h2 { font-size: clamp(26px, 3vw, 36px); line-height: 1.12; font-weight: 700; margin: 0 0 16px; letter-spacing: -0.015em; }
.feat-copy > p:not(.app) { color: var(--vp-c-text-2); font-size: 16.5px; line-height: 1.65; margin: 0 0 20px; }
.feat-copy ul { list-style: none; margin: 0; padding: 0; border-top: 1px solid var(--mp-line); }
.feat-copy li { padding: 10px 0 10px 22px; border-bottom: 1px solid var(--mp-line); font-size: 15px; position: relative; }
.feat-copy li::before { content: ''; position: absolute; left: 2px; top: 17px; width: 8px; height: 8px; background: var(--mp-acc); clip-path: polygon(50% 0, 100% 50%, 50% 100%, 0 50%); }
/* Let the screenshot run off the page edge a little, so it reads as a window, not a thumbnail. */
.feat:not(.flip) .frame { margin-right: -64px; }
.feat.flip .frame { margin-left: -64px; }

/* ---------- gallery ---------- */
.tabs { display: flex; flex-wrap: wrap; gap: 0; border-bottom: 1px solid var(--mp-line); margin-bottom: 24px; }
.tabs button {
  padding: 10px 16px; font: 600 15px 'Oxanium', sans-serif; color: var(--mp-mute); background: none;
  border: 0; border-bottom: 2px solid transparent; margin-bottom: -1px; cursor: pointer;
}
.tabs button:hover { color: var(--mp-ink); }
.tabs button[aria-selected='true'] { color: var(--mp-ink); border-bottom-color: var(--mp-acc); }
.tabs button:focus-visible { outline: 2px solid var(--mp-hold); outline-offset: -2px; }
.g-cap { color: var(--vp-c-text-2); font-size: 15.5px; margin: 16px 0 0; }

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
  .feat { grid-template-columns: 1fr; gap: 28px; }
  .feat.flip .feat-copy { order: 0; }
  .feat:not(.flip) .frame, .feat.flip .frame { margin: 0; }
  .tour { gap: 88px; }
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
  .studio { margin: 0 -16px; }
  .studio figcaption { grid-template-columns: 1fr; gap: 12px; padding: 24px 16px 32px; }
  .studio img { height: 260px; }
  .tabs { flex-wrap: nowrap; overflow-x: auto; }
  .tabs button { white-space: nowrap; }
}
</style>
