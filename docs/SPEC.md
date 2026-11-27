# MultiParity: standalone base spec

Status: draft 1, 2026-11-26. Covers the base that works with no Kitlig involvement.
Kitlig is a separate commercial product and stays closed source; its integration is a later,
optional client of the same local daemon and is out of scope here.

## What it is

A self-hosted version control system for game and art teams. One server per studio, deployed with
Docker Compose, unlimited users, open source. Artists use a desktop app and their file manager;
developers use the same app or a CLI.

## Who it's for

- Artists and other non-technical people who need to share large files without overwriting each
  other, and who never want to read the word "changelist".
- Developers who need atomic numbered changes, a scriptable CLI and CI hooks.
- The one person who runs the server and does not want it to be a job.

## Vocabulary

One set of words everywhere. The CLI accepts the Perforce words as aliases.

| Word | Meaning | Perforce |
|---|---|---|
| Project | A versioned tree on the server | Depot |
| Change | One atomic, numbered set of file updates | Changelist |
| Claim | Reserve a file so nobody else can publish it | Check out / lock |
| Release | Give a claim back without publishing | Unlock |
| Checkpoint | A private save point of unpublished work, kept on this computer | (none; Git commit) |
| Publish | Send your edits to the server as a change | Submit |
| Get latest | Bring your folder up to date | Sync |
| Save draft | Save unfinished work on the server without publishing; it becomes a Draft | Shelve |
| Discard | Throw away local edits to a file | Revert |
| Rewind | Bring a file or folder back to an earlier change | Roll back |
| Branch | A named, shared copy of a project that can be merged back. Every project has Main | Stream / branch |
| Update from Main | Bring the parent branch's new changes into this branch | Merge down |
| Merge into Main | Deliver this branch's work to its parent | Copy up |

## Principles

1. Prevent conflicts, do not resolve them. Binary files are claimed; text files merge.
2. No "check out before edit". The daemon watches the folder; the first save of a lockable file
   claims it automatically, or tells the artist at once that someone else holds it.
3. No clientspecs, no typemaps. People tick the folders they want. File kind and lockability are
   detected, with a per-project rules file to override.
4. Nothing is lost. Every destructive action has a way back for at least 30 days.
5. Everything the apps can do, the HTTP API can do.

## New files

A file that exists in a synced folder but not on the server has the status **New**.

- The daemon notices new files by itself. There is no "add" step. Dragging files onto the app or
  into the folder in the file manager is the same thing.
- New files are ticked into the next change by default. The artist can untick one (**Leave out**,
  for this change only) or choose **Ignore** for a file, a folder or a kind of file.
- Ignore rules live in a versioned `.mpignore` at the project root, so the whole team gets
  them. Projects start with rules for OS junk, autosaves and backups, and engine caches
  (Unreal `Saved/`, `Intermediate/`, `DerivedDataCache/`; Unity `Library/`, `Temp/`).
- The app always shows how many files the rules are hiding, with a way to see them, so a file is
  never silently missing from a change.
- When the daemon sees a new lockable file it reserves the path on the server, so two people
  cannot both create `hero_body_normal.png` and find out at publish time.
- A new folder is just its files; empty folders are not tracked.
- New files larger than a per-project limit (default 10 GB) ask before being included.
- CLI: `mp status` lists new files; `mp publish` includes them; `mp add` and `mp ignore` exist for
  scripts and for people who expect them.

## Ignore list

Every project has an ignore list: patterns for files that stay on people's computers and are
never published, never show as New and never count as edited.

**Adding to it by right-click.** Right-clicking any file or folder, in the app or in the file
manager, offers **Ignore**. The choices are worked out from that file's name and place, widest
last, each with a live count of what it would match:

| Choice | Example for `Characters/Hero/hero_body_autosave_0142.blend1` |
|---|---|
| Only this file | `Characters/Hero/hero_body_autosave_0142.blend1` |
| Files named like this (numbers and dates become wildcards) | `hero_body_autosave_*.blend1` |
| Anything with a junk word in the name (autosave, backup, tmp, copy, old) | `*autosave*` |
| This kind of file, in this folder | `Characters/Hero/*.blend1` |
| This kind of file, everywhere | `*.blend1` |
| Everything in this folder | `Characters/Hero/` |

