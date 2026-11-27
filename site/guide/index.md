# What MultiParity is

MultiParity is version control for game and art teams, on your own server. It keeps every
version of your project's files, including the very big ones (scenes, textures, renders,
audio), and lets a whole team work on them without overwriting each other.

It is built to be a friendlier Perforce:

- **Claims instead of merge conflicts.** Binary files cannot be merged, so before you change one
  you claim it. Nobody else can publish it until you publish or release it. The desktop app and
  the Blender add-on claim a file for you the moment you start editing it.
- **Big files are cheap.** Files are cut into parts by their content. A small change to a 4 GB
  scene sends only the parts that changed, and a file nobody changed is never stored twice.
- **Apps artists can use.** A desktop app for Windows and macOS, a web app, and a Blender add-on.
  No commands to learn, no staging area, no detached heads.
- **Your server, your data.** Runs with Docker Compose on a machine in the studio or in the
  cloud, with file data on its disks, a NAS or any S3-compatible storage. Unlimited people.

## The words

MultiParity uses plain words. If you know Perforce, here is how they line up:

| In MultiParity | What it means | In Perforce |
|---|---|---|
| Claim / Release | Reserve a file so nobody else can publish it / give it back | Check out / unlock |
| Publish | Send your edits to the server as one numbered change | Submit |
| Get latest | Bring your folder up to date | Sync |
| Checkpoint | A private save point on your computer | (a Git commit) |
| Save draft | Unfinished work saved on the server, only for you | Shelve |
| Discard / Rewind | Throw away edits / go back to a checkpoint | Revert |
| Branch, Update from Main, Merge into Main | Parallel work, and bringing it back | Streams |

## What you get

| Piece | What it does |
|---|---|
| **Server** | Accounts, organizations, projects, branches, claims, history, tags, drafts, administration, nightly backups, email, webhooks. One Docker Compose file. |
| **Desktop app** | Every project from every account you sign in to. Choose which projects and folders live on this computer; it syncs, claims on first edit, keeps checkpoints, publishes, and tells you when others publish. Updates itself. |
| **Web app** | Browse files with thumbnails and previews, history, older versions, branches, people and administration from any browser. |
| `mp` | The command line, for scripts, build machines and people who like terminals. |
| **Blender add-on** | The open file's state, and claim, publish, checkpoint and get latest without leaving Blender. |
| **For AI assistants** | `mp mcp` lets Claude and other assistants look up projects, files, history and claims. |

Next: [run a server](./server), or if your studio already has one, [get the desktop app](https://github.com/vltmedia/MultiParity/releases/latest) and read [Use it day to day](./using).
