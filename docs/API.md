# MultiParity HTTP API (v1)

All endpoints are JSON under `/api`. Authentication is a bearer token:
`Authorization: Bearer <token>`. The web UI stores the token in `localStorage` under `mp.token`.
For scripts and AI assistants, make a personal access token (below, or on your account page in
the web app): it acts as you, with exactly your access, until you revoke it.

Every endpoint applies the caller's access (see [Access and policies](#access-and-policies)):
a project someone cannot see answers `404`, one they can only look at answers `403` to changes.

Errors: non-2xx with `{"error": {"code": "snake_case", "message": "Human readable."}}`.
Common codes: `unauthorized` (401), `forbidden` (403), `not_found` (404), `conflict` (409),
`invalid` (422), `suspended` (403).

Ids are UUID strings. Times are RFC 3339 strings. Sizes are bytes (numbers).

## Auth

| Method | Path | Body | Returns |
|---|---|---|---|
| POST | `/api/auth/login` | `{email, password, device?}` | `{token, user: Me}` |
| POST | `/api/auth/logout` | | `{}` |
| GET | `/api/me` | | `Me` |
| POST | `/api/invites/{token}/accept` | `{name, password}` (new account) or `{}` (signed in) | `{token, user: Me}` |
| GET | `/api/invites/{token}` | | `{org: Org, role, email?, expires_at?}` (no auth) |
| GET | `/api/me/tokens` | | `Session[]`: your sign-ins and access tokens |
| POST | `/api/me/tokens` | `{label}` | `{token, label}`: a personal access token, shown once |
| DELETE | `/api/me/tokens/{id}` | | `{}`: revoke a token or sign-in |

```ts
type Me = { id, email, name, is_server_admin: boolean, status: 'active'|'suspended',
            orgs: Array<{ org: Org, role: Role }> }
type Role = 'owner' | 'admin' | 'member' | 'guest'
type Org = { id, slug, name, created_at, member_count?: number, project_count?: number }
```

## Organizations and projects

| Method | Path | Body | Returns |
|---|---|---|---|
| GET | `/api/orgs` | | `Org[]` (ones I belong to; all for server admins) |
| POST | `/api/orgs` | `{name, slug?}` | `Org` (server admin) |
| GET | `/api/projects` | | `ProjectSummary[]` (all I can see, every org) |
| POST | `/api/orgs/{org}/projects` | `{name, slug?, storage?, client?, production?}` | `Project` (org admin); `production` decides the client |
| GET | `/api/projects/{project}` | | `Project` |

```ts
type ProjectSummary = { id, slug, name, org: Org, archived: boolean, head_change: number,
  size: number, file_count: number, my_claims: number, claims: number,
  last_change?: { number, message, author: UserRef, created_at }, branches: number,
  // where it is filed: Organization > Client > Production > Project (both optional)
  client: { id, name } | null,
  production: { id, name, main_project_id: string | null } | null,
  is_main: boolean }  // the main project of its production: listed first
type Project = ProjectSummary & { branches_list: Branch[], my_role: Role,
  can_write: boolean, can_admin: boolean }
type UserRef = { id, name, email }
```

`{project}` accepts the id. Lists are ordered by organization, client, production (main project
first) and name.

## Clients and productions

An organization can file its projects under clients (who the work is for) and productions (a
show, game or campaign); both are optional. Each project keeps its own history, branches, claims
and access. Removing a client or production never removes projects: what was in it moves up a
level. Org admins only.

| Method | Path | Body | Returns |
|---|---|---|---|
| GET | `/api/admin/orgs/{org}/structure` | | `{clients: Client[], productions: Production[]}` |
| POST | `/api/admin/orgs/{org}/clients` | `{name}` | `{id, name}` |
| PATCH | `/api/admin/clients/{id}` | `{name}` | `{id, name}` |
| DELETE | `/api/admin/clients/{id}` | | `{ok}` |
| POST | `/api/admin/orgs/{org}/productions` | `{name, client?}` | `{id, name, client_id}` |
| PATCH | `/api/admin/productions/{id}` | `{name?, client?: id \| null, main_project?: id \| null}` | `{ok}`; its projects follow a new client |
| DELETE | `/api/admin/productions/{id}` | | `{ok}` |
| PATCH | `/api/admin/projects/{p}` | `{client?: id \| null, production?: id \| null}` | `Project`: files it (a production decides the client) |

