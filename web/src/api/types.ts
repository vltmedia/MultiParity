// Shapes from docs/API.md (v1). Keep these in step with the server.
// Untyped fields in API.md (ids, names, times, paths) are strings; sizes and numbers are numbers.

export type Id = string;
/** RFC 3339 */
export type Time = string;

export type ApiErrorBody = {
  error: { code: string; message: string; details?: unknown };
};

// ---------- Auth ----------

export type Role = 'owner' | 'admin' | 'member' | 'guest';

export type Org = {
  id: Id;
  slug: string;
  name: string;
  created_at: Time;
  member_count?: number;
  project_count?: number;
  /** The organization's look for light and dark mode (theme ids), when an admin set one. */
  theme_light?: string | null;
  theme_dark?: string | null;
};

export type Me = {
  id: Id;
  email: string;
  name: string;
  is_server_admin: boolean;
  status: 'active' | 'suspended';
  orgs: Array<{ org: Org; role: Role }>;
};

export type LoginResponse = { token: string; user: Me };

export type InviteInfo = { org: Org; role: Role; email?: string; expires_at?: Time; also?: Array<{ org: Org; role: Role }> };

// ---------- Organizations and projects ----------

export type UserRef = { id: Id; name: string; email: string };

export type ProjectSummary = {
  id: Id;
  slug: string;
  name: string;
  org: Org;
  archived: boolean;
  head_change: number;
  size: number;
  file_count: number;
  my_claims: number;
  claims: number;
  last_change?: { number: number; message: string; author: UserRef; created_at: Time };
  branches: number;
  /** what the project is set to */
  checkpoint_setting: CheckpointSetting;
  /** where checkpoints are kept right now; "auto" uses the server once the project is 20 GB or larger */
  checkpoint_storage: CheckpointStorage;
  /** take checkpoints automatically every few minutes while people work; off unless turned on */
  auto_checkpoints: boolean;
  /** The look for people who have not picked their own: the project's, else its organization's.
   * own_* are the project's own settings (null: follow the organization). */
  theme?: { light: string | null; dark: string | null; own_light: string | null; own_dark: string | null };
  /** name of the storage location that holds this project's file data; "main" is the default */
  storage: string;
  /** file data is being moved to another storage location right now */
  storage_moving: boolean;
  /** Where it is filed: Organization > Client > Production > Project. Both levels are optional. */
  client?: { id: Id; name: string } | null;
  production?: { id: Id; name: string; main_project_id: Id | null } | null;
  /** the main project of its production: listed first, and where opening the production lands */
  is_main?: boolean;
};

export type Client = { id: Id; name: string; created_at: Time; productions: number; projects: number };
export type Production = { id: Id; name: string; client_id: Id | null; main_project_id: Id | null; created_at: Time; projects: number };
export type OrgStructure = { clients: Client[]; productions: Production[] };

// ---------- Policies ----------

export type ScopeKind = 'org' | 'client' | 'production' | 'project';
export type RuleLevel = 'none' | 'read' | 'write';
export type PolicyRule = { scope_kind: ScopeKind; scope_id: Id; level: RuleLevel; scope_name?: string };
export type Policy = {
  id: Id;
  name: string;
  description: string;
  /** every member (not guest) has it without being given it */
  is_default: boolean;
  created_at: Time;
  rules: PolicyRule[];
  people: Array<UserRef & { role: Role | null }>;
  /** for the default policy: how many members have it */
  member_count: number | null;
};
export type WhoCanSee = { user: UserRef; role: Role; level: 'none' | 'read' | 'write' | 'admin'; because: string };

export type CheckpointSetting = 'auto' | 'server' | 'local';
export type CheckpointStorage = 'server' | 'local';

export type Project =ProjectSummary & { branches_list: Branch[]; my_role: Role };

// ---------- Branches ----------

export type Branch = {
  id: Id;
  name: string;
  parent_id: string | null;
  shares_claims: boolean;
  protected: boolean;
  archived: boolean;
  head_change: number;
  created_at: Time;
  created_by: UserRef;
  /** paths changed on this branch not yet in parent */
  ahead: number;
  /** paths changed on parent since last update */
  behind: number;
};

export type MergeReport = {
  up_to_date: boolean;
  only_here: number;
  only_parent: number;
  auto_merged: string[];
  conflicts: Array<{ path: string; kind: 'text' | 'binary' }>;
};

// ---------- Files, changes, history ----------

export type FileKind =
  | 'model'
  | 'texture'
  | 'image'
  | 'audio'
  | 'video'
  | 'level'
  | 'scene'
  | 'code'
  | 'text'
  | 'cache'
  | 'other';

export type TagRef = { id: Id; name: string; color: string };

export type FileEntry = {
  path: string;
  size: number;
  file_hash: string;
  change: number;
  /** derived from the extension, see FileKind */
  kind: string;
  claim?: { user: UserRef; since: Time; device?: string };
  tags: TagRef[];
};

export type Change = {
  number: number;
  branch_id: Id;
  message: string;
  author: UserRef;
  created_at: Time;
  kind: 'publish' | 'merge' | 'update';
  file_count: number;
  size: number;
  merged_from?: number[];
};

