# Automation: webhooks, event scripts, AI assistants, plugins

## Webhooks

Admin > Fixing > Webhooks (pick the organization at the top). Give an address and which
events; the server posts each one there as it happens:

    POST https://hooks.example.com/multiparity
    Content-Type: application/json
    X-MultiParity-Event: change.published
    X-MultiParity-Delivery: 4812
    X-MultiParity-Signature: sha256=5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843

    {"id": 4812, "kind": "change.published", "org_id": "...", "project_id": "...",
     "actor": {"id": "...", "name": "Sana", "email": "sana@studio.example"},
     "data": {"number": 312, "branch_id": "...", "message": "Hero retopo", "files": 3},
     "created_at": "2026-11-26T18:02:11Z", "server": "https://mp.studio.example"}

The secret is shown once, when the webhook is added. Check the signature before trusting a
delivery (Python):

    import hmac, hashlib
    want = "sha256=" + hmac.new(SECRET.encode(), request_body, hashlib.sha256).hexdigest()
    ok = hmac.compare_digest(want, request.headers["X-MultiParity-Signature"])

A delivery that fails is tried again after 10 s and 60 s. After 20 failures in a row the webhook
is paused; Resume turns it back on. **Send a test** posts a `webhook.test` event straight away.

Events: `change.published`, `change.merged`, `claim.added`, `claim.released`, `claim.nudged`,
`draft.saved`, `branch.created`, `branch.archived`, `tag.applied`, `project.created`,
`member.added`, `member.removed`, `member.suspended`, and `admin.*` for administration. Patterns
like `change.*` pick a group; no pattern means everything.

Over the API: `GET/POST /api/admin/orgs/{org}/webhooks` (`{url, events?, project_id?}`),
`PATCH/DELETE /api/admin/webhooks/{id}` (`{url?, events?, active?}`),
`POST /api/admin/webhooks/{id}/test`.

## Event scripts: `mp watch`

Run your own command for each event, on any computer signed in with `mp login` (a build
machine, a render manager):

    mp watch --project Emberfall --event change.published --exec ./on-publish.sh

The command runs through the shell, one event at a time, with the event as JSON on its stdin and
in `MP_EVENT_JSON`, plus `MP_EVENT_KIND`, `MP_EVENT_ID`, `MP_EVENT_PROJECT_ID`, `MP_EVENT_ORG_ID`,
`MP_EVENT_ACTOR` and `MP_EVENT_ACTOR_EMAIL`. Without `--exec`, each event is printed as one JSON
line, ready for `jq`. It reconnects by itself if the network drops.

Example: get the latest and start a build whenever someone publishes.

    mp watch --project Emberfall --event change.published \
      --exec 'cd ~/build/Emberfall && mp get-latest && ./build.sh'

## AI assistants (MCP)

MultiParity speaks the Model Context Protocol, so an assistant such as Claude can look things up,
claim files and, for admins, check access and start exports, as you and with exactly your access
(your policies included). There are two ways to connect it.

**From the server (nothing to install).** In the web app, open your account menu > **Access
tokens**, make a token, and copy the setup it shows. For Claude Code:

    claude mcp add --transport http multiparity https://mp.example.com/api/mcp \
      --header "Authorization: Bearer <token>"

Assistants that take an MCP server address (Claude Desktop, Cursor, VS Code…):

    { "mcpServers": { "multiparity": { "type": "http", "url": "https://mp.example.com/api/mcp",
        "headers": { "Authorization": "Bearer <token>" } } } }

The token works until you revoke it on the same page. Treat it like a password.

**From your computer: `mp mcp`.** Sign in with `mp login`, then add it as a command. It also
sees the folders synced on that computer (`local_folders`). For Claude Code:

    claude mcp add multiparity -- mp mcp

For Claude Desktop, in `claude_desktop_config.json`:

    { "mcpServers": { "multiparity": { "command": "mp", "args": ["mcp"] } } }

With several accounts, add `--account <name or email>`.

Tools (both ways, except where noted):

| Tool | Does |
|---|---|
| `list_projects` | Projects this account can open, with client, production and main project (filter by client or production) |
| `list_files` | Files on a branch, with who has each claimed (filter by folder or name) |
| `recent_changes`, `change_details` | The history, and what a change touched |
| `file_history` | Every version of one file |
| `claims` | Who has what claimed |
| `claim_files`, `release_files`, `ask_to_release` | Claims, as this account |
| `organization_structure` | Admins: clients and productions, and each production's main project |
| `who_can_see` | Admins: who can change, look at or not see a project, and which role or policy decides it |
| `list_policies` | Admins: access policies, their rules and who has each |
| `list_exports`, `start_export` | Admins: a project's exports, and start one (to the server or a place set up for exports; never archives) |
| `local_folders` | `mp mcp` only: folders synced on this computer and their unpublished files |

It does not publish, discard, delete or change access: those stay with people.

## Plugins: Blender

`plugins/blender/multiparity` is a Blender add-on (Blender 2.93 to 5.x). In the 3D view's sidebar
(N), tab **MultiParity**, it shows the open file's state (up to date, edited, claimed by whom,
newer on the server) and has **Claim / Release / Ask to release**, **Publish** (saves first),
**Save checkpoint**, **Get latest** (reopens the file when a newer version came in) and
**Older versions** with **Put back**.

It talks to the MultiParity app on the same computer (the desktop app or `mp daemon`), so it
uses the account and folders already set up there.

Install: zip the folder, then in Blender, Edit > Preferences > Add-ons > Install from Disk:

    cd plugins/blender && python -m zipfile -c multiparity.zip multiparity

Other tools can do the same through the app's local API: `GET /local/file?path=<full path>`
says which synced folder a file is in and its state; the rest is in docs/API.md.