- Several files can be selected at once; the choices are then the patterns they have in common.
- Each rule is either **for everyone on the project** or **only for me**. Adding a rule for
  everyone needs the permission to edit the list; anyone can add personal rules.
- If a choice would match files that are already published, the app says how many and which. By
  default they stay tracked and only future files are ignored; "stop tracking these too" is a
  separate, explicit tick that removes them from the project in the next change (history kept).

**The list itself.** A screen per project shows every rule with how many files it matches on
this computer, who added it and when, and whether it is for everyone or personal. Rules can be
switched off, edited or removed. A "check a name" field says whether a given file name would be
ignored, and by which rule. The files currently ignored can always be listed.

- Shared rules are stored in `.mpignore` at the project root, which is itself versioned, so
  changes to the list are published, reviewable and can be rewound like any file. Folders can
  carry their own `.mpignore`. Personal rules are stored by the daemon and never published.
- Pattern syntax is the same as `.gitignore`, including `!` to make an exception, so existing
  ignore files can be dropped in and developers already know it.
- Organizations can keep **starter lists** (Unreal, Unity, Godot, Blender, Maya, Houdini,
  Photoshop, OS junk) that new projects pick from.
- An ignored file can be un-ignored from the same right-click menu, which shows the rule that
  catches it.
- CLI: `mp ignore <pattern>`, `mp ignore --mine <pattern>`, `mp ignore --check <path>`,
  `mp ignored`.

## Looks

47 themes ship with the web and desktop apps. Which one shows:

1. The person's own pick, kept in that browser or desktop app (Look, in the account menu or the
   sidebar).
2. Otherwise (Automatic, the starting point) the open project's look, then its organization's,
   each a theme for light mode and one for dark mode, following the computer's setting.
3. Otherwise Ice Lab in light mode and Orbital HUD in dark mode.

Organization admins set an organization's look on its admin page, and a project's with Look…
in Admin > Projects.

## Thumbnails and previews

The server draws a picture of every published image (PNG, JPEG, GIF, WebP, BMP, TGA, TIFF, and
EXR and HDR, tone mapped) and Blender file (the preview Blender saves inside the `.blend`, also
when the file is compressed). A background worker does it after each publish and for files
published before, keeping a 256 px thumbnail for lists and a 1280 px preview, by content, so a
file is drawn once however often it is published. Thumbnails are not backed up; they are drawn
again after a restore.

On the web, file lists show them, and clicking one (or Preview) opens the big picture, or the
text of text files. Other kinds keep a coloured placeholder for now (models, video, audio, PSD).

## Older versions of a file

Every published version of a file stays on the server. Right-click a file (desktop) or select it
and choose Older versions (web) to see them, newest first, and:

- **Download** one (web), or **Save a copy** beside the file as `name (change 12).ext` (desktop,
  `mp versions <file> --copy 12`). The file itself is left alone.
- **Put back** one in your folder in place of the file (desktop, `--get 12`). It shows as edited;
  publish it to make it the latest for everyone. Your unpublished edits go into a checkpoint first.
- **Make latest**: make it the latest on the server straight away, as a new change, without a
  folder (web, desktop, `--make-latest 12`). Refused while someone else has the file claimed.

Going back never loses anything: the newer versions stay in the history.

## Checkpoints

The step between editing and publishing. Three levels, each one further from your computer:

| Step | Where it lives | Who sees it | Git equivalent |
|---|---|---|---|
| Checkpoint | This computer | Only you | Commit |
| Save draft | The server | Only you, or people you share it with | Push to a private branch |
| Publish | The server | Everyone, as a numbered change | Push or merge to main |

- **Automatic checkpoints are off by default.** Most art tools already autosave, and copying
  big files every few minutes costs disk, battery and upload. A project admin can turn them on
  for a project (Admin > Projects), and a person can turn them on or off for one folder
  (`mp space --auto-checkpoints on|off|project`). When on, the daemon takes one when files have
  changed, at most every five minutes.
- **Named checkpoints.** "Save checkpoint" with a note ("before retopo") covers every edited and
  new file at that moment. Named ones are kept until the work is published or discarded.
- **Going back.** Any file, or the whole set of unpublished work, can be rewound to a checkpoint.
  Rewinding makes a checkpoint of the current state first, so it can be undone.
- **Cheap.** A checkpoint keeps only the parts of a file that exist nowhere else. Parts the server
  already has (the unchanged 99% of a big file after a small edit, or anything since published)
  are never written to `.mp/cache`; they are fetched again if someone rewinds. A file that has
  not changed since the last checkpoint is not read again.
