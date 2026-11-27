# Plugins

A plugin adds to MultiParity on each person's computer:

- **hooks** that run around publishing and getting the latest, and can stop a publish (a naming
  rule, a missing texture, a file that is too big);
- **operations** on files (right-click a `.blend`: *Render preview*) or on a whole project (the
  project's **Plugins…** button);
- **views**: web pages shown as a tab beside *Your work* and *Files*, or as a page of their own
  in the app's sidebar.

A plugin is a GitHub repository (or a folder in one) with a `multiparity-plugin.json`. Its code is
any program: Python, Node, a compiled tool, a Blender script started with `blender -b -P`.
MultiParity starts it for each call and talks JSON over stdin and stdout.

`plugins/example` in the MultiParity repository is a complete example.

## Installing

In the desktop app: **Plugins**, type `owner/repo` and **Look it up**. You see what the plugin
does, the command it runs and what it says it needs, before anything is installed. From a
terminal:

    mp plugin install owner/repo               # latest release, else the default branch
    mp plugin install owner/repo@v1.2          # a tag, branch or commit
    mp plugin install owner/repo/tools/checks  # a plugin in a folder of a repository
    mp plugin list
    mp plugin update [id]                      # asks again if it wants new permissions
    mp plugin off <id> / mp plugin on <id>
    mp plugin remove <id>

Each plugin is pinned to the exact commit installed, in `<config>/plugins/<id>/<commit>/`.
Private repositories: set `GITHUB_TOKEN` (or `MP_GITHUB_TOKEN`) to a token that can read them.

A plugin runs as you, with your access to this computer. Install only plugins you trust, as you
would any program.

## Making one

    mp plugin link path/to/my-plugin   # use a folder as a plugin; edits apply straight away

### multiparity-plugin.json

```json
{
  "id": "studio-checks",
  "name": "Studio checks",
  "version": "1.0.0",
  "description": "Naming rules and previews for the art team.",
  "author": "Cinder Works",
  "run": { "command": ["python3", "main.py"], "windows": ["python", "main.py"], "timeout": 120 },
  "hooks": ["before_publish", "after_publish", "after_get_latest"],
  "operations": [
    { "id": "render", "title": "Render preview", "files": ["*.blend"], "confirm": "Render a preview? It takes a minute." },
    { "id": "report", "title": "Texture report" }
  ],
  "views": [
    { "id": "queue", "title": "Render queue", "page": "ui/index.html", "place": "project" },
    { "id": "home", "title": "Studio tools", "page": "ui/home.html", "place": "app" }
  ],
  "permissions": ["files:read", "programs"]
}
```

| Field | |
|---|---|
| `id` | Letters, numbers, `-`, `_`, `.`; unique among your plugins |
| `run.command` | The program and its arguments. `./tool` or `bin/tool` run from the plugin's folder; other names come from `PATH`. `windows`, `macos`, `linux` override it per system. `timeout` in seconds (default 600, hooks 120) |
| `hooks` | `before_publish` (can stop it), `after_publish`, `after_get_latest` |
| `operations` | `files`: glob patterns (`*.blend`, `Art/**/*.png`), matched against the path and the file name, ignoring case; none: a project operation. `confirm`: asked first |
| `views` | `page`: an HTML file in the plugin. `place`: `project` (a tab) or `app` (a sidebar page) |
| `permissions` | Shown before installing, and checked by the app for views: `files:read`, `files:write`, `network`, `programs`, `server` |

### The call

MultiParity writes one JSON line to the program's stdin and closes it:

```json
{ "protocol": 1, "kind": "hook", "hook": "before_publish",
  "workspace": { "root": "D:\\Work\\Emberfall", "project": { "id": "…", "name": "Emberfall" },
                 "organization": "Cinder Works", "branch": "Main", "server": "https://mp.example.com",
                 "workspace_id": "…" },
  "data": { "message": "Hero rig fixes",
            "files": [{ "path": "Art/hero.blend", "abs": "D:\\Work\\Emberfall\\Art\\hero.blend", "op": "edit", "size": 1234 }] } }
```

- `before_publish` data: `message`, `files` (`op` is `add`, `edit` or `delete`).
- `after_publish` data: `change`, `message`, `paths`.
- `after_get_latest` data: `change`, `downloaded`, `removed` (only when something changed).
- An operation: `"kind": "operation"`, `"operation": "<id>"`, and `"files": [{ "path", "abs" }]`.

The program writes JSON lines to stdout, then exits:

```json
{"message": "Rendering 3 of 10"}
{"result": {"ok": true, "message": "Rendered 10 previews.", "open": "D:\\Work\\Emberfall\\previews"}}
```

- `{"message": …}` lines show as progress in the app (and the terminal).
- The last `{"result": …}` is the answer. `message` is shown to the person; `open` (a path) is
  opened in the desktop app. For `before_` hooks, `{"result": {"ok": false, "message": "why"}}`
  stops the publish with that message.
- `{"error": "…"}` instead of a result reports a failure. Exiting without a result counts as
  success when the exit code is 0, and as a failure (with stderr) otherwise.
- A `before_` hook that fails or times out also stops the publish, saying so; the person can turn
  the plugin off under **Plugins** to go ahead without it. `after_` hooks only report.

The environment has `MP_PLUGIN_DIR` and `MP_PLUGIN_ID`; the working directory is the plugin's
folder.

### Views

A view is an ordinary web page from the plugin's folder, shown in a sandboxed frame: it cannot
reach the app's local API or storage directly. It talks to the app through `window.mp`:

```html
<script src="/local/plugin-sdk.js"></script>
<script>
  const ctx = await mp.context();        // { plugin, view, workspace, workspaces, selection, theme }
  const st = await mp.local("status");   // the folder's new, edited and deleted files (files:read)
  const r = await mp.run("report");      // run one of this plugin's operations; its result
  const p = await mp.server("claims");   // GET from the server for this project, as you (server)
  mp.toast("Done");                      // a message in the app
  mp.open("Art/hero.blend");             // open (or mp.reveal) a file in this project's folder
</script>
```

`mp.local` reads `status`, `tree`, `checkpoints` and `versions` of the view's folder;
`mp.server` paths are relative to the project on the server (`claims`, `changes?limit=10`,
`branches/<id>/files`). Project views get the folder they are shown for; app views get
`workspaces`, and pass `{ workspace }` to operations they run.

The page may load scripts, styles and images from its own folder and from `https:` addresses, and
fetch from `https:` addresses.

## Coming next

- Organization admins requiring, allowing or blocking plugins for everyone in an organization.
- Plugins on the server: rules on every publish for everyone, and views in the web app.
