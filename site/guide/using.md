# Use it day to day

## Get the desktop app

Download it from the [latest release](https://github.com/vltmedia/MultiParity/releases/latest),
or from **Get the desktop app** in the web app's account menu:

- **Windows**: `MultiParity_…_x64-setup.exe`. Until the installer is code signed, Windows may say
  "Windows protected your PC": choose **More info > Run anyway**.
- **macOS** (Apple silicon and Intel): `MultiParity_…_universal.dmg`, then drag MultiParity to
  Applications. Signed and notarized.

The app updates itself. It lives in the menu bar or system tray, so closing the window keeps
your projects in sync.

## Set it up

1. **Accounts**: sign in with the server address, email and password your studio gave you. Add
   as many servers and accounts as you need.
2. **Projects**: every project you can open, from every account. Turn on the ones you want on
   this computer, choose where they live, and tick the folders to bring (you can leave out the
   huge ones you do not work on). A folder that already has the project's files is matched, not
   downloaded again.

## A working day

1. **Get latest** when the app says others published (it sends a notification).
2. **Edit**. The first time you change a file that cannot be merged, like a `.blend`, `.psd` or
   `.fbx`, it is claimed for you, so nobody else can publish it meanwhile. If someone else has it,
   you see who, and can ask them to release it.
3. **Save checkpoints** before trying something risky. They are private save points; rewind to
   one any time.
4. **Publish**: tick the files, say what you changed, and they go to everyone as one change. Your
   claims are released.

Not finished but need to stop, or switch to another branch? **Save a draft**: it goes to the
server where only you can see it, and you can pick it up on any computer.

## Going back

- **Discard** throws away your edits to a file (they are kept in a checkpoint first).
- **Rewind** puts files back as they were at a checkpoint.
- **Older versions** (right-click a file) lists every published version. Put one back in your
  folder, save a copy beside the file, or make it the latest for everyone. Nothing is lost:
  newer versions stay in the history.

## The web app

Open your server's address in a browser for the same projects with thumbnails and previews of
images and Blender files, the history, branches, people, and, for admins, everything in
**Admin**. **Look** in the account menu changes how it looks; **Automatic** follows your
computer's light or dark mode.

## The command line

```bash
mp login https://mp.yourstudio.com --label work
mp connect Emberfall ~/Work/Emberfall --only Art --only Audio
mp status
mp publish -m "Hero retopo"
```

`mp --help` lists everything: `get-latest`, `claim`, `release`, `checkpoint`, `rewind`,
`discard`, `versions`, `draft`, `branch`, `update`, `merge`, `ignore`, `folders`, `tag`, `admin`,
`watch` and `mcp`. Perforce words work too (`submit`, `sync`, `edit`). Add `--json` to any
command for scripts.

## Blender

The add-on in [`plugins/blender`](https://github.com/vltmedia/MultiParity/tree/main/plugins/blender)
puts a **MultiParity** tab in the 3D view's sidebar: the open file's state, and claim, publish,
checkpoint, get latest and older versions. See [Automation](/docs/AUTOMATION#plugins-blender) to
install it.