export type ChangeFile = { path: string; op: 'add' | 'edit' | 'delete'; size: number; file_hash?: string | null };

export type ChangeDetail = Change & { files: ChangeFile[] };

export type History = { changes: number[]; files: Array<{ path: string; changes: number[] }> };

export type Webhook = {
  id: string;
  url: string;
  events: string[];
  project_id: string | null;
  active: boolean;
  secret_hint: string;
  /** Only in the answer to adding one. */
  secret?: string;
  created_at: string;
  last_at: string | null;
  last_status: number | null;
  last_error: string | null;
  failures: number;
};

/** One published version of a file: the change that added, edited or deleted it. */
export type FileVersion = {
  change: number;
  op: 'add' | 'edit' | 'delete';
  size: number;
  file_hash: string | null;
  manifest_hash: string | null;
  message: string;
  created_at: Time;
  kind: string;
  branch_id: Id;
  branch_name: string;
  author: UserRef | null;
  /** The version the branch has now. */
  current: boolean;
};

// ---------- Claims ----------

export type Claim = {
  path: string;
  branch_id: Id;
  user: UserRef;
  device?: string;
  created_at: Time;
};

export type ClaimResponse = { claimed: string[]; held: Claim[] };

// ---------- Tags ----------

export type Tag = TagRef & { group?: string; org_id: Id; project_id?: string };

// ---------- Drafts ----------

export type Draft = {
  id: Id;
  title: string;
  branch_id: Id;
  created_at: Time;
  file_count: number;
  size: number;
};

export type DraftDetail = Draft & { entries: PublishEntry[] };

// ---------- Sync protocol ----------

export type Manifest = {
  hash: string;
  size: number;
  file_hash: string;
  chunks: Array<[hash: string, len: number]>;
};

export type PublishEntry = {
  path: string;
  op: 'put' | 'delete';
  prev_file_hash: string | null;
  manifest?: Manifest;
};

export type TreeDelta = {
  head: number;
  entries: Array<{
    path: string;
    file_hash: string | null;
    manifest_hash: string | null;
    size: number;
    change: number;
  }>;
};

export type PublishConflictDetails = { stale: string[]; claimed: Claim[] };

// ---------- Events ----------

export type EventKind =
  | 'change.published'
  | 'change.merged'
  | 'claim.added'
  | 'claim.released'
  | 'claim.nudged'
  | 'tag.applied'
  | 'tag.removed'
  | 'branch.created'
  | 'branch.archived'
  | 'draft.saved'
  | 'member.added'
  | 'member.suspended'
  | 'member.reactivated'
  | 'member.removed'
  | 'project.created'
  | 'project.archived'
  | 'project.restored'
  | `admin.${string}`;

export type MpEvent = {
  id: number;
  kind: EventKind | string;
  org_id?: Id;
  project_id?: Id;
  actor?: UserRef;
  data: Record<string, unknown>;
  created_at: Time;
};

// ---------- Admin ----------

export type AdminOverview = {
  users: number;
  orgs: number;
  projects: number;
  /** Whole-server figures: null when the overview is for some organizations only. */
  storage_bytes: number | null;
  chunks: number | null;
  active_claims: number;
  stale_claims: number;
  suspended: number;
};

export type AdminUser = {
  id: Id;
  email: string;
  name: string;
  status: 'active' | 'suspended';
  is_server_admin: boolean;
  created_at: Time;
  last_seen_at?: Time;
  suspended_reason?: string;
  orgs: Array<{ org: Org; role: Role }>;
  claims: number;
  sessions: number;
};

export type ClaimWithProject = Claim & { project: { id: Id; name: string } };

/**
 * API.md writes this as `AdminUser & {sessions: Session[], claims: Claim[]}`, but AdminUser already has
 * numeric `claims` and `sessions` counts, so the intersection would be impossible. On the detail
 * endpoint the arrays replace the counts.
 */
export type AdminUserDetail = Omit<AdminUser, 'claims' | 'sessions'> & { sessions: Session[]; claims: ClaimWithProject[] };

export type Member = {
  user: UserRef & { status: 'active' | 'suspended'; last_seen_at?: Time };
  role: Role;
  expires_at?: Time;
  created_at: Time;
};

export type Invite = {
  id: Id;
  email?: string;
  role: Role;
  expires_at?: Time;
  created_at: Time;
  created_by: UserRef;
  url: string;
  accepted_at?: Time;
  /** Other organizations the same link adds the person to. */
  also?: Array<{ org_id: Id; role: Role }>;
  /** Policies given on joining. */
  policies?: Id[];
  /** Just made, and sent by email (when the server sends email). */
  emailed?: boolean;
};

export type Session = {
  id: Id;
  kind: 'session' | 'token';
  label?: string;
  device?: string;
  created_at: Time;
  last_used_at?: Time;
};

export type StorageCheckReport = {
  checked: number;
  missing: Array<{ hash: string; changes: number[] }>;
  ok: boolean;
};