- **Renders and caches are left out.** Render output and simulation caches can be made again
  and are far too big to copy every few minutes, so checkpoints skip them: `*.exr`, `*.dpx`,
  `*.cin`, `*.vdb`, `*.abc`, `*.bgeo*`, `*.sim`, and the folders `renders/`, `render/`,
  `comp/out/`, `cache/`, `caches/`, `playblasts/`, `flipbooks/` (any letter case). Projects add
  their own rules in `.mpnocheckpoint` (shared, published like `.mpignore`, same syntax; `!rule`
  makes an exception such as `!Textures/**/*.exr`), people add personal ones, and right-clicking
  a file in Your work offers "leave out of checkpoints" with the same pattern choices as the
  ignore list. `mp checkpoint --all` includes everything once. Publishing is not affected.
  When discard, rewind or picking up a draft would replace a changed file that checkpoints
  leave out, the file is moved to `.mp/safety/` (a rename, no copy) instead of being lost.
- **A limit that fits the disk.** Automatic and undo checkpoints may use a tenth of the free
  disk in each folder unless a number is set (`mp space --limit <MB>|auto`). A checkpoint that
  would not fit is not written at all, and the person is told why (in Your work, and in
  `mp space`). Over the limit, the oldest automatic checkpoints go first (also after 30 days or
  beyond 50), then undo checkpoints over a week old. Ones people made themselves are never
  removed, are not held to the limit, and are refused only if they would leave under 1 GB free.
  `.mp/safety/` is emptied of anything older than 30 days.
- **On the server for big projects.** A project keeps checkpoints on the server when its admin
  says so, or automatically from 20 GB (`MP_SERVER_CHECKPOINTS_FROM_GB`); a folder can override
  with `mp space --keep server|local|project`. The checkpoint's parts are uploaded (only what
  the server lacks) and nothing is added to the laptop's disk. Server checkpoints are private
  to their author, survive a dead disk (`mp checkpoints --server`, `mp rewind <id>` from any
  computer), and fall back to the local kind when the server cannot be reached. Admins see how
  much space each person's checkpoints take per project and can clear them (Admin > Fixing),
  and choose where each project keeps them (Admin > Projects); they never see the contents.
- **No second copy of the project.** Downloaded and published files are never copied into
  `.mp/`: publishing reads parts straight from the files. A synced folder takes the space of
  its files, plus only unpublished checkpoint data. `mp space` shows it and frees what is not
  needed; the daemon trims each folder hourly and after automatic checkpoints.
- **Publishing collapses them.** A publish sends the final state as one change. The named
  checkpoints behind it stay attached to that change as steps that can be opened, so the project
  history stays clean without losing how the work got there. Per project this can be turned off.
- **Save draft sends checkpoints to the server**, which is also the backup: local checkpoints die
  with the disk. A per-project setting can do this automatically for named checkpoints.
- Claims are not affected: a claim is held from the first edit until publish, release or discard,
  however many checkpoints happen in between.
- CLI: `mp checkpoint -m "before retopo" [--all]`, `mp checkpoints [--server]`,
  `mp rewind <checkpoint>`, `mp no-checkpoint <pattern> [--personal] | --check <file>`,
  `mp space [--limit <MB>|auto] [--keep server|local|project]`.

## Branches and merging

A branch is a named, shared copy of a project that changes on its own until it is merged back.
Every project starts with one branch, **Main**.

The rules that keep it simple:

1. **Making a branch is instant and free.** It points at the same stored chunks; nothing is
   copied, whatever the project size.
2. **A branch merges only with the branch it came from.** Branches form a tree. There is no
   cherry-picking, rebasing or merging between unrelated branches in the base.
3. **Two directions, two buttons.** "Update from Main" brings the parent's new changes into the
   branch. "Merge into Main" delivers the branch's work to the parent.
4. **Update before you merge.** A branch must be up to date with its parent before it can merge
   into it. Clashes are therefore always sorted out on the branch, never on Main.
5. **Claims are shared with the parent by default.** A claimed binary file is claimed on the
   branch and its parent at once, so two people cannot edit the same model on two branches and
   find out at merge time. With this on, binary clashes cannot happen.
6. **A merge is one change.** It gets the next change number on the target and lists the changes
   it came from.

