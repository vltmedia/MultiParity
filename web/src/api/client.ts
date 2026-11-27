import { MOCK, loadMock } from './mode';
import type {
  AdminOverview,
  AdminUser,
  AdminUserDetail,
  Branch,
  Change,
  ChangeDetail,
  CheckpointSetting,
  CheckpointUsage,
  Claim,
  ClearCheckpoints,
  ClaimResponse,
  ClaimWithProject,
  Draft,
  DraftDetail,
  FileEntry,
  GcDryRun,
  GcResult,
  History,
  FileVersion,
  Invite,
  InviteInfo,
  LoginResponse,
  Me,
  Member,
  MergeReport,
  MpEvent,
  Org,
  Project,
  ProjectStorage,
  ProjectSummary,
  Role,
  StorageCheckReport,
  StorageMove,
  StorageOverview,
  BackupStatus,
  Tag,
} from './types';

const TOKEN_KEY = 'mp.token';

export function getToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

export function setToken(token: string | null) {
  try {
    if (token) localStorage.setItem(TOKEN_KEY, token);
    else localStorage.removeItem(TOKEN_KEY);
  } catch {
    /* storage blocked: the session lasts until the tab closes */
  }
}

/** An error from the server or the network, worded for people. */
export class ApiError extends Error {
  code: string;
  status: number;
  details?: unknown;
  constructor(status: number, code: string, message: string, details?: unknown) {
    super(message);
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

/** What went wrong and how to fix it, in the interface's voice. */
export function explain(err: unknown, doing?: string): string {
  const lead = doing ? `Could not ${doing}. ` : '';
  if (err instanceof ApiError) {
    const msg = err.message && !/^[a-z_]+$/.test(err.message) ? ensureStop(err.message) + ' ' : '';
    switch (err.code) {
      case 'network':
        return `${lead}The server did not answer. Check your connection, or ask whoever runs the server whether it is up, then try again.`;
      case 'unauthorized':
        return `${lead}You are signed out. Sign in again to carry on.`;
      case 'suspended':
        return `${lead}${msg || 'This account is suspended. '}Ask an organization admin to reactivate it.`;
      case 'forbidden':
        return `${lead}${msg}You do not have permission for this. Ask an owner or admin of the organization.`;
      case 'not_found':
        return `${lead}${msg || 'It is not there any more. '}It may have been removed or renamed. Reload to see the latest.`;
      case 'conflict':
        return `${lead}${msg || 'Someone else changed this at the same time. '}Reload and try again.`;
      case 'invalid':
        return `${lead}${msg || 'Something in the form is not right. '}Check the fields and try again.`;
      default:
        return `${lead}${msg || 'The server had a problem. '}Try again in a moment; if it keeps happening, tell whoever runs the server.`;
    }
  }
  return `${lead}Something unexpected happened. Reload the page and try again.`;
}

function ensureStop(s: string) {
  return /[.!?]$/.test(s.trim()) ? s.trim() : s.trim() + '.';
}

let unauthorizedHandler: (() => void) | null = null;
export function onUnauthorized(fn: () => void) {
  unauthorizedHandler = fn;
}

async function transport(url: string, init: RequestInit): Promise<Response> {
  if (MOCK) {
    const m = await loadMock();
    return m.mockFetch(url, init);
  }
  return fetch(url, init);
}

type Query = Record<string, string | number | boolean | undefined | null>;

function qs(q?: Query) {
  if (!q) return '';
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(q)) if (v !== undefined && v !== null && v !== '') p.set(k, String(v));
  const s = p.toString();
  return s ? `?${s}` : '';
}

const enc = encodeURIComponent;

export async function request<T>(method: string, path: string, body?: unknown, query?: Query): Promise<T> {
  const headers: Record<string, string> = { Accept: 'application/json' };
  const token = getToken();
  if (token) headers.Authorization = `Bearer ${token}`;
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  let res: Response;
  try {
    res = await transport(`/api${path}${qs(query)}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new ApiError(0, 'network', 'The server did not answer.');
  }
  if (res.status === 204) return {} as T;
  const text = await res.text();
  let data: unknown = undefined;
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      data = undefined;
    }
  }
  if (!res.ok) {
    const e = (data as { error?: { code?: string; message?: string; details?: unknown } } | undefined)?.error;
    const code = e?.code || (res.status === 401 ? 'unauthorized' : res.status === 404 ? 'not_found' : 'server');
    const err = new ApiError(res.status, code, e?.message || '', e?.details);
    if (res.status === 401 && !path.startsWith('/auth/login') && !path.startsWith('/invites/')) {
      setToken(null);
      unauthorizedHandler?.();
    }
    throw err;
  }
  return data as T;
}

/** A GET with the bearer token, as a raw response (images, file bytes). */
export async function fetchAuthed(path: string): Promise<Response> {
  const headers: Record<string, string> = {};
  const token = getToken();
  if (token) headers.Authorization = `Bearer ${token}`;
  return transport(path, { headers });
}

/** Fetch a file with the bearer token and hand it to the browser as a download. */
export async function downloadFile(projectId: string, branchId: string, path: string, change?: number) {
  const headers: Record<string, string> = {};
  const token = getToken();
  if (token) headers.Authorization = `Bearer ${token}`;
  let res: Response;
  try {
    res = await transport(`/api/projects/${enc(projectId)}/download${qs({ path, branch: branchId, change })}`, { headers });
  } catch {
    throw new ApiError(0, 'network', 'The server did not answer.');
  }
  if (!res.ok) throw new ApiError(res.status, res.status === 404 ? 'not_found' : 'server', '');
  const blob = await res.blob();
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  const name = path.split('/').pop() || 'file';
  const dot = name.lastIndexOf('.');
  a.download = change === undefined ? name : dot > 0 ? `${name.slice(0, dot)} (change ${change})${name.slice(dot)}` : `${name} (change ${change})`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 10_000);
}

const p = (id: string) => `/projects/${enc(id)}`;

export const api = {
  // auth
  login: (email: string, password: string) =>
    request<LoginResponse>('POST', '/auth/login', { email, password, device: 'Web browser' }),
  logout: () => request<object>('POST', '/auth/logout'),
  me: () => request<Me>('GET', '/me'),
  serverInfo: () => request<{ email: boolean }>('GET', '/server'),
  forgotPassword: (email: string) => request<{ sent: boolean }>('POST', '/auth/forgot', { email }),
  resetPassword: (token: string, password: string) => request<{ email: string }>('POST', '/auth/reset', { token, password }),
  desktop: () =>
    request<{ version: string | null; notes?: string; downloads: Array<{ platform: string; name: string; url: string; size?: number | null }> }>('GET', '/desktop'),
  invite: (token: string) => request<InviteInfo>('GET', `/invites/${enc(token)}`),
  acceptInvite: (token: string, body: { name: string; password: string } | Record<string, never>) =>
    request<LoginResponse>('POST', `/invites/${enc(token)}/accept`, body),

  // orgs and projects
  orgs: () => request<Org[]>('GET', '/orgs'),
  createOrg: (name: string, slug?: string) => request<Org>('POST', '/orgs', { name, slug: slug || undefined }),
  projects: () => request<ProjectSummary[]>('GET', '/projects'),
  createProject: (org: string, name: string, slug?: string, storage?: string) =>
    request<Project>('POST', `/orgs/${enc(org)}/projects`, { name, slug: slug || undefined, storage: storage || undefined }),
  project: (id: string) => request<Project>('GET', p(id)),

  // branches
  branches: (pid: string) => request<Branch[]>('GET', `${p(pid)}/branches`),
  createBranch: (pid: string, name: string, from: string, shares_claims: boolean) =>
    request<Branch>('POST', `${p(pid)}/branches`, { name, from, shares_claims }),
  updateBranch: (pid: string, bid: string, resolutions?: Record<string, 'mine' | 'parent'>) =>
    request<MergeReport>('POST', `${p(pid)}/branches/${enc(bid)}/update`, resolutions ? { resolutions } : {}),
  mergePreview: (pid: string, bid: string) => request<MergeReport>('GET', `${p(pid)}/branches/${enc(bid)}/merge-preview`),
  merge: (pid: string, bid: string, message: string) =>
    request<{ change: Change }>('POST', `${p(pid)}/branches/${enc(bid)}/merge`, { message }),
  archiveBranch: (pid: string, bid: string) => request<Branch>('POST', `${p(pid)}/branches/${enc(bid)}/archive`),

  // files and changes
  files: (pid: string, bid: string, q?: { prefix?: string; tag?: string; q?: string }) =>
    request<FileEntry[]>('GET', `${p(pid)}/branches/${enc(bid)}/files`, undefined, q),
  changes: (pid: string, q?: { branch?: string; before?: number; limit?: number }) =>
    request<Change[]>('GET', `${p(pid)}/changes`, undefined, q),
  change: (pid: string, n: number) => request<ChangeDetail>('GET', `${p(pid)}/changes/${n}`),
  history: (pid: string, q: { branch: string; prefix?: string; limit?: number }) =>
    request<History>('GET', `${p(pid)}/history`, undefined, q),
  versions: (pid: string, branch: string, path: string) =>
    request<{ path: string; versions: FileVersion[] }>('GET', `${p(pid)}/versions`, undefined, { path, branch }),
  restoreVersion: (pid: string, bid: string, path: string, change: number, message?: string) =>
    request<{ change: Change }>('POST', `${p(pid)}/branches/${enc(bid)}/restore-version`, { path, change, message }),

  // claims
  claims: (pid: string) => request<Claim[]>('GET', `${p(pid)}/claims`),
  claim: (pid: string, branch: string, paths: string[]) =>
    request<ClaimResponse>('POST', `${p(pid)}/claims`, { branch, paths, device: 'Web browser' }),
  release: (pid: string, branch: string, paths: string[]) =>
    request<{ released: string[] }>('POST', `${p(pid)}/claims/release`, { branch, paths }),
  nudge: (pid: string, branch: string, path: string) =>
    request<object>('POST', `${p(pid)}/claims/nudge`, { branch, path }),

  // tags
  tags: (pid: string) => request<Tag[]>('GET', `${p(pid)}/tags`),
  createTag: (pid: string, body: { name: string; color: string; group?: string; project_only?: boolean }) =>
    request<Tag>('POST', `${p(pid)}/tags`, body),
  applyTag: (pid: string, tag: string, paths: string[]) =>
    request<object>('POST', `${p(pid)}/tags/apply`, { tag, paths }),
  removeTag: (pid: string, tag: string, paths: string[]) =>
    request<object>('POST', `${p(pid)}/tags/remove`, { tag, paths }),

  // drafts
  drafts: (pid: string) => request<Draft[]>('GET', `${p(pid)}/drafts`),
  draft: (pid: string, id: string) => request<DraftDetail>('GET', `${p(pid)}/drafts/${enc(id)}`),
  deleteDraft: (pid: string, id: string) => request<object>('DELETE', `${p(pid)}/drafts/${enc(id)}`),

  // admin
  admin: {
    overview: (org?: string) => request<AdminOverview>('GET', '/admin/overview', undefined, { org }),
    users: (q?: { q?: string; org?: string; status?: string }) =>
      request<AdminUser[]>('GET', '/admin/users', undefined, q),
    user: (id: string) => request<AdminUserDetail>('GET', `/admin/users/${enc(id)}`),
    suspend: (id: string, reason?: string) =>
      request<AdminUser>('POST', `/admin/users/${enc(id)}/suspend`, { reason: reason || undefined }),
    reactivate: (id: string) => request<AdminUser>('POST', `/admin/users/${enc(id)}/reactivate`),
    signOut: (id: string) => request<{ revoked: number }>('POST', `/admin/users/${enc(id)}/sign-out`),
    deleteUser: (id: string, confirm: string, transfer_to?: string) =>
      request<object>('DELETE', `/admin/users/${enc(id)}`, { confirm, transfer_to: transfer_to || undefined }),
    setPassword: (id: string, password: string) =>
      request<object>('POST', `/admin/users/${enc(id)}/password`, { password }),
    members: (org: string) => request<Member[]>('GET', `/admin/orgs/${enc(org)}/members`),
    updateMember: (org: string, user: string, body: { role?: Role; expires_at?: string | null }) =>
      request<Member>('PATCH', `/admin/orgs/${enc(org)}/members/${enc(user)}`, body),
    removeMember: (org: string, user: string) => request<object>('DELETE', `/admin/orgs/${enc(org)}/members/${enc(user)}`),
    invites: (org: string) => request<Invite[]>('GET', `/admin/orgs/${enc(org)}/invites`),
    createInvites: (org: string, body: { emails?: string[]; role: Role; expires_in_days?: number }) =>
      request<Invite[]>('POST', `/admin/orgs/${enc(org)}/invites`, body),
    revokeInvite: (id: string) => request<object>('DELETE', `/admin/invites/${enc(id)}`),
    projects: (org?: string) => request<ProjectSummary[]>('GET', '/admin/projects', undefined, { org }),
    updateProject: (pid: string, body: { name?: string; org?: string; checkpoints?: CheckpointSetting; auto_checkpoints?: boolean }) =>
      request<Project>('PATCH', `/admin/projects/${enc(pid)}`, body),
    archiveProject: (pid: string) => request<Project>('POST', `/admin/projects/${enc(pid)}/archive`),
    restoreProject: (pid: string) => request<Project>('POST', `/admin/projects/${enc(pid)}/restore`),
    claims: (q?: { older_than_hours?: number; org?: string }) =>
      request<ClaimWithProject[]>('GET', '/admin/claims', undefined, q),
    releaseClaims: (claims: Array<{ project: string; branch: string; path: string }>) =>
      request<{ released: number }>('POST', '/admin/claims/release', { claims }),
    storageCheck: (project?: string) =>
      request<StorageCheckReport>('POST', '/admin/storage/check', project ? { project } : {}),
    gcDryRun: () => request<GcDryRun>('GET', '/admin/storage/gc'),
    gcRun: () => request<GcResult>('POST', '/admin/storage/gc'),
    storage: () => request<StorageOverview>('GET', '/admin/storage'),
    backups: () => request<BackupStatus>('GET', '/admin/backups'),
    email: () => request<{ configured: boolean; kind?: string; via?: string; from?: string }>('GET', '/admin/email'),
    emailTest: () => request<{ sent_to: string }>('POST', '/admin/email/test'),
    backupNow: () => request<{ started: boolean }>('POST', '/admin/backups/run'),
    projectStorage: (pid: string) => request<ProjectStorage>('GET', `/admin/projects/${enc(pid)}/storage`),
    moveProjectStorage: (pid: string, to: string) =>
      request<StorageMove>('POST', `/admin/projects/${enc(pid)}/storage`, { to }),
    checkpoints: (org?: string) => request<CheckpointUsage[]>('GET', '/admin/checkpoints', undefined, { org }),
    clearCheckpoints: (body: ClearCheckpoints) =>
      request<{ removed: number }>('POST', '/admin/checkpoints/clear', body),
    audit: (q?: { org?: string; project?: string; actor?: string; kind?: string; before?: number; limit?: number }) =>
      request<MpEvent[]>('GET', '/admin/audit', undefined, q),
  },
};