```ts
type Client = { id, name, created_at, productions: number, projects: number }
type Production = { id, name, client_id: string | null, main_project_id: string | null,
                    created_at, projects: number }
```

## Access and policies

Owners and admins of an organization can do everything in it. Everyone else gets what their
**policies** give: named sets of rules, each giving `none`, `read` or `write` on the whole
organization, a client, a production or one project. Members (not guests) also have the
organization's default policy, *All members* (`write` on everything until an admin changes it).
For each project the most specific rules win (project, then production, then client, then
organization), and among those the highest level. Access given on a project itself
(`/api/admin/projects/{p}/access`) counts as a project rule.

| Method | Path | Body | Returns |
|---|---|---|---|
| GET | `/api/admin/orgs/{org}/policies` | | `Policy[]` (the default first) |
| POST | `/api/admin/orgs/{org}/policies` | `{name, description?, rules: Rule[]}` | `{id, name}` |
| PATCH | `/api/admin/policies/{id}` | `{name?, description?, rules?: Rule[]}` (rules replace all) | `{ok}` |
| DELETE | `/api/admin/policies/{id}` | | `{ok}` (not the default policy) |
| PUT | `/api/admin/policies/{id}/people` | `{users: id[]}` (exactly who has it; people in the org) | `{ok}` |
| GET | `/api/admin/projects/{p}/who` | | `WhoCanSee[]`: everyone in the org, their access and why |

```ts
type Rule = { scope_kind: 'org'|'client'|'production'|'project', scope_id, level: 'none'|'read'|'write' }
type Policy = { id, name, description, is_default: boolean, created_at,
  rules: Array<Rule & { scope_name }>, people: Array<UserRef & { role }>,
  member_count: number | null }  // for the default policy
type WhoCanSee = { user: UserRef, role: Role, level: 'none'|'read'|'write'|'admin', because: string }
```

Changes apply straight away. Invites can give policies on joining (`policies` below).

## Exports

Write one branch, as it is now, as plain files or one `.zip`: to the server's exports folder
(the `.zip` can then be downloaded) or to a place set up in `MP_EXPORT_TARGETS` (a folder, NAS
or S3 bucket). People can keep working; later changes are not included. Org admins.

| Method | Path | Body | Returns |
|---|---|---|---|
| GET | `/api/admin/export-targets` | | `Array<{name, kind: 'local'\|'mount'\|'s3', description}>` (`server` is always there) |
| GET | `/api/admin/projects/{p}/exports` | | `Export[]`, newest first |
| POST | `/api/admin/projects/{p}/exports` | `{target, format: 'zip'\|'files', branch?, archive_after?}` | `Export` (runs in the background; `409` while one is running) |
| POST | `/api/admin/exports/{id}/link` | | `{url}`: downloads a finished `.zip` on the server once, within ten minutes |
| GET | `/api/exports/{id}/zip?token=` | | the `.zip` (the link above; no other auth) |

```ts
type Export = { id, project_id, branch_id, branch_name, change, target, format, name,
  archive_after: boolean, status: 'queued'|'running'|'done'|'failed',
  files_total, files_done, bytes_total, bytes_done, error?, created_at, finished_at?,
  created_by?: UserRef, downloadable: boolean }
```

## Branches