When files changed on both sides:

| Kind of file | What happens |
|---|---|
| Changed on one side only | Goes in as it is. This is nearly every file. |
| Text, changed on both | Merged automatically. Overlapping edits open a side-by-side resolver. |
| Binary, changed on both | Only possible when claims are not shared. Choose "Keep this branch's" or "Keep Main's" per file, with both previews side by side. The one not kept stays in history. |

Other details:

- **Independent branches.** A branch can be created with its own claims, for a release or a demo
  that must keep moving separately. Merging those back uses the per-file choice above.
- **Switching branches** on a computer downloads only the files that differ. Unpublished work and
  its checkpoints stay with the branch they were made on and come back when you switch back.
- **Protected branches.** A branch can require a group, or an approved review, to merge into it.
  Main can be protected so that all work arrives by merge.
- **Finishing.** A merged branch is archived, not deleted: hidden from lists, still in history.
- Change numbers stay one sequence per project across all branches, so "4813" is never ambiguous.
- Drafts are still the answer for one person's unfinished work. A branch is for work that several
  people share, or that must stay out of Main for a while.
- CLI: `mp branch new|switch|list|archive`, `mp update` (from parent), `mp merge` (into parent).

## Accounts and organizations

- A **server** holds **organizations**. An organization owns projects, members, groups, storage
  points, tags and ignore defaults. A small studio has one; a publisher or a freelancer
  collective can have several on one server.
- An **account** is a person on one server. An account can belong to several organizations with a
  different role in each: Owner, Admin, Member or Guest. Guests see only the projects they are
  invited to and can have an expiry date.
- The desktop app, CLI and daemon can be **signed in to several accounts at once**, on the same
  server or on different ones (a work account and a freelance account, for example).
- **Each project remembers which account it uses** on this computer. The choice is made when the
  project is connected and can be changed later without re-downloading anything. Publishing,
  claims and presence for that project always use that account.
- Per-project user settings sit on top of the account: display name and avatar for that project,
  notification level, which folders are on this computer, and whether new files are ticked by
  default.
- The app always shows which account and organization the current project uses, so nobody
  publishes client work under the wrong name.
- Tokens are stored in the OS keychain, one per account. Signing out of one account leaves the
  others and their local files alone.
- CLI: `mp account list|add|remove`, `mp project use-account <account>`.

## Projects page and selective sync

The Projects page is the first screen of the app and the place where people decide what is on
their computer.

- One list, grouped by account and then by organization. Each project row shows: name, a cover
  thumbnail, how much of it is on this computer ("12 of 40 GB"), its state (Up to date, 3 newer,
  2 unpublished), and a sync switch.
- Filters across the top: All, On this computer, Not on this computer, Has my work, Starred.
  A search field matches project, organization and folder names.
- A project that is not on this computer costs nothing. Turning it on asks where it should live
  (adopting an existing folder as described under Connecting folders) and which folders to bring.
- **Selective sync is per folder, per project, per computer.** Opening a project's sync settings
  shows its folder tree with a tick box and the size of each folder. Ticking a parent ticks its
  children; a partly ticked parent shows a dash. Sizes update as you tick.
- Selective sync works the same whichever organization or account a project belongs to, and the
  totals at the top of the page add up across all of them, with free space on each disk.
- Unticking a folder follows the same rule as everywhere else: only files identical to the server
  are removed; edited and new files stay and the app says so.
- Rules can follow file kinds as well as folders ("everything except video") for people on small
  disks. Stored per computer by the daemon, never shared.
- Pinning a project or folder keeps it at the top and keeps it downloaded.

## Administration

An Admin area in the web UI, and the same actions in the API and CLI (`mp admin ...`). What an
admin can see is scoped: server admins see everything; organization owners and admins see their
organization.

**People**
- List of members with search and filters (role, status, organization, last seen, has claims).
- Invite by email or by link; the link can carry the organization, role, groups and an expiry.
  Bulk invite from a pasted list or CSV.
- Change role, groups and per-project access from the person's page.
- **Suspend**: the account cannot sign in or sync, its tokens stop working at once, its claims
  stay so nobody's work is overwritten, and everything is reversible.
- **Revoke from an organization**: removes membership and project access; their published work
  stays attributed to them.
- **Delete account** (server admins only): requires typing the name, offers to hand their claims
  and drafts to someone else first.