export type GcDryRun = {
  unreferenced_chunks: number;
  bytes: number;
  /** the same amounts, split by storage location */
  by_location?: Array<{ location: string; chunks: number; bytes: number }>;
};
export type GcResult = { deleted: number; bytes: number };

/** One person's checkpoints kept on the server for one project. Sizes only, never contents. */
export type CheckpointUsage = {
  user: UserRef;
  project: { id: Id; name: string };
  org: { id: Id; name: string };
  checkpoints: number;
  /** how many of `checkpoints` were taken automatically */
  automatic: number;
  /** bytes uploaded only for these checkpoints */
  stored: number;
  /** bytes of the files as they were */
  size: number;
  oldest: Time;
  newest: Time;
};

// ---------- Storage locations ----------

export type StorageKind = 'local' | 'mount' | 's3';

/** A named place where the server keeps project file data. "main" is the default. */
export type StorageLocation = {
  name: string;
  kind: StorageKind;
  main: boolean;
  /** path or bucket; only server admins get it, otherwise null */
  description: string | null;
  chunks: number;
  stored_bytes: number;
  projects: Array<{ id: Id; name: string; org: string }>;
};

export type StorageMoveStatus = 'running' | 'done' | 'failed' | 'interrupted';

export type StorageMove = {
  id: Id;
  project_id: Id;
  project_name: string;
  from: string;
  to: string;
  status: StorageMoveStatus;
  total_chunks: number;
  copied_chunks: number;
  copied_bytes: number;
  error: string | null;
  started_at: Time;
  finished_at: Time | null;
};

export type StorageOverview = {
  locations: StorageLocation[];
  /** projects whose location is no longer configured on the server */
  unavailable: Array<{ id: Id; name: string; storage: string }>;
  /** the 20 most recent moves, newest first */
  moves: StorageMove[];
};

export type BackupRun = {
  id: string; started_at: string; finished_at: string | null; status: 'running' | 'done' | 'failed'; error: string | null;
  snapshot: string | null; db_bytes: number | null; new_chunks: number | null; new_chunk_bytes: number | null; total_chunks: number | null;
};
export type BackupSnapshot = { id: string; created_at: string; server_version: string; tables: number; rows: number; chunks: number; db_bytes: number; new_chunks: number; new_chunk_bytes: number };
/** Backup status for Admin > Fixing. `configured: false` when the server has no backup target. */
export type BackupStatus =
  | { configured: false }
  | {
      configured: true; target: string; target_error: string | null; schedule: string; next_run: string;
      keep_daily: number; keep_weekly: number; running: boolean; runs: BackupRun[]; snapshots: BackupSnapshot[];
    };
export type ProjectStorage = { storage: string; previous: string | null; move: StorageMove | null };

export type ClearCheckpoints ={ user: Id; project: Id; only_automatic?: boolean; older_than_days?: number };

// ---------- Local daemon ----------

export type DaemonHealth = { version: string; workspaces: number };

export type Workspace = {
  id: Id;
  root: string;
  server: string;
  account_email: string;
  project_id: Id;
  project_name: string;
  org_name: string;
  branch_id: Id;
  branch_name: string;
  synced_change: number;
  include: string[];
  exclude: string[];
  local_bytes: number;
  total_bytes: number;
};

export type LocalFile = { path: string; size: number; claimed: boolean };

export type Checkpoint = {
  id: Id;
  note?: string;
  auto: boolean;
  created_at: Time;
  files: number;
  /** Kept on the server instead of this computer. */
  on_server?: boolean;
  /** Render output and cache files this checkpoint left out. */
  left_out?: number;
};

export type LocalStatus = {
  new: LocalFile[];
  edited: LocalFile[];
  deleted: LocalFile[];
  behind: number;
  ignored: number;
  checkpoints: Checkpoint[];
  /** Why the last automatic checkpoint was skipped, or what it left out. */
  checkpoint_notice?: string | null;
  /** Where this folder keeps checkpoints. */
  checkpoint_storage?: 'server' | 'local';
  /** Whether checkpoints are also taken automatically in this folder. */
  auto_checkpoints?: boolean;
};

export type FolderNode = {
  path: string;
  name: string;
  size: number;
  local_size: number;
  included: 'all' | 'some' | 'none';
  children: FolderNode[];
};

export type IgnoreChoice = {
  label: string;
  pattern: string;
  matches: number;
  bytes: number;
  tracked: number;
};

// ---------- Exports ----------

export type ExportTarget = { name: string; description: string; kind: 'local' | 'mount' | 's3'; downloadable: boolean };

export type ProjectExport = {
  id: Id;
  project_id: Id;
  branch_id: Id;
  branch_name?: string;
  change: number;
  target: string;
  format: 'zip' | 'files';
  name: string;
  archive_after: boolean;
  status: 'queued' | 'running' | 'done' | 'failed';
  files_total: number;
  files_done: number;
  bytes_total: number;
  bytes_done: number;
  error?: string | null;
  created_at: Time;
  finished_at?: Time | null;
  created_by?: UserRef | null;
  downloadable: boolean;
};