| Method | Path | Body | Returns |
|---|---|---|---|
| GET | `/api/projects/{p}/branches` | | `Branch[]` |
| POST | `/api/projects/{p}/branches` | `{name, from: branchId, shares_claims?: true}` | `Branch` |
| POST | `/api/projects/{p}/branches/{b}/update` | `{resolutions?: {[path]: 'mine'|'parent'}}` | `MergeReport & {change: Change \| null}` (nothing is changed while `conflicts` is not empty) |
| GET | `/api/projects/{p}/branches/{b}/merge-preview` | | `MergeReport` |
| POST | `/api/projects/{p}/branches/{b}/merge` | `{message}` | `{change: Change}` |
| POST | `/api/projects/{p}/branches/{b}/archive` | | `Branch` |

```ts
type Branch = { id, name, parent_id: string|null, shares_claims: boolean, protected: boolean,
  archived: boolean, head_change: number, created_at, created_by: UserRef,
  ahead: number,      // paths changed on this branch not yet in parent
  behind: number }    // paths changed on parent since last update
type MergeReport = { up_to_date: boolean, only_here: number, only_parent: number,
  auto_merged: string[], conflicts: Array<{ path, kind: 'text'|'binary' }> }
```

## Files, changes, history

| Method | Path | Query | Returns |
|---|---|---|---|
| GET | `/api/projects/{p}/branches/{b}/files` | `prefix?`, `tag?`, `q?` | `FileEntry[]` |
| GET | `/api/projects/{p}/branches/{b}/tree` | `since?` (change number) | `TreeDelta` (client sync) |
| GET | `/api/projects/{p}/changes` | `branch?`, `before?`, `limit?` | `Change[]` newest first |
| GET | `/api/projects/{p}/changes/{n}` | | `Change & {files: ChangeFile[]}` |
| GET | `/api/projects/{p}/history` | `branch`, `prefix?`, `limit?` (changes) | `History` |
| GET | `/api/projects/{p}/download` | `path`, `branch`, `change?` (an older version) | file bytes |
| POST | `/api/auth/forgot` | body `{email}` | `{sent: true}` whether or not the address has an account; `503 email_off` without email |
| POST | `/api/auth/reset` | body `{token, password}` | `{email}`; signs out other sessions. `422` for an expired or used link |
| GET | `/api/server` | | `{email: boolean}` (public) |
| GET | `/api/admin/email` | | `{configured, kind?, via?, from?}` (server admins) |
| POST | `/api/admin/email/test` | | `{sent_to}`; `502 email_failed` with the provider's error |
| GET | `/api/projects/{p}/thumbs/{file_hash}` | `size?` (`large` for 1280 px; 256 px otherwise) | JPEG, cached for good (`X-Image-Size: WxH` is the original's size); `202 {status: 'pending'}` while it is drawn; `404 no_thumbnail` when there is nothing to show |
| GET | `/api/projects/{p}/versions` | `path`, `branch` | `{path, versions: FileVersion[]}` newest first, all branches |
| POST | `/api/projects/{p}/branches/{b}/restore-version` | body `{path, change, message?}` | `{change: Change}`: the old version made the latest as a new change |

```ts
type FileEntry = { path, size, file_hash, change: number, kind: string,
  claim?: { user: UserRef, since, device? }, tags: TagRef[] }
type Change = { number, branch_id, message, author: UserRef, created_at,
  kind: 'publish'|'merge'|'update', file_count: number, size: number, merged_from?: number[] }
type ChangeFile = { path, op: 'add'|'edit'|'delete', size }
type History = { changes: number[], files: Array<{ path, changes: number[] }> }
type TagRef = { id, name, color }
type FileVersion = { change: number, op: 'add'|'edit'|'delete', size, file_hash?, manifest_hash?,
  message, created_at, kind, branch_id, branch_name, author?: UserRef, current: boolean }
```

`restore-version` needs write access (admin on a protected branch), refuses a file someone else
has claimed (409 with `details.claimed`), a version that deleted the file, and the version the
branch already has. The daemon offers the folder side at
`GET|POST /local/workspaces/{id}/versions` (`{path, change, to: 'folder'|'copy'|'latest'}`).

`kind` is derived from the extension: `model`, `texture`, `image`, `audio`, `video`, `level`,
`scene`, `code`, `text`, `cache`, `other`.

## Claims

| Method | Path | Body | Returns |
|---|---|---|---|
| GET | `/api/projects/{p}/claims` | | `Claim[]` |
| POST | `/api/projects/{p}/claims` | `{branch, paths: string[], device?}` | `{claimed: string[], held: Claim[]}` |
| POST | `/api/projects/{p}/claims/release` | `{branch, paths: string[]}` | `{released: string[]}` |
| POST | `/api/projects/{p}/claims/nudge` | `{branch, path}` | `{}` (notifies holder) |

```ts
type Claim = { path, branch_id, user: UserRef, device?: string, created_at }
```

## Tags

| Method | Path | Body | Returns |
|---|---|---|---|
| GET | `/api/projects/{p}/tags` | | `Tag[]` (org + project) |
| POST | `/api/projects/{p}/tags` | `{name, color, group?, project_only?}` | `Tag` |
| POST | `/api/projects/{p}/tags/apply` | `{tag, paths: string[]}` | `{}` |
| POST | `/api/projects/{p}/tags/remove` | `{tag, paths: string[]}` | `{}` |

```ts
type Tag = TagRef & { group?: string, org_id, project_id?: string }
```

## Drafts

| Method | Path | Body | Returns |
|---|---|---|---|
| GET | `/api/projects/{p}/drafts` | | `Draft[]` (mine) |
| POST | `/api/projects/{p}/drafts` | `{branch, title, entries: PublishEntry[]}` | `Draft` |
| GET | `/api/projects/{p}/drafts/{id}` | | `Draft & {entries}` |
| DELETE | `/api/projects/{p}/drafts/{id}` | | `{}` |

```ts
type Draft = { id, title, branch_id, created_at, file_count, size }
```

## Checkpoints kept on the server

Private to their author: nobody else can list or fetch them, admins included (admins see sizes
only, under Admin). The file data is uploaded first, as for publishing. The server thins each
person's automatic checkpoints per project and computer to the newest 50 and 30 days, and undo
checkpoints to 30 days; ones people made themselves stay until cleared.

| Method | Path | Body | Returns |
|---|---|---|---|
| GET | `/api/projects/{p}/checkpoints` | | `ServerCheckpoint[]` (mine, every computer) |
| POST | `/api/projects/{p}/checkpoints` | `{id, branch, device?, note?, auto?, system?, stored?, entries: PublishEntry[]}` | `ServerCheckpoint` |
| GET | `/api/projects/{p}/checkpoints/{id}` | | `ServerCheckpoint & {entries}` |
| DELETE | `/api/projects/{p}/checkpoints/{id}` | | `{}` |

```ts
// id is the one the person's computer gave it; stored is the data uploaded only for it
type ServerCheckpoint = { id, server_id, branch_id, device?, note?, auto, system,
                          file_count, size, stored, created_at }
```

A project says where its checkpoints go in `checkpoint_setting` (`'auto'|'server'|'local'`) and
`checkpoint_storage` (`'server'|'local'`, what is in effect). `auto_checkpoints` (boolean, off
unless an admin turns it on) says whether people's computers take checkpoints by themselves. On `auto` the server is used once
the project is `MP_SERVER_CHECKPOINTS_FROM_GB` (default 20) or larger.

## Sync protocol (clients)

| Method | Path | Body | Returns |
|---|---|---|---|
| POST | `/api/projects/{p}/chunks/missing` | `{hashes: string[]}` | `{missing: string[]}` |
| PUT | `/api/projects/{p}/chunks/{hash}` | zstd-compressed chunk bytes | `{}` |
| GET | `/api/projects/{p}/chunks/{hash}` | | zstd-compressed chunk bytes |
| GET | `/api/projects/{p}/manifests/{hash}` | | `Manifest` |
| POST | `/api/projects/{p}/branches/{b}/publish` | `{message, entries: PublishEntry[]}` | `{change: Change}` |

```ts
type Manifest = { hash, size, file_hash, chunks: Array<[hash: string, len: number]> }
type PublishEntry = { path, op: 'put'|'delete', prev_file_hash: string|null,
                      manifest?: Manifest }
type TreeDelta = { head: number, entries: Array<{ path, file_hash: string|null,
                   manifest_hash: string|null, size, change: number }> }
```

Publish is rejected with `409 conflict` and `details: {stale: string[], claimed: Claim[]}` if a
path changed on the server since `prev_file_hash`, or is claimed by someone else.

## Events

`GET /api/events?project={p}&token={token}` is a Server-Sent Events stream. Each event:
`event: <kind>` with `data: Event`.

```ts
type Event = { id: number, kind: string, org_id?, project_id?, actor?: UserRef,
               data: object, created_at }
```

Each person only receives events of projects they can see (their policies as they are when they
connect).

Kinds: `change.published`, `change.merged`, `claim.added`, `claim.released`, `claim.nudged`,
`tag.applied`, `tag.removed`, `branch.created`, `branch.archived`, `draft.saved`,
`member.added`, `member.suspended`, `member.reactivated`, `member.removed`, `project.created`,
`project.archived`, `project.restored`, `project.exported`, `admin.*` (including
`admin.client_*`, `admin.production_*` and `admin.policy_*`).

## Admin

Server admins can do everything. Org owners and admins can act on their own org (`org` query or
body field required for them).

| Method | Path | Body / query | Returns |
|---|---|---|---|
| GET | `/api/admin/overview` | `org?` | `{users, orgs, projects, storage_bytes, chunks, active_claims, stale_claims, suspended}` (`storage_bytes` and `chunks` are `null` unless a server admin looks at the whole server) |
| GET | `/api/admin/users` | `q?`, `org?`, `status?` | `AdminUser[]` |
| GET | `/api/admin/users/{id}` | | `AdminUser` with `claims: Claim[]` (with project) in place of the count, plus `claim_count` and `sessions: Session[]` |
| POST | `/api/admin/users/{id}/suspend` | `{reason?}` | `AdminUser` |
| POST | `/api/admin/users/{id}/reactivate` | | `AdminUser` |
| POST | `/api/admin/users/{id}/sign-out` | | `{revoked: number}` |
| DELETE | `/api/admin/users/{id}` | `{confirm: name, transfer_to?: userId}` | `{}` (server admin) |
| POST | `/api/admin/users/{id}/password` | `{password}` | `{}` |
| GET | `/api/admin/orgs/{org}/members` | | `Member[]` |
| PATCH | `/api/admin/orgs/{org}` | `{name?, theme_light?, theme_dark?}` (theme ids; `""` back to the default) | `Org` |
| PATCH | `/api/admin/orgs/{org}/members/{user}` | `{role?, expires_at?}` | `Member` |
| DELETE | `/api/admin/orgs/{org}/members/{user}` | | `{}` (revoke from org) |
| GET | `/api/admin/orgs/{org}/invites` | | `Invite[]` |
| POST | `/api/admin/orgs/{org}/invites` | `{emails?: string[], role, expires_in_days?, also?: Array<{org, role}>, policies?: id[]}` | `Invite[]` (with `url`); `also` joins more organizations with the same link, `policies` are given on joining |
| DELETE | `/api/admin/invites/{id}` | | `{}` |
| GET | `/api/admin/projects` | `org?` | `ProjectSummary[]` incl. archived |
| PATCH | `/api/admin/projects/{p}` | `{name?, org?, client?, production?, checkpoints?: 'auto'\|'server'\|'local', auto_checkpoints?: boolean, theme_light?, theme_dark?}` (theme ids; `""` follows the organization) | `Project`, whose `theme` is `{light, dark, own_light, own_dark}` |
| POST | `/api/admin/projects/{p}/archive` / `restore` | | `Project` |
| GET | `/api/admin/claims` | `older_than_hours?`, `org?` | `Array<Claim & {project: {id,name}}>` |
| POST | `/api/admin/claims/release` | `{claims: Array<{project, branch, path}>}` | `{released}` |
| GET | `/api/admin/checkpoints` | `org?` | `Array<{user: UserRef, project: {id,name}, org: {id,name}, checkpoints, automatic, stored, size, oldest, newest}>`, biggest first |
| POST | `/api/admin/checkpoints/clear` | `{user, project, only_automatic?, older_than_days?}` | `{removed}` (space returns at the next gc) |
| POST | `/api/admin/storage/check` | `{project?}` | `{checked, missing: Array<{hash, changes}>, ok}` |
| GET | `/api/admin/storage/gc` | | `{unreferenced_chunks, bytes}` (dry run) |
| POST | `/api/admin/storage/gc` | | `{deleted, bytes}` (server admin) |
| GET | `/api/admin/audit` | `org?`, `project?`, `actor?`, `kind?`, `before?`, `limit?` | `Event[]` |

```ts
type AdminUser = { id, email, name, status: 'active'|'suspended', is_server_admin,
  created_at, last_seen_at?, suspended_reason?, orgs: Array<{org: Org, role: Role}>,
  claims: number, sessions: number }
type Member = { user: UserRef & {status, last_seen_at?}, role: Role, expires_at?, created_at }
type Invite = { id, email?, role, expires_at?, created_at, created_by: UserRef, url, accepted_at?,
  also: Array<{org_id, role}>, policies: string[] }
type Session = { id, kind: 'session'|'token', label?, device?, created_at, last_used_at? }
```

## AI assistants (MCP)

`POST /api/mcp` serves the Model Context Protocol (Streamable HTTP, protocol 2025-06-18 and
earlier) with a personal access token in `Authorization: Bearer <token>`. Each POST carries one
JSON-RPC message or a batch and is answered with JSON (`202` for notifications); there is no
server-sent stream and no session, so `GET` and `DELETE` answer `405`. Methods: `initialize`,
`ping`, `tools/list`, `tools/call`. The tools call the endpoints above as the token's person, so
they get the same answers and the same `403`/`404`s; they are listed in docs/AUTOMATION.md.

## Local daemon API

The daemon (`mp daemon`) listens on `http://127.0.0.1:18519` and allows CORS from the server's
web UI origin and from the desktop app. No auth: it is bound to loopback only.

| Method | Path | Body | Returns |
|---|---|---|---|
| GET | `/local/health` | | `{version, workspaces: number}` |
| GET | `/local/workspaces` | | `Workspace[]` |
| GET | `/local/workspaces/{id}/status` | | `LocalStatus` |
| GET | `/local/workspaces/{id}/folders` | | `FolderNode` (tree with sizes, included flag) |
| PUT | `/local/workspaces/{id}/sync` | `{include: string[], exclude: string[]}` | `Workspace` |
| POST | `/local/workspaces/{id}/get-latest` | | `{downloaded, bytes}` |
| POST | `/local/workspaces/{id}/publish` | `{message, paths?: string[], keep_mine?: string[]}` | `{change}`; `409` with `details: {stale, claimed}` |
| POST | `/local/workspaces/{id}/checkpoint` | `{note?, all?}` | `Checkpoint & {left_out, left_out_bytes}`; `409` when it would not fit or only render output changed |
| POST | `/local/workspaces/{id}/no-checkpoint` | `{pattern, personal?}` | `{file}` |
| GET | `/local/workspaces/{id}/space` | | `CacheReport` (trims, then reports checkpoint space, limit, free disk) |
| GET | `/local/workspaces/{id}/ignore/suggest` | `path` | `IgnoreChoice[]` |
| POST | `/local/workspaces/{id}/ignore` | `{pattern, personal?}` | `{file}` |
| GET | `/local/workspaces/{id}/checkpoints` | | `Checkpoint[]` |
| GET | `/local/workspaces/{id}/checkpoints/{cp}` | | `Checkpoint & {files: Array<{path, size, deleted}>}` (also ones kept on the server) |
| POST | `/local/workspaces/{id}/rewind` | `{checkpoint, paths?: string[]}` | `{files, undo}`: `undo` is the checkpoint taken first |
| POST | `/local/workspaces/{id}/discard` | `{paths: string[]}` | `{files, undo}` |
| POST | `/local/workspaces/{id}/draft` | `{title}` | `Draft` |
| POST | `/local/workspaces/{id}/drafts/{draft}/restore` | | `{files}`: the draft's files written into the folder (current work checkpointed first) |
| POST | `/local/workspaces/{id}/branch` | `{branch_id}` | `{branch, downloaded, removed, workspace}`; `409` while there is unpublished work |
| GET, POST | `/local/workspaces/{id}/versions` | `path` / `{path, change, to: 'folder'\|'copy'\|'latest'}` | versions, or what was done |
| GET | `/local/file` | `path` (full path on this computer) | `{synced, workspace_id?, project?, branch?, branch_id?, path?, state?: 'new'\|'edited'\|'unchanged'\|'deleted'\|'ignored', claim?: {user, mine, since}, change?, newer_on_server?, server_change?}`, for plugins |
| GET | `/local/notifications` | `after?` | `{notifications: Notice[], last}`: newer than `after` (none without it; start from `last`). `Notice = {id, kind: 'newer'\|'nudge', title, body, workspace_id?, project_id?}`. The daemon follows each account's `/api/events` and keeps others' publishes and merges to a branch one of your folders is on, and requests to release your claimed files. |
| GET, PUT | `/local/notifications/settings` | `{newer, nudges}` | which kinds to keep (saved in the config folder) |
| GET | `/local/behind` | | `{workspace_id: files behind}` for folders whose branch has newer files |
| any | `/local/workspaces/{id}/server/{rest}` | passed through | the server's `/api/projects/{project}/{rest}` with the folder's account. Only from the app itself (or that server's web UI); server errors come back unchanged. The desktop app's Files, History, Branches and Drafts tabs use it. `MP_DAEMON_DEV_ORIGIN` allows one more origin for working on those pages in a browser. |
| POST | `/local/workspaces/{id}/disconnect` | | `{root}` (files stay) |
| GET | `/local/accounts` | | `Array<{id, server, email, name, label?}>` |
| POST | `/local/accounts/login` | `{server, email, password, label?}` | account |
| DELETE | `/local/accounts/{id}` | | `{}` |
| GET | `/local/accounts/{id}/session` | | `{server, token}` (desktop app only; refused for web origins) |
| GET | `/local/projects` | | `{accounts: Array<{account, projects: Array<{project, workspaces: Workspace[]}>, error?}>}` |
| POST | `/local/connect` | `{account_id, project_id, branch?, folder, include?, exclude?, dry_run?, download?}` | `{report: ConnectReport, workspace?}` |

Requests carrying an `Origin` header are accepted only from a signed-in server's web UI, the
desktop app and localhost.

```ts
type ConnectReport = { matched: number, different: string[], local_only: number, ignored: number,
  to_download: number, download_bytes: number, downloaded: number }
```

```ts
type Workspace = { id, root, server, account_email, project_id, project_name, org_name,
  branch_id, branch_name, synced_change, include: string[], exclude: string[],
  local_bytes, total_bytes }
type LocalStatus = { new: LocalFile[], edited: LocalFile[], deleted: LocalFile[],
  behind: number, ignored: number, checkpoints: Checkpoint[] }
type LocalFile = { path, size, claimed: boolean }
type FolderNode = { path, name, size, local_size, included: 'all'|'some'|'none', children: FolderNode[] }
type IgnoreChoice = { label, pattern, matches: number, bytes: number, tracked: number }
type Checkpoint = { id, note?, auto: boolean, created_at, files: number }
```