- Per person: active sessions and devices with "sign out everywhere", API tokens with revoke,
  current claims with "release" (the holder is notified), last activity.

**Projects**
- Create, rename, archive, restore. Move a project between organizations.
- Storage point per project, retention rules, size and growth.
- Access by group or person, per folder.
- Branch settings: protected branches, who can merge.

**Fixing things**
- Stuck claims: list claims older than a threshold or held by suspended people; release in bulk.
- Interrupted uploads: list and clean up partial uploads.
- Storage check: verify that every revision's chunks exist and match their hash, per project or
  for the whole server, with a report of anything missing and which changes it affects.
- Garbage collection: show how much space unreferenced chunks use and reclaim it.
- Rebuild previews and thumbnails for a project.
- Audit log: who did what and when, filterable, exportable.

**Server**
- Organizations: create, rename, transfer ownership, delete when empty.
- Storage points: add, test, mark read-only, show free space.
- Sign-in methods (local passwords, OIDC providers), webhooks, backups, version and health.

## Tags

- Tags are labels on files or folders, for example "Approved", "Needs review", "Hero asset".
- An organization defines a shared set (name, colour, optional description). A project can add
  its own. People with the right permission can create tags on the fly.
- A tag sits on the path, not on one revision, and applies at once without a publish. Adding or
  removing a tag is recorded in the event log with who and when, and fires an event that scripts
  and webhooks can react to.
- A tag on a folder is inherited by what is inside it, shown as inherited, and can be removed per
  file.
- A tag can be marked **exclusive within a group** to work as a status: one of "In progress",
  "Needs review", "Approved". A tag can also be marked as **protecting**: files carrying it need a
  given group to publish, which is how "Approved" becomes a light gate.
- Everything that lists files can filter and search by tag: the app, the web UI, the CLI
  (`mp files --tag approved`) and the API.
- Personal tags (visible only to you) are allowed and never count toward permissions.

## Connecting folders

Choosing where a project lives on a computer, or changing which of its folders are on this
computer, never deletes or overwrites a file without an explicit yes.

When a project is connected to a folder that already has files in it, the daemon adopts them in
place:

| What it finds | What happens |
|---|---|
| Same content as the server | Marked up to date. Nothing is downloaded. |
| Different from the server | Kept. Shown as Edited, to keep, publish or replace per file. |
| Only on this computer | Kept. Shown as New (ignore rules still apply). |
| Only on the server | Listed with the total size; downloading is a tick box. |

- The app shows this summary before anything is written, with counts and sizes.
- Matching is by content hash, so a folder copied from a colleague or restored from a backup
  connects in minutes instead of re-downloading everything.
- If someone later chooses the server's version of a file, their copy first goes to **Safety
  copies** on that computer for 30 days.
- Turning a folder off for this computer removes only files that are identical to the server.
  Edited and new files stay where they are, and the app says so.
- Moving a project to another disk is a move, not a re-download.
- Two projects cannot be connected to the same folder or to nested folders.

## Architecture

```text
 Desktop app (Tauri) ─┐
 CLI (mp) ────────────┼─► Local daemon (mpd) ──HTTPS──► Server (mp-server) ─► Postgres
 File manager badges ─┘     watches folders,             API, auth, locks,    (metadata)
 Engine / tool plugins      chunks, syncs                changes, events   ─► Blob store
                                                                              (local, mount, S3)
 Web UI (browser) ─────────────────────────────HTTPS──►
```

- **mp-core** (Rust library): chunking, hashing, manifests, the change model, storage traits.
- **mp-server** (Rust): HTTP API, auth, locks, changes, event stream, serves the web UI.
- **mpd** (Rust): per-machine daemon. Owns synced folders, watches for edits, uploads and
  downloads chunks, holds the local cache. Exposes a local API on a socket.
- **mp** (Rust): CLI, a client of mpd. Every command has `--json`.
- **Desktop app** (Tauri + web UI code): a client of mpd.
- **Web UI**: browse, history, previews, review, admin. Served by mp-server.

Every client goes through mpd, so third-party tools integrate with one small local API.

## Storage

- Files are split with content-defined chunking (FastCDC), hashed with BLAKE3, compressed with
  zstd. Identical chunks are stored once across all projects.
- Blob store is a trait with three implementations in the base: local directory, mounted path
  (same code, different health checks), S3-compatible (AWS, MinIO, R2, Backblaze).
