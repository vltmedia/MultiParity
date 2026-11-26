# MultiParity HTTP API (v1)

All endpoints are JSON under `/api`. Authentication is a bearer token:
`Authorization: Bearer <token>`. The web UI stores the token in `localStorage` under `mp.token`.

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
| POST | `/api/orgs/{org}/projects` | `{name, slug?}` | `Project` (org admin) |
| GET | `/api/projects/{project}` | | `Project` |

```ts
type ProjectSummary = { id, slug, name, org: Org, archived: boolean, head_change: number,
  size: number, file_count: number, my_claims: number, claims: number,
  last_change?: { number, message, author: UserRef, created_at }, branches: number }
type Project = ProjectSummary & { branches_list: Branch[], my_role: Role }
type UserRef = { id, name, email }
```

`{project}` accepts the id.

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

Kinds: `change.published`, `change.merged`, `claim.added`, `claim.released`, `claim.nudged`,
`tag.applied`, `tag.removed`, `branch.created`, `branch.archived`, `draft.saved`,
`member.added`, `member.suspended`, `member.reactivated`, `member.removed`, `project.created`,
`project.archived`, `project.restored`, `admin.*`.

## Admin

Server admins can do everything. Org owners and admins can act on their own org (`org` query or
body field required for them).

| Method | Path | Body / query | Returns |
|---|---|---|---|
| GET | `/api/admin/overview` | | `{users, orgs, projects, storage_bytes, chunks, active_claims, stale_claims, suspended}` |
| GET | `/api/admin/users` | `q?`, `org?`, `status?` | `AdminUser[]` |
| GET | `/api/admin/users/{id}` | | `AdminUser` with `claims: Claim[]` (with project) in place of the count, plus `claim_count` and `sessions: Session[]` |
| POST | `/api/admin/users/{id}/suspend` | `{reason?}` | `AdminUser` |
| POST | `/api/admin/users/{id}/reactivate` | | `AdminUser` |
| POST | `/api/admin/users/{id}/sign-out` | | `{revoked: number}` |
| DELETE | `/api/admin/users/{id}` | `{confirm: name, transfer_to?: userId}` | `{}` (server admin) |
| POST | `/api/admin/users/{id}/password` | `{password}` | `{}` |
| GET | `/api/admin/orgs/{org}/members` | | `Member[]` |
| PATCH | `/api/admin/orgs/{org}/members/{user}` | `{role?, expires_at?}` | `Member` |
| DELETE | `/api/admin/orgs/{org}/members/{user}` | | `{}` (revoke from org) |
| GET | `/api/admin/orgs/{org}/invites` | | `Invite[]` |
| POST | `/api/admin/orgs/{org}/invites` | `{emails?: string[], role, expires_in_days?}` | `Invite[]` (with `url`) |
| DELETE | `/api/admin/invites/{id}` | | `{}` |
| GET | `/api/admin/projects` | `org?` | `ProjectSummary[]` incl. archived |
| PATCH | `/api/admin/projects/{p}` | `{name?, org?, checkpoints?: 'auto'\|'server'\|'local', auto_checkpoints?: boolean}` | `Project` |
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
type Invite = { id, email?, role, expires_at?, created_at, created_by: UserRef, url, accepted_at? }
type Session = { id, kind: 'session'|'token', label?, device?, created_at, last_used_at? }
```

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