- Each project picks a storage point. A project can have a second, colder point for old revisions.
- Retention rules per path, for example "keep the last 10 revisions of `*.abc`".
- Uploads are resumable at chunk granularity. Garbage collection is mark and sweep, run on demand
  or on a schedule, never during a publish.

## Data model (Postgres)

- `organization`, `membership` (account, organization, role, expiry)
- `project` (belongs to an organization), `line`, `change` (per-project increasing number, author,
  message, time)
- `tag` (organization or project, name, colour, group, flags) and `path_tag` (project, path, tag)
- `file_revision` (path, change, manifest hash, size, kind) and `manifest` (ordered chunk hashes)
- `branch` (project, name, parent, created at change, shares claims, protected, archived)
- `claim` (path, claim scope, holder, since, machine) with a unique index so two holders cannot
  exist. Branches that share claims share one scope.
- `checkpoint` lives only in the daemon's local store until a draft uploads it
- `draft_change`, `user`, `group`, `permission` (path prefix, group, read/write/admin)
- `event` (append-only, feeds webhooks, the live stream and the audit log)

## Base feature set

In the base:

- Projects, changes, history, Rewind
- Checkpoints
- Branches: create, switch, update from parent, merge into parent, shared claims
- Claim, Release, Publish, Get latest, Save draft, Discard
- Auto-claim on first save, live presence ("Mara is editing this"), nudge to release, stale claim
  expiry, admin override
- Pick-folders partial sync
- Three storage backends, dedupe, compression, resumable transfer
- Organizations, roles and guests; local accounts plus OIDC sign-in, groups, per-path
  permissions, API tokens
- Several accounts signed in at once, with the account chosen per project
- Tags on files and folders, with filtering everywhere
- Connecting to a folder that already has files, without deleting or re-downloading
- Desktop app, CLI, web UI
- Thumbnails and previews for images, audio, video and glTF; text diff; image before/after
- Webhooks and a live event stream
- One-command backup and restore, storage usage report, audit log
- Invite links that configure the desktop app in one click
- Projects page with selective sync per folder, across organizations and accounts
- Admin area: people (invite, suspend, revoke, delete), projects, fixing tools, audit log

Later, in rough order:

- File manager badges and right-click menus (Explorer, Finder, Nautilus)
- Event scripts and plugins (sandboxed WASM), MCP server on mp-server and mpd
- Unreal, Unity, Godot, Blender plugins
- Perforce and Git LFS importers
- Review with annotations and approval gates
- Virtual files (download on open), office edge cache
- Git bridge for code paths
- Kitlig client

## Docker Compose

```text
services:
  server    mp-server, one port, serves API and web UI
  db        postgres
  worker    thumbnails and previews (ffmpeg, image and glTF tooling)
  minio     optional, only for the S3 profile
```

One `.env` file: admin account, storage point, public URL. `docker compose up` gives a working
server on a local directory with no other setup.

## Milestones

1. **Engine and CLI.** mp-core chunk store on a local directory, change model, `mp` commands
   against an in-process server. Done when two folders on one machine can publish and get latest
   with a 5 GB file and an interrupted upload resumes.
2. **Server.** HTTP API, Postgres, auth, claims, events, Compose file, S3 and mounted backends.
   Done when two machines share a project through a container.
3. **Daemon.** File watching, auto-claim, presence, partial sync, local cache.
4. **Desktop app and web UI.** The agreed UI concept, built on the daemon and server APIs.
5. **Previews and admin.** Worker, thumbnails, diffs, usage report, backup and restore.
6. **First release.** Installers for Windows, macOS and Linux, docs, invite links.

## Licensing

- mp-server, mpd, mp, desktop app, web UI: AGPL-3.0.
- The daemon client SDK and API definitions: Apache-2.0, so closed-source tools can integrate.
- Outside contributions under a contributor agreement.

## UI direction

Fixed by the brief: not Material or Bootstrap, square buttons, no Inter, premium and Mac-like in
behaviour (native menus and shortcuts, drag and drop, spacebar preview, restrained motion).
Colour and type are being chosen through concept rounds in `design/concepts/`.

## Open questions

- UI direction (concept round 1 in progress).
- SQLite single-binary mode for very small teams: worth the second database path or not.
- Whether the side-by-side text resolver ships in the base, or the base only merges text that
  does not overlap and asks for a per-file choice otherwise.
