// Mock server and mock daemon. Loaded only with a dynamic import when mock mode is on.
import type {
  AdminUser,
  Branch,
  CheckpointUsage,
  Claim,
  FileEntry,
  FolderNode,
  Member,
  MpEvent,
  Org,
  Project,
  ProjectSummary,
  Role,
  StorageMove,
} from '../types';
import {
  CINDER,
  DAY,
  GB,
  ME,
  MB,
  claims,
  drafts,
  events,
  filesOf,
  hash,
  invites,
  kindOf,
  leftovers,
  locations,
  members,
  orgs,
  pathTags,
  projectById,
  projects,
  pushEvent,
  scopeOf,
  serverCheckpoints,
  sessions,
  storageMoves,
  tags,
  uid,
  userById,
  userRef,
  users,
  workspaces,
  type MChange,
  type MMove,
  type MProject,
  clients,
  productions,
  type MUser,
  type MWorkspace,
} from './data';

class Fail extends Error {
  constructor(public status: number, public code: string, message: string, public details?: unknown) {
    super(message);
  }
}

const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

// ---------- auth ----------

const TOK = 'mp.mock.tokens';
function tokens(): Record<string, string> {
  try {
    return JSON.parse(sessionStorage.getItem(TOK) || '{}');
  } catch {
    return {};
  }
}
function saveTokens(t: Record<string, string>) {
  try {
    sessionStorage.setItem(TOK, JSON.stringify(t));
  } catch {
    /* ignore */
  }
}
function issue(user: MUser) {
  const t = `mock-${user.id.slice(0, 8)}-${Math.random().toString(36).slice(2)}`;
  saveTokens({ ...tokens(), [t]: user.id });
  return t;
}

function who(headers: Headers, query?: URLSearchParams): MUser {
  const h = headers.get('Authorization') || '';
  const t = h.startsWith('Bearer ') ? h.slice(7) : query?.get('token') || '';
  const id = tokens()[t];
  const u = id ? userById(id) : undefined;
  if (!u) throw new Fail(401, 'unauthorized', 'Sign in to continue.');
  if (u.status === 'suspended') throw new Fail(403, 'suspended', 'This account is suspended.');
  return u;
}

const orgWithCounts = (o: Org): Org => ({
  ...o,
  member_count: members.filter((m) => m.org_id === o.id).length,
  project_count: projects.filter((p) => p.org_id === o.id && !p.archived).length,
});
const orgsOf = (u: MUser) =>
  members.filter((m) => m.user_id === u.id).map((m) => ({ org: orgWithCounts(orgs.find((o) => o.id === m.org_id)!), role: m.role }));
const meOf = (u: MUser) => ({ id: u.id, email: u.email, name: u.name, is_server_admin: u.is_server_admin, status: u.status, orgs: orgsOf(u) });
const roleIn = (u: MUser, orgId: string): Role | undefined => members.find((m) => m.user_id === u.id && m.org_id === orgId)?.role;
const canAdminOrg = (u: MUser, orgId: string) => u.is_server_admin || ['owner', 'admin'].includes(roleIn(u, orgId) || '');
function needAdmin(u: MUser, orgId?: string | null) {
  if (u.is_server_admin) return;
  if (orgId && canAdminOrg(u, orgId)) return;
  if (!orgId && orgsOf(u).some((o) => o.role === 'owner' || o.role === 'admin'))
    throw new Fail(422, 'invalid', 'Choose an organization: organization admins act on one organization at a time.');
  throw new Fail(403, 'forbidden', 'Only admins can do this.');
}
const adminOrgIds = (u: MUser, org?: string | null) =>
  org ? [org] : u.is_server_admin ? orgs.map((o) => o.id) : orgsOf(u).filter((o) => o.role === 'owner' || o.role === 'admin').map((o) => o.org.id);

// ---------- projects ----------

function visibleProjects(u: MUser) {
  return projects.filter((p) => u.is_server_admin || roleIn(u, p.org_id));
}
function getProject(u: MUser, id: string): MProject {
  const p = projectById(id);
  if (!p || !visibleProjects(u).includes(p)) throw new Fail(404, 'not_found', 'That project does not exist, or you cannot see it.');
  return p;
}
function getBranch(p: MProject, id: string | null | undefined): Branch {
  const b = id ? p.branches.find((x) => x.id === id || x.name.toLowerCase() === id.toLowerCase()) : p.branches[0];
  if (!b) throw new Fail(404, 'not_found', 'That branch does not exist.');
  return b;
}
function summary(p: MProject, u: MUser): ProjectSummary {
  const main = filesOf(p, p.branches[0].id);
  const last = p.changes.filter((c) => c.branch_id === p.branches[0].id).sort((a, b) => b.number - a.number)[0];
  const pc = claims.filter((c) => c.project_id === p.id);
  const bytes = main.reduce((a, f) => a + f.size, 0);
  return {
    id: p.id, slug: p.slug, name: p.name, org: orgWithCounts(orgs.find((o) => o.id === p.org_id)!), archived: p.archived,
    head_change: p.head, size: bytes, file_count: main.length,
    my_claims: pc.filter((c) => c.user.id === u.id).length, claims: pc.length,
    last_change: last ? { number: last.number, message: last.message, author: last.author, created_at: last.created_at } : undefined,
    branches: p.branches.filter((b) => !b.archived).length,
    checkpoint_setting: p.checkpoints,
    auto_checkpoints: !!p.auto_checkpoints,
    checkpoint_storage: p.checkpoints === 'auto' ? (bytes >= 20 * GB ? 'server' : 'local') : p.checkpoints,
    storage: p.storage,
    storage_moving: advanceMoves().some((x) => x.project_id === p.id && x.status === 'running'),
    ...(() => {
      const c = clients.find((x) => x.id === p.client_id);
      const pr = productions.find((x) => x.id === p.production_id);
      return {
        client: c ? { id: c.id, name: c.name } : null,
        production: pr ? { id: pr.id, name: pr.name, main_project_id: pr.main_project_id } : null,
        is_main: !!pr && pr.main_project_id === p.id,
      };
    })(),
    theme: (() => {
      const o = orgs.find((x) => x.id === p.org_id) as { theme_light?: string | null; theme_dark?: string | null } | undefined;
      const q = p as unknown as { theme_light?: string | null; theme_dark?: string | null };
      return { light: q.theme_light || o?.theme_light || null, dark: q.theme_dark || o?.theme_dark || null, own_light: q.theme_light || null, own_dark: q.theme_dark || null };
    })(),
  };
}

// ---------- storage locations ----------

const MOVE_MS = 10_000;
/** Play running moves forward to now. */
function advanceMoves() {
  for (const mv of storageMoves) {
    if (mv.status !== 'running') continue;
    const f = Math.min(1, (Date.now() - mv.ms) / MOVE_MS);
    mv.copied_chunks = Math.max(mv.copied_chunks, Math.round(mv.total_chunks * f));
    mv.copied_bytes = Math.max(mv.copied_bytes, Math.round(mv.total_bytes * f));
    if (f >= 1) {
      mv.status = 'done';
      mv.finished_at = new Date().toISOString();
      const p = projectById(mv.project_id);
      if (p) p.storage_previous = null;
      const left = leftovers.get(mv.from) || { chunks: 0, bytes: 0 };
      leftovers.set(mv.from, { chunks: left.chunks + mv.total_chunks, bytes: left.bytes + mv.total_bytes });
    }
  }
  return storageMoves;
}
const latestMove = (pid: string) => storageMoves.find((mv) => mv.project_id === pid);
const projectChunks = (p: MProject) => filesOf(p, p.branches[0].id).length * 37;
const projectStored = (p: MProject) => Math.round(filesOf(p, p.branches[0].id).reduce((b, f) => b + f.size * 1.6, 0));
const moveOut = ({ ms: _m, total_bytes: _t, ...mv }: MMove): StorageMove => (void _m, void _t, { ...mv });

function locationsOut(u: MUser) {
  advanceMoves();
  return locations.map((l) => {
    const here = projects.filter((p) => p.storage === l.name);
    // a project part-way through a move still has file data in the old place
    const open = storageMoves.filter((mv) => mv.status !== 'done' && mv === latestMove(mv.project_id));
    const halfway = open.filter((mv) => mv.from === l.name && projectById(mv.project_id)?.storage_previous === l.name);
    const copying = open.filter((mv) => mv.to === l.name);
    const left = leftovers.get(l.name) || { chunks: 0, bytes: 0 };
    let chunks = left.chunks;
    let bytes = left.bytes;
    for (const p of here) {
      const mv = copying.find((x) => x.project_id === p.id);
      chunks += mv ? mv.copied_chunks : projectChunks(p);
      bytes += mv ? mv.copied_bytes : projectStored(p);
    }
    for (const mv of halfway) {
      const p = projectById(mv.project_id)!;
      chunks += projectChunks(p);
      bytes += projectStored(p);
    }
    return {
      name: l.name, kind: l.kind, main: l.main, description: u.is_server_admin ? l.description : null, chunks, stored_bytes: bytes,
      projects: here.map((p) => ({ id: p.id, name: p.name, org: orgs.find((o) => o.id === p.org_id)!.name })),
    };
  });
}
function fullProject(p: MProject, u: MUser): Project {
  return { ...summary(p, u), branches_list: p.branches, my_role: roleIn(u, p.org_id) || 'admin' };
}

function claimsFor(p: MProject, b: Branch) {
  const scope = scopeOf(p, b);
  return claims.filter((c) => c.project_id === p.id && scopeOf(p, getBranch(p, c.branch_id)) === scope);
}

function tagsFor(p: MProject) {
  return tags.filter((t) => t.org_id === p.org_id && (!t.project_id || t.project_id === p.id));
}

function fileEntries(p: MProject, b: Branch, q: URLSearchParams): FileEntry[] {
  const prefix = q.get('prefix') || '';
  const tag = q.get('tag');
  const text = (q.get('q') || '').toLowerCase();
  const cl = claimsFor(p, b);
  const tg = tagsFor(p);
  return filesOf(p, b.id)
    .filter((f) => !prefix || f.path.startsWith(prefix.endsWith('/') ? prefix : prefix + '/'))
    .map((f) => {
      const c = cl.find((x) => x.path === f.path);
      const ft = pathTags
        .filter((pt) => pt.project_id === p.id && (pt.path === f.path || f.path.startsWith(pt.path + '/')))
        .map((pt) => tg.find((t) => t.id === pt.tag_id))
        .filter(Boolean)
        .map((t) => ({ id: t!.id, name: t!.name, color: t!.color }));
      return {
        path: f.path, size: f.size, file_hash: f.file_hash, change: f.changes[f.changes.length - 1], kind: kindOf(f.path),
        claim: c ? { user: c.user, since: c.created_at, device: c.device } : undefined, tags: ft,
      };
    })
    .filter((f) => !tag || f.tags.some((t) => t.id === tag || t.name === tag))
    .filter((f) => !text || f.path.toLowerCase().includes(text));
}

function emit(e: Omit<MpEvent, 'id' | 'created_at'>) {
  const full = pushEvent(e);
  for (const s of subs) if (!e.project_id || s.project === e.project_id) s.fn(full);
  return full;
}

function publishChange(p: MProject, b: Branch, author: MUser, message: string, paths: Array<{ path: string; size: number; op: 'add' | 'edit' | 'delete' }>, kind: MChange['kind'] = 'publish', merged_from?: number[]): MChange {
  p.head += 1;
  const files = filesOf(p, b.id);
  for (const x of paths) {
    const f = files.find((y) => y.path === x.path);
    if (x.op === 'delete') {
      if (f) files.splice(files.indexOf(f), 1);
    } else if (f) {
      f.changes.push(p.head);
      f.size = x.size;
      f.file_hash = hash();
    } else files.push({ path: x.path, size: x.size, file_hash: hash(), changes: [p.head] });
  }
  b.head_change = p.head;
  const ch: MChange = {
    number: p.head, branch_id: b.id, message, author: userRef(author), created_at: new Date().toISOString(), kind,
    file_count: paths.length, size: paths.reduce((a, x) => a + x.size, 0), merged_from, files: paths.map((x) => ({ ...x })),
  };
  p.changes.unshift(ch);
  if (b.parent_id) { if (kind !== 'update') b.ahead += paths.length; }
  else for (const other of p.branches) if (other.parent_id === b.id && !other.archived) other.behind += paths.length;
  emit({ kind: kind === 'merge' ? 'change.merged' : 'change.published', org_id: p.org_id, project_id: p.id, actor: userRef(author), data: { change: ch.number, branch: b.id, files: paths.length } });
  return ch;
}

// ---------- folder trees for the daemon ----------

const isIncluded = (path: string, inc: string[], exc: string[]) => {
  let best = -1;
  let val = false;
  const test = (rule: string, v: boolean) => {
    if (rule === '' || path === rule || path.startsWith(rule + '/')) {
      if (rule.length > best) {
        best = rule.length;
        val = v;
      }
    }
  };
  inc.forEach((r) => test(r, true));
  exc.forEach((r) => test(r, false));
  return val;
};

function folderTree(ws: MWorkspace): FolderNode {
  const p = projectById(ws.project_id)!;
  const files = filesOf(p, ws.branch_id);
  const root: FolderNode = { path: '', name: p.name, size: 0, local_size: 0, included: 'none', children: [] };
  const leafFiles = new Map<FolderNode, { size: number; inc: boolean }[]>();
  for (const f of files) {
    const parts = f.path.split('/');
    let node = root;
    for (let i = 0; i < parts.length - 1; i++) {
      const path = parts.slice(0, i + 1).join('/');
      let child = node.children.find((c) => c.path === path);
      if (!child) {
        child = { path, name: parts[i], size: 0, local_size: 0, included: 'none', children: [] };
        node.children.push(child);
      }
      node = child;
    }
    const inc = isIncluded(f.path, ws.include, ws.exclude);
    if (!leafFiles.has(node)) leafFiles.set(node, []);
    leafFiles.get(node)!.push({ size: f.size, inc });
  }
  const walk = (n: FolderNode): { all: boolean; none: boolean } => {
    let all = true;
    let none = true;
    n.size = 0;
    n.local_size = 0;
    for (const lf of leafFiles.get(n) || []) {
      n.size += lf.size;
      if (lf.inc) {
        n.local_size += lf.size;
        none = false;
      } else all = false;
    }
    n.children.sort((a, b) => a.name.localeCompare(b.name));
    for (const c of n.children) {
      const r = walk(c);
      n.size += c.size;
      n.local_size += c.local_size;
      if (!r.all) all = false;
      if (!r.none) none = false;
    }
    n.included = all ? 'all' : none ? 'none' : 'some';
    return { all, none };
  };
  walk(root);
  return root;
}

function refreshWs(ws: MWorkspace) {
  const t = folderTree(ws);
  ws.total_bytes = t.size;
  ws.local_bytes = t.local_size;
  return ws;
}
workspaces.forEach(refreshWs);
const wsOut = (ws: MWorkspace) => {
  const { status: _s, ...rest } = refreshWs(ws);
  void _s;
  return rest;
};

// ---------- router ----------

type Ctx = { m: RegExpMatchArray; body: any; q: URLSearchParams; headers: Headers };
type Handler = (c: Ctx) => unknown;
const routes: Array<[string, RegExp, Handler]> = [];
const on = (method: string, pattern: string, h: Handler) =>
  routes.push([method, new RegExp('^' + pattern.replace(/:(\w+)/g, '([^/]+)') + '$'), h]);

on('POST', '/api/auth/login', ({ body }) => {
  const u = users.find((x) => x.email.toLowerCase() === String(body?.email || '').trim().toLowerCase());
  if (!u || !body?.password) throw new Fail(401, 'unauthorized', 'That email and password do not match an account on this server.');
  if (u.status === 'suspended') throw new Fail(403, 'suspended', 'This account is suspended.');
  u.last_seen_at = new Date().toISOString();
  return { token: issue(u), user: meOf(u) };
});
on('POST', '/api/auth/logout', ({ headers }) => {
  const t = (headers.get('Authorization') || '').slice(7);
  const all = tokens();
  delete all[t];
  saveTokens(all);
  return {};
});
on('GET', '/api/me', ({ headers }) => meOf(who(headers)));
on('GET', '/api/server', () => ({ email: true }));
on('POST', '/api/auth/forgot', () => ({ sent: true }));
on('POST', '/api/auth/reset', ({ body }) => {
  if (String(body?.password || '').length < 10) throw new Fail(422, 'invalid', 'Use a password of at least 10 characters.');
  if (body?.token === 'used') throw new Fail(422, 'invalid', 'This link has expired or was already used. Ask for a new one.');
  return { email: 'justin@cinderworks.example' };
});
const mockHooks: Array<Record<string, unknown>> = [];
on('GET', '/api/admin/orgs/:org/webhooks', () => mockHooks);
on('POST', '/api/admin/orgs/:org/webhooks', ({ body }) => {
  const secret = 'whsec_' + Math.random().toString(36).slice(2) + Math.random().toString(36).slice(2);
  const h = { id: 'wh' + (mockHooks.length + 1), url: body?.url, events: body?.events || [], project_id: null, active: true, secret_hint: '…' + secret.slice(-4),
    created_at: new Date().toISOString(), last_at: null, last_status: null, last_error: null, failures: 0 };
  mockHooks.push(h);
  return { ...h, secret };
});
on('PATCH', '/api/admin/webhooks/:id', ({ m, body }) => { const h = mockHooks.find((x) => x.id === m[1])!; Object.assign(h, body); return h; });
on('DELETE', '/api/admin/webhooks/:id', ({ m }) => { mockHooks.splice(mockHooks.findIndex((x) => x.id === m[1]), 1); return {}; });
on('POST', '/api/admin/webhooks/:id/test', ({ m }) => { const h = mockHooks.find((x) => x.id === m[1])!; Object.assign(h, { last_at: new Date().toISOString(), last_status: 204, last_error: null }); return { ok: true, status: 204, error: null }; });
on('GET', '/api/admin/email', () => ({ configured: true, kind: 'smtp', via: 'SMTP smtp.example.com:587 (starttls)', from: 'MultiParity <multiparity@cinderworks.example>' }));
on('POST', '/api/admin/email/test', ({ headers }) => ({ sent_to: who(headers).email }));
on('GET', '/api/desktop', () => ({
  version: '0.2.0', notes: 'Files, History, Branches and Drafts in the app; notifications; updates itself.',
  downloads: [
    { platform: 'windows-x86_64', name: 'MultiParity_0.2.0_x64-setup.exe', url: '#', size: 9_400_000 },
    { platform: 'darwin-aarch64', name: 'MultiParity_0.2.0_aarch64.dmg', url: '#', size: 11_200_000 },
  ],
}));
on('GET', '/api/invites/:token', ({ m }) => {
  const inv = invites.find((i) => i.token === m[1]);
  if (!inv || inv.accepted_at) throw new Fail(404, 'not_found', 'This invite link has been used or revoked.');
  if (inv.expires_at && Date.parse(inv.expires_at) < Date.now()) throw new Fail(404, 'not_found', 'This invite link has expired.');
  return { org: orgWithCounts(orgs.find((o) => o.id === inv.org_id)!), role: inv.role, email: inv.email, expires_at: inv.expires_at };
});
on('POST', '/api/invites/:token/accept', ({ m, body, headers }) => {
  const inv = invites.find((i) => i.token === m[1]);
  if (!inv || inv.accepted_at) throw new Fail(404, 'not_found', 'This invite link has been used or revoked.');
  let u: MUser;
  if (body?.name) {
    if (String(body.password || '').length < 8) throw new Fail(422, 'invalid', 'The password needs at least 8 characters.');
    u = {
      id: uid('usr00010'), name: body.name, email: inv.email || `${String(body.name).toLowerCase().replace(/\W+/g, '.')}@invited.example`,
      status: 'active', is_server_admin: false, created_at: new Date().toISOString(), last_seen_at: new Date().toISOString(),
    };
    users.push(u);
    sessions.set(u.id, []);
  } else u = who(headers);
  if (!roleIn(u, inv.org_id)) members.push({ org_id: inv.org_id, user_id: u.id, role: inv.role, created_at: new Date().toISOString() });
  inv.accepted_at = new Date().toISOString();
  emit({ kind: 'member.added', org_id: inv.org_id, actor: userRef(u), data: { user: u.name, role: inv.role } });
  return { token: issue(u), user: meOf(u) };
});

on('PATCH', '/api/admin/orgs/:org', ({ headers, m, body }) => {
  const u = who(headers);
  const o = orgs.find((x) => x.id === m[1]);
  if (!o) throw new Fail(404, 'not_found', 'That organization does not exist.');
  needAdmin(u, o.id);
  if (body?.name) o.name = String(body.name).trim();
  for (const k of ['theme_light', 'theme_dark'] as const) if (typeof body?.[k] === 'string') (o as unknown as Record<string, string | null>)[k] = body[k] || null;
  return orgWithCounts(o);
});
on('GET', '/api/orgs', ({ headers }) => {
  const u = who(headers);
  return (u.is_server_admin ? orgs : orgsOf(u).map((o) => o.org)).map(orgWithCounts);
});
on('POST', '/api/orgs', ({ headers, body }) => {
  const u = who(headers);
  if (!u.is_server_admin) throw new Fail(403, 'forbidden', 'Only server admins can create organizations.');
  const name = String(body?.name || '').trim();
  if (!name) throw new Fail(422, 'invalid', 'Give the organization a name.');
  const slug = body?.slug || name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
  if (orgs.some((o) => o.slug === slug)) throw new Fail(409, 'conflict', `An organization with the address ${slug} already exists.`);
  const o: Org = { id: uid('org00009'), slug, name, created_at: new Date().toISOString() };
  orgs.push(o);
  members.push({ org_id: o.id, user_id: u.id, role: 'owner', created_at: o.created_at });
  emit({ kind: 'admin.org.created', org_id: o.id, actor: userRef(u), data: { org: name } });
  return orgWithCounts(o);
});
on('GET', '/api/projects', ({ headers }) => {
  const u = who(headers);
  return visibleProjects(u).map((p) => summary(p, u));
});
on('POST', '/api/orgs/:org/projects', ({ headers, m, body }) => {
  const u = who(headers);
  const o = orgs.find((x) => x.id === m[1] || x.slug === m[1]);
  if (!o) throw new Fail(404, 'not_found', 'That organization does not exist.');
  if (!canAdminOrg(u, o.id)) throw new Fail(403, 'forbidden', 'Only organization admins can create projects.');
  const name = String(body?.name || '').trim();
  if (!name) throw new Fail(422, 'invalid', 'Give the project a name.');
  const storage = String(body?.storage || 'main');
  if (!locations.some((l) => l.name === storage)) throw new Fail(422, 'invalid', `The server has no storage location called ${storage}.`);
  const id = uid('pr000009');
  const p: MProject = {
    id, slug: body?.slug || name.toLowerCase().replace(/[^a-z0-9]+/g, '-'), name, org_id: o.id, archived: false, checkpoints: 'auto',
    storage, storage_previous: null,
    created_at: new Date().toISOString(), head: 0, files: new Map(), changes: [], merge: new Map(),
    branches: [{ id: uid('brn00009'), name: 'Main', parent_id: null, shares_claims: true, protected: false, archived: false, head_change: 0, created_at: new Date().toISOString(), created_by: userRef(u), ahead: 0, behind: 0 }],
  };
  p.files.set(p.branches[0].id, []);
  projects.push(p);
  if (body?.client || body?.production) place(p, body.client || null, body.production || null);
  emit({ kind: 'project.created', org_id: o.id, project_id: p.id, actor: userRef(u), data: { project: name } });
  return fullProject(p, u);
});
on('GET', '/api/projects/:p', ({ headers, m }) => {
  const u = who(headers);
  return fullProject(getProject(u, m[1]), u);
});

// branches
on('GET', '/api/projects/:p/branches', ({ headers, m }) => getProject(who(headers), m[1]).branches);
on('POST', '/api/projects/:p/branches', ({ headers, m, body }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  const from = getBranch(p, body?.from);
  const name = String(body?.name || '').trim();
  if (!name) throw new Fail(422, 'invalid', 'Give the branch a name.');
  if (p.branches.some((b) => b.name.toLowerCase() === name.toLowerCase() && !b.archived)) throw new Fail(409, 'conflict', `There is already a branch called ${name}.`);
  const b: Branch = {
    id: uid('brn00010'), name, parent_id: from.id, shares_claims: body?.shares_claims !== false, protected: false, archived: false,
    head_change: p.head, created_at: new Date().toISOString(), created_by: userRef(u), ahead: 0, behind: 0,
  };
  p.branches.push(b);
  p.merge.set(b.id, { updated: true, only_here: 0, only_parent: 0, auto: [], conflicts: [] });
  emit({ kind: 'branch.created', org_id: p.org_id, project_id: p.id, actor: userRef(u), data: { branch: name, from: from.name } });
  return b;
});
function report(p: MProject, b: Branch) {
  const s = p.merge.get(b.id) || { updated: b.behind === 0, only_here: b.ahead, only_parent: b.behind, auto: [], conflicts: [] };
  return {
    up_to_date: s.updated && b.behind === 0 && s.conflicts.length === 0,
    only_here: b.ahead, only_parent: b.behind, auto_merged: s.auto, conflicts: s.conflicts,
  };
}
on('POST', '/api/projects/:p/branches/:b/update', async ({ headers, m, body }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  const b = getBranch(p, m[2]);
  if (!b.parent_id) throw new Fail(422, 'invalid', 'Main has no parent branch to update from.');
  await wait(500);
  const s = p.merge.get(b.id) || { updated: false, only_here: b.ahead, only_parent: b.behind, auto: [], conflicts: [] };
  const res = (body?.resolutions || {}) as Record<string, string>;
  s.conflicts = s.conflicts.filter((c) => !res[c.path]);
  if (s.conflicts.length) return { ...report(p, b), up_to_date: false };
  const brought = b.behind;
  b.behind = 0;
  s.updated = true;
  p.merge.set(b.id, s);
  if (brought) publishChange(p, b, u, `Update from ${getBranch(p, b.parent_id).name}`, [{ path: 'Config/DefaultGame01.ini', size: 2048, op: 'edit' }], 'update');
  return report(p, b);
});
on('GET', '/api/projects/:p/branches/:b/merge-preview', async ({ headers, m }) => {
  const p = getProject(who(headers), m[1]);
  await wait(250);
  return report(p, getBranch(p, m[2]));
});
on('POST', '/api/projects/:p/branches/:b/merge', async ({ headers, m, body }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  const b = getBranch(p, m[2]);
  if (!b.parent_id) throw new Fail(422, 'invalid', 'Main cannot be merged into anything.');
  const r = report(p, b);
  if (!r.up_to_date) throw new Fail(409, 'conflict', `This branch has to be updated from ${getBranch(p, b.parent_id).name} first.`);
  if (b.protected && !u.is_server_admin) throw new Fail(403, 'forbidden', 'This branch is protected.');
  await wait(600);
  const parent = getBranch(p, b.parent_id);
  const from = p.changes.filter((c) => c.branch_id === b.id).map((c) => c.number);
  const paths = filesOf(p, b.id).slice(0, Math.max(1, Math.min(b.ahead, 12))).map((f) => ({ path: f.path, size: f.size, op: 'edit' as const }));
  const ch = publishChange(p, parent, u, String(body?.message || `Merge ${b.name} into ${parent.name}`), paths, 'merge', from);
  ch.file_count = b.ahead;
  b.ahead = 0;
  return { change: ch };
});
on('POST', '/api/projects/:p/branches/:b/archive', ({ headers, m }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  const b = getBranch(p, m[2]);
  if (!b.parent_id) throw new Fail(422, 'invalid', 'Main cannot be archived.');
  b.archived = true;
  emit({ kind: 'branch.archived', org_id: p.org_id, project_id: p.id, actor: userRef(u), data: { branch: b.name } });
  return b;
});

// files and changes
on('GET', '/api/projects/:p/branches/:b/files', async ({ headers, m, q }) => {
  const p = getProject(who(headers), m[1]);
  await wait(120);
  return fileEntries(p, getBranch(p, m[2]), q);
});
on('GET', '/api/projects/:p/changes', ({ headers, m, q }) => {
  const p = getProject(who(headers), m[1]);
  const branch = q.get('branch');
  const before = Number(q.get('before') || 0);
  const limit = Number(q.get('limit') || 50);
  let list = [...p.changes].sort((a, b) => b.number - a.number);
  if (branch) {
    const b = getBranch(p, branch);
    list = list.filter((c) => c.branch_id === b.id);
  }
  if (before) list = list.filter((c) => c.number < before);
  return list.slice(0, limit).map(({ files: _f, ...c }) => (void _f, c));
});
on('GET', '/api/projects/:p/changes/:n', ({ headers, m }) => {
  const p = getProject(who(headers), m[1]);
  const c = p.changes.find((x) => x.number === Number(m[2]));
  if (!c) throw new Fail(404, 'not_found', `There is no change ${m[2]} in ${p.name}.`);
  return c;
});
on('GET', '/api/projects/:p/history', ({ headers, m, q }) => {
  const p = getProject(who(headers), m[1]);
  const b = getBranch(p, q.get('branch'));
  const prefix = q.get('prefix') || '';
  const limit = Number(q.get('limit') || 10);
  const files = filesOf(p, b.id).filter((f) => !prefix || f.path.startsWith(prefix.endsWith('/') ? prefix : prefix + '/'));
  const nums = [...new Set(files.flatMap((f) => f.changes))].sort((a, b2) => b2 - a).slice(0, limit).sort((a, b2) => a - b2);
  return {
    changes: nums,
    files: files
      .map((f) => ({ path: f.path, changes: f.changes.filter((n) => nums.includes(n)) }))
      .filter((f) => f.changes.length || prefix)
      .sort((a, b2) => (b2.changes[b2.changes.length - 1] || 0) - (a.changes[a.changes.length - 1] || 0)),
  };
});
on('GET', '/api/projects/:p/download', ({ headers, m, q }) => {
  getProject(who(headers), m[1]);
  const at = q.get('change');
  return new Response(new Blob([`Mock contents of ${q.get('path')}${at ? ` as of change ${at}` : ''}\n`], { type: 'application/octet-stream' }), { status: 200 });
});
on('GET', '/api/projects/:p/thumbs/:hash', ({ headers, m, q }) => {
  getProject(who(headers), m[1]);
  // A little picture coloured from the hash, so lists show what real thumbnails look like.
  const h = parseInt(m[2].slice(0, 6), 16) % 360;
  const big = q.get('size') === 'large' ? 640 : 128;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${big}" height="${big}" viewBox="0 0 100 100"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${h},60%,62%)"/><stop offset="1" stop-color="hsl(${(h + 50) % 360},55%,28%)"/></linearGradient></defs><rect width="100" height="100" fill="url(#g)"/><circle cx="34" cy="38" r="14" fill="hsl(${(h + 30) % 360},90%,85%)" opacity=".7"/><path d="M0 80 L35 52 L60 72 L78 58 L100 76 L100 100 L0 100Z" fill="hsl(${(h + 80) % 360},40%,20%)" opacity=".8"/></svg>`;
  return new Response(new Blob([svg], { type: 'image/svg+xml' }), { status: 200 });
});
on('GET', '/api/projects/:p/versions', ({ headers, m, q }) => {
  const p = getProject(who(headers), m[1]);
  const b = getBranch(p, q.get('branch'));
  const path = q.get('path') || '';
  const f = filesOf(p, b.id).find((x) => x.path === path);
  if (!f) return { path, versions: [] };
  const nums = [...f.changes].sort((a, b2) => b2 - a);
  return {
    path,
    versions: nums.map((n, i) => {
      const c = p.changes.find((x) => x.number === n);
      const touched = c?.files.find((x) => x.path === path);
      return {
        change: n, op: i === nums.length - 1 ? 'add' : 'edit', size: touched?.size ?? f.size, file_hash: i === 0 ? f.file_hash : hash(), manifest_hash: hash(),
        message: c?.message || 'Earlier work', created_at: c?.created_at || new Date(Date.now() - (n % 50) * 86_400_000).toISOString(), kind: c?.kind || 'publish',
        branch_id: b.id, branch_name: b.name, author: c?.author || null, current: i === 0,
      };
    }),
  };
});
on('POST', '/api/projects/:p/branches/:b/restore-version', ({ headers, m, body }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  const b = getBranch(p, m[2]);
  const f = filesOf(p, b.id).find((x) => x.path === body?.path);
  if (!f || !f.changes.includes(Number(body?.change))) throw new Fail(404, 'not_found', `There is no version of that file in change ${body?.change}.`);
  if (f.changes[f.changes.length - 1] === Number(body.change)) throw new Fail(422, 'invalid', `The version from change ${body.change} is already the latest on ${b.name}.`);
  const holder = claims.find((c) => c.path === f.path && c.user.id !== u.id);
  if (holder) throw new Fail(409, 'conflict', `${f.path} is claimed by ${holder.user.name}. Ask them to release it first.`);
  const name = f.path.split('/').pop();
  const change = publishChange(p, b, u, body.message || `Brought back ${name} as of change ${body.change}`, [{ path: f.path, size: f.size, op: 'edit' }]);
  return { change };
});

// claims
on('GET', '/api/projects/:p/claims', ({ headers, m }) => claims.filter((c) => c.project_id === getProject(who(headers), m[1]).id).map(stripProject));
function stripProject(c: Claim & { project_id?: string }): Claim {
  const { project_id: _p, ...rest } = c;
  void _p;
  return rest;
}
on('POST', '/api/projects/:p/claims', ({ headers, m, body }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  const b = getBranch(p, body?.branch);
  const held: Claim[] = [];
  const claimed: string[] = [];
  const existing = claimsFor(p, b);
  for (const path of (body?.paths || []) as string[]) {
    const c = existing.find((x) => x.path === path);
    if (c && c.user.id !== u.id) held.push(stripProject(c));
    else if (!c) {
      claims.push({ project_id: p.id, path, branch_id: b.id, user: userRef(u), device: body?.device, created_at: new Date().toISOString() });
      claimed.push(path);
    } else claimed.push(path);
  }
  if (claimed.length) emit({ kind: 'claim.added', org_id: p.org_id, project_id: p.id, actor: userRef(u), data: { paths: claimed, branch: b.id } });
  return { claimed, held };
});
on('POST', '/api/projects/:p/claims/release', ({ headers, m, body }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  const b = getBranch(p, body?.branch);
  const released: string[] = [];
  for (const path of (body?.paths || []) as string[]) {
    const c = claimsFor(p, b).find((x) => x.path === path);
    if (!c) continue;
    if (c.user.id !== u.id && !canAdminOrg(u, p.org_id)) throw new Fail(403, 'forbidden', `${c.user.name} holds ${path}.`);
    claims.splice(claims.indexOf(c), 1);
    released.push(path);
  }
  if (released.length) emit({ kind: 'claim.released', org_id: p.org_id, project_id: p.id, actor: userRef(u), data: { paths: released, branch: b.id } });
  return { released };
});
on('POST', '/api/projects/:p/claims/nudge', ({ headers, m, body }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  const c = claims.find((x) => x.project_id === p.id && x.path === body?.path);
  if (!c) throw new Fail(404, 'not_found', 'Nobody holds that file any more.');
  emit({ kind: 'claim.nudged', org_id: p.org_id, project_id: p.id, actor: userRef(u), data: { path: c.path, holder: c.user.id } });
  return {};
});

// tags
on('GET', '/api/projects/:p/tags', ({ headers, m }) => tagsFor(getProject(who(headers), m[1])));
on('POST', '/api/projects/:p/tags', ({ headers, m, body }) => {
  const p = getProject(who(headers), m[1]);
  const name = String(body?.name || '').trim();
  if (!name) throw new Fail(422, 'invalid', 'Give the tag a name.');
  if (tagsFor(p).some((t) => t.name.toLowerCase() === name.toLowerCase())) throw new Fail(409, 'conflict', `There is already a tag called ${name}.`);
  const t = { id: uid('tag00009'), name, color: body?.color || '#1b1324', group: body?.group, org_id: p.org_id, project_id: body?.project_only ? p.id : undefined };
  tags.push(t);
  return t;
});
on('POST', '/api/projects/:p/tags/apply', ({ headers, m, body }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  const t = tagsFor(p).find((x) => x.id === body?.tag || x.name === body?.tag);
  if (!t) throw new Fail(404, 'not_found', 'That tag does not exist.');
  for (const path of body?.paths || []) {
    if (t.group) {
      // exclusive within a group
      for (const other of tagsFor(p).filter((x) => x.group === t.group && x.id !== t.id)) {
        const i = pathTags.findIndex((pt) => pt.project_id === p.id && pt.path === path && pt.tag_id === other.id);
        if (i >= 0) pathTags.splice(i, 1);
      }
    }
    if (!pathTags.some((pt) => pt.project_id === p.id && pt.path === path && pt.tag_id === t.id)) pathTags.push({ project_id: p.id, path, tag_id: t.id });
  }
  emit({ kind: 'tag.applied', org_id: p.org_id, project_id: p.id, actor: userRef(u), data: { tag: t.name, paths: body?.paths } });
  return {};
});
on('POST', '/api/projects/:p/tags/remove', ({ headers, m, body }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  for (const path of body?.paths || []) {
    const i = pathTags.findIndex((pt) => pt.project_id === p.id && pt.path === path && pt.tag_id === body?.tag);
    if (i >= 0) pathTags.splice(i, 1);
  }
  emit({ kind: 'tag.removed', org_id: p.org_id, project_id: p.id, actor: userRef(u), data: { tag: body?.tag, paths: body?.paths } });
  return {};
});

// drafts
on('GET', '/api/projects/:p/drafts', ({ headers, m }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  return drafts.filter((d) => d.project_id === p.id && d.user_id === u.id).map(({ entries: _e, project_id: _p, user_id: _u, ...d }) => (void _e, void _p, void _u, d));
});
on('GET', '/api/projects/:p/drafts/:id', ({ headers, m }) => {
  const u = who(headers);
  const p = getProject(u, m[1]);
  const d = drafts.find((x) => x.id === m[2] && x.project_id === p.id && x.user_id === u.id);
  if (!d) throw new Fail(404, 'not_found', 'That draft does not exist any more.');
  return d;
});
on('DELETE', '/api/projects/:p/drafts/:id', ({ headers, m }) => {
  const u = who(headers);
  const i = drafts.findIndex((x) => x.id === m[2] && x.user_id === u.id);
  if (i < 0) throw new Fail(404, 'not_found', 'That draft does not exist any more.');
  drafts.splice(i, 1);
  return {};
});

// ---------- admin ----------

function adminUser(u: MUser): AdminUser {
  return {
    id: u.id, email: u.email, name: u.name, status: u.status, is_server_admin: u.is_server_admin, created_at: u.created_at,
    last_seen_at: u.last_seen_at, suspended_reason: u.status === 'suspended' ? u.suspended_reason : undefined, orgs: orgsOf(u),
    claims: claims.filter((c) => c.user.id === u.id).length, sessions: (sessions.get(u.id) || []).length,
  };
}
function targetUser(actor: MUser, id: string): MUser {
  const t = userById(id);
  if (!t) throw new Fail(404, 'not_found', 'That person does not have an account here.');
  const shared = actor.is_server_admin || orgsOf(t).some((o) => canAdminOrg(actor, o.org.id));
  if (!shared) throw new Fail(403, 'forbidden', 'This person is not in an organization you administer.');
  return t;
}

on('GET', '/api/admin/overview', ({ headers, q }) => {
  const u = who(headers);
  const org = q.get('org');
  needAdmin(u, org);
  const ids = adminOrgIds(u, org);
  const ps = projects.filter((p) => ids.includes(p.org_id));
  const us = users.filter((x) => members.some((mm) => mm.user_id === x.id && ids.includes(mm.org_id)));
  const cl = claims.filter((c) => ps.some((p) => p.id === c.project_id));
  // As on the server: storage is only counted for the whole server.
  const all = u.is_server_admin && !org;
  return {
    users: us.length, orgs: ids.length, projects: ps.filter((p) => !p.archived).length,
    storage_bytes: all ? ps.reduce((a, p) => a + filesOf(p, p.branches[0].id).reduce((b, f) => b + f.size * 1.6, 0), 0) : null,
    chunks: all ? ps.reduce((a, p) => a + filesOf(p, p.branches[0].id).length * 37, 0) : null,
    active_claims: cl.length, stale_claims: cl.filter((c) => Date.now() - Date.parse(c.created_at) > 3 * DAY).length,
    suspended: us.filter((x) => x.status === 'suspended').length,
  };
});
// --- clients and productions ---
function place(p: MProject, client: string | null, production: string | null) {
  const pr = production ? productions.find((x) => x.id === production) : undefined;
  if (production && (!pr || pr.org_id !== p.org_id)) throw new Fail(404, 'not_found', 'That production does not exist.');
  if (!pr && client && !clients.some((c) => c.id === client && c.org_id === p.org_id)) throw new Fail(404, 'not_found', 'That client does not exist.');
  for (const x of productions) if (x.main_project_id === p.id && x.id !== pr?.id) x.main_project_id = null;
  p.production_id = pr ? pr.id : null;
  p.client_id = pr ? pr.client_id : client;
}
const cleanName = (v: unknown, what: string) => {
  const n = String(v || '').split(/\s+/).filter(Boolean).join(' ');
  if (!n) throw new Fail(422, 'invalid', `Give the ${what} a name.`);
  return n;
};
on('GET', '/api/admin/orgs/:org/structure', ({ headers, m }) => {
  needAdmin(who(headers), m[1]);
  const live = projects.filter((p) => !p.archived);
  return {
    clients: clients.filter((c) => c.org_id === m[1]).sort((a, b) => a.name.localeCompare(b.name)).map((c) => ({
      ...c, productions: productions.filter((x) => x.client_id === c.id).length, projects: live.filter((p) => p.client_id === c.id).length,
    })),
    productions: productions.filter((x) => x.org_id === m[1]).sort((a, b) => a.name.localeCompare(b.name)).map((x) => ({
      ...x, projects: live.filter((p) => p.production_id === x.id).length,
    })),
  };
});
on('POST', '/api/admin/orgs/:org/clients', ({ headers, m, body }) => {
  needAdmin(who(headers), m[1]);
  const name = cleanName(body?.name, 'client');
  if (clients.some((c) => c.org_id === m[1] && c.name.toLowerCase() === name.toLowerCase())) throw new Fail(409, 'conflict', `There is already a client called ${name}.`);
  const c = { id: uid('cl000009'), org_id: m[1], name, created_at: new Date().toISOString() };
  clients.push(c);
  return { id: c.id, name };
});
on('PATCH', '/api/admin/clients/:id', ({ headers, m, body }) => {
  const c = clients.find((x) => x.id === m[1]);
  if (!c) throw new Fail(404, 'not_found', 'That client does not exist.');
  needAdmin(who(headers), c.org_id);
  c.name = cleanName(body?.name, 'client');
  return { id: c.id, name: c.name };
});
on('DELETE', '/api/admin/clients/:id', ({ headers, m }) => {
  const c = clients.find((x) => x.id === m[1]);
  if (!c) throw new Fail(404, 'not_found', 'That client does not exist.');
  needAdmin(who(headers), c.org_id);
  clients.splice(clients.indexOf(c), 1);
  for (const x of productions) if (x.client_id === c.id) x.client_id = null;
  for (const p of projects) if (p.client_id === c.id) p.client_id = null;
  return { ok: true };
});
on('POST', '/api/admin/orgs/:org/productions', ({ headers, m, body }) => {
  needAdmin(who(headers), m[1]);
  const name = cleanName(body?.name, 'production');
  const client = body?.client || null;
  if (productions.some((x) => x.org_id === m[1] && x.client_id === client && x.name.toLowerCase() === name.toLowerCase())) throw new Fail(409, 'conflict', `There is already a production called ${name} there.`);
  const x = { id: uid('pd000009'), org_id: m[1], client_id: client, name, main_project_id: null, created_at: new Date().toISOString() };
  productions.push(x);
  return { id: x.id, name, client_id: client };
});
on('PATCH', '/api/admin/productions/:id', ({ headers, m, body }) => {
  const x = productions.find((y) => y.id === m[1]);
  if (!x) throw new Fail(404, 'not_found', 'That production does not exist.');
  needAdmin(who(headers), x.org_id);
  if (body?.name !== undefined) x.name = cleanName(body.name, 'production');
  if (body && 'client' in body) {
    x.client_id = body.client || null;
    for (const p of projects) if (p.production_id === x.id) p.client_id = x.client_id;
  }
  if (body && 'main_project' in body) {
    if (body.main_project && !projects.some((p) => p.id === body.main_project && p.production_id === x.id)) throw new Fail(422, 'invalid', "The main project has to be one of this production's projects.");
    x.main_project_id = body.main_project || null;
  }
  return { ok: true };
});
on('DELETE', '/api/admin/productions/:id', ({ headers, m }) => {
  const x = productions.find((y) => y.id === m[1]);
  if (!x) throw new Fail(404, 'not_found', 'That production does not exist.');
  needAdmin(who(headers), x.org_id);
  productions.splice(productions.indexOf(x), 1);
  for (const p of projects) if (p.production_id === x.id) p.production_id = null;
  return { ok: true };
});
on('GET', '/api/admin/users', ({ headers, q }) => {
  const u = who(headers);
  const org = q.get('org');
  needAdmin(u, org || (u.is_server_admin ? null : adminOrgIds(u)[0]));
  const ids = adminOrgIds(u, org);
  const text = (q.get('q') || '').toLowerCase();
  const status = q.get('status');
  return users
    .filter((x) => (u.is_server_admin && !org) || members.some((mm) => mm.user_id === x.id && ids.includes(mm.org_id)))
    .filter((x) => !text || x.name.toLowerCase().includes(text) || x.email.toLowerCase().includes(text))
    .filter((x) => !status || x.status === status)
    .map(adminUser);
});
on('GET', '/api/admin/users/:id', ({ headers, m }) => {
  const t = targetUser(who(headers), m[1]);
  return {
    ...adminUser(t), sessions: sessions.get(t.id) || [],
    claims: claims.filter((c) => c.user.id === t.id).map((c) => ({ ...stripProject(c), project: { id: c.project_id, name: projectById(c.project_id)!.name } })),
  };
});
on('POST', '/api/admin/users/:id/suspend', ({ headers, m, body }) => {
  const u = who(headers);
  const t = targetUser(u, m[1]);
  if (t.id === u.id) throw new Fail(422, 'invalid', 'You cannot suspend your own account.');
  t.status = 'suspended';
  t.suspended_reason = body?.reason || undefined;
  sessions.set(t.id, []);
  emit({ kind: 'member.suspended', actor: userRef(u), data: { user: t.name, reason: body?.reason } });
  return adminUser(t);
});
on('POST', '/api/admin/users/:id/reactivate', ({ headers, m }) => {
  const u = who(headers);
  const t = targetUser(u, m[1]);
  t.status = 'active';
  t.suspended_reason = undefined;
  emit({ kind: 'member.reactivated', actor: userRef(u), data: { user: t.name } });
  return adminUser(t);
});
on('POST', '/api/admin/users/:id/sign-out', ({ headers, m }) => {
  const u = who(headers);
  const t = targetUser(u, m[1]);
  const n = (sessions.get(t.id) || []).length;
  sessions.set(t.id, []);
  emit({ kind: 'admin.user.signed_out', actor: userRef(u), data: { user: t.name, revoked: n } });
  return { revoked: n };
});
on('DELETE', '/api/admin/users/:id', ({ headers, m, body }) => {
  const u = who(headers);
  if (!u.is_server_admin) throw new Fail(403, 'forbidden', 'Only server admins can delete accounts.');
  const t = targetUser(u, m[1]);
  if (body?.confirm !== t.name) throw new Fail(422, 'invalid', 'The name you typed does not match.');
  if (t.id === u.id) throw new Fail(422, 'invalid', 'You cannot delete your own account.');
  const to = body?.transfer_to ? userById(body.transfer_to) : undefined;
  for (const c of claims.filter((x) => x.user.id === t.id)) {
    if (to) c.user = userRef(to);
    else claims.splice(claims.indexOf(c), 1);
  }
  for (const d of drafts.filter((x) => x.user_id === t.id)) {
    if (to) d.user_id = to.id;
    else drafts.splice(drafts.indexOf(d), 1);
  }
  users.splice(users.indexOf(t), 1);
  for (let i = members.length - 1; i >= 0; i--) if (members[i].user_id === t.id) members.splice(i, 1);
  emit({ kind: 'admin.user.deleted', actor: userRef(u), data: { user: t.name, transfer_to: to?.name } });
  return {};
});
on('POST', '/api/admin/users/:id/password', ({ headers, m, body }) => {
  const u = who(headers);
  const t = targetUser(u, m[1]);
  if (String(body?.password || '').length < 8) throw new Fail(422, 'invalid', 'The password needs at least 8 characters.');
  sessions.set(t.id, []);
  emit({ kind: 'admin.user.password_reset', actor: userRef(u), data: { user: t.name } });
  return {};
});
function orgFor(u: MUser, id: string) {
  const o = orgs.find((x) => x.id === id || x.slug === id);
  if (!o) throw new Fail(404, 'not_found', 'That organization does not exist.');
  needAdmin(u, o.id);
  return o;
}
on('GET', '/api/admin/orgs/:org/members', ({ headers, m }) => {
  const o = orgFor(who(headers), m[1]);
  return members.filter((x) => x.org_id === o.id).map((x): Member => {
    const t = userById(x.user_id)!;
    return { user: { ...userRef(t), status: t.status, last_seen_at: t.last_seen_at }, role: x.role, expires_at: x.expires_at, created_at: x.created_at };
  });
});
on('PATCH', '/api/admin/orgs/:org/members/:user', ({ headers, m, body }) => {
  const u = who(headers);
  const o = orgFor(u, m[1]);
  const mm = members.find((x) => x.org_id === o.id && x.user_id === m[2]);
  if (!mm) throw new Fail(404, 'not_found', 'That person is not a member of this organization.');
  if (mm.role === 'owner' && body?.role && body.role !== 'owner' && members.filter((x) => x.org_id === o.id && x.role === 'owner').length === 1)
    throw new Fail(409, 'conflict', 'An organization needs at least one owner. Make someone else owner first.');
  if (body?.role) mm.role = body.role;
  if (body && 'expires_at' in body) mm.expires_at = body.expires_at || undefined;
  const t = userById(mm.user_id)!;
  emit({ kind: 'admin.member.role_changed', org_id: o.id, actor: userRef(u), data: { user: t.name, role: mm.role } });
  return { user: { ...userRef(t), status: t.status, last_seen_at: t.last_seen_at }, role: mm.role, expires_at: mm.expires_at, created_at: mm.created_at };
});
on('DELETE', '/api/admin/orgs/:org/members/:user', ({ headers, m }) => {
  const u = who(headers);
  const o = orgFor(u, m[1]);
  const i = members.findIndex((x) => x.org_id === o.id && x.user_id === m[2]);
  if (i < 0) throw new Fail(404, 'not_found', 'That person is not a member of this organization.');
  const t = userById(m[2])!;
  members.splice(i, 1);
  emit({ kind: 'member.removed', org_id: o.id, actor: userRef(u), data: { user: t.name } });
  return {};
});
on('GET', '/api/admin/orgs/:org/invites', ({ headers, m }) => {
  const o = orgFor(who(headers), m[1]);
  return invites.filter((i) => i.org_id === o.id).map(({ org_id: _o, token: _t, ...i }) => (void _o, void _t, i));
});
on('POST', '/api/admin/orgs/:org/invites', ({ headers, m, body }) => {
  const u = who(headers);
  const o = orgFor(u, m[1]);
  const emails: Array<string | undefined> = body?.emails?.length ? body.emails : [undefined];
  const days = Number(body?.expires_in_days || 0);
  const out = emails.map((email) => {
    const token = Math.random().toString(36).slice(2, 10);
    const inv = {
      id: uid('inv00009'), org_id: o.id, token, email, role: body?.role || 'member',
      expires_at: days ? new Date(Date.now() + days * DAY).toISOString() : undefined, created_at: new Date().toISOString(),
      created_by: userRef(u), url: `${location.origin}/invite/${token}`,
    };
    invites.unshift(inv);
    const { org_id: _o, token: _t, ...rest } = inv;
    void _o;
    void _t;
    return rest;
  });
  emit({ kind: 'admin.invite.created', org_id: o.id, actor: userRef(u), data: { count: out.length, role: body?.role } });
  return out;
});
on('DELETE', '/api/admin/invites/:id', ({ headers, m }) => {
  const u = who(headers);
  const i = invites.findIndex((x) => x.id === m[1]);
  if (i < 0) throw new Fail(404, 'not_found', 'That invite does not exist any more.');
  needAdmin(u, invites[i].org_id);
  invites.splice(i, 1);
  return {};
});
on('GET', '/api/admin/projects', ({ headers, q }) => {
  const u = who(headers);
  const org = q.get('org');
  const ids = adminOrgIds(u, org);
  return projects.filter((p) => ids.includes(p.org_id)).map((p) => summary(p, u));
});
on('PATCH', '/api/admin/projects/:p', ({ headers, m, body }) => {
  const u = who(headers);
  const p = projectById(m[1]);
  if (!p) throw new Fail(404, 'not_found', 'That project does not exist.');
  needAdmin(u, p.org_id);
  if (body?.name) p.name = String(body.name).trim();
  if (body?.org) {
    const o = orgs.find((x) => x.id === body.org);
    if (!o) throw new Fail(404, 'not_found', 'That organization does not exist.');
    if (!canAdminOrg(u, o.id)) throw new Fail(403, 'forbidden', 'You need to be an admin of the organization you move it to.');
    p.org_id = o.id;
  }
  if (body?.org) place(p, null, null);
  if (body && ('client' in body || 'production' in body)) place(p, 'client' in body ? body.client : p.client_id || null, body.production || null);
  if (typeof body?.auto_checkpoints === 'boolean') p.auto_checkpoints = body.auto_checkpoints;
  for (const k of ['theme_light', 'theme_dark'] as const) if (typeof body?.[k] === 'string') (p as unknown as Record<string, string | null>)[k] = body[k] || null;
  if (body?.checkpoints) {
    if (!['auto', 'server', 'local'].includes(body.checkpoints)) throw new Fail(422, 'invalid', 'Choose where this project keeps checkpoints.');
    p.checkpoints = body.checkpoints;
  }
  emit({ kind: 'admin.project.updated', org_id: p.org_id, project_id: p.id, actor: userRef(u), data: { name: body?.name, org: body?.org, checkpoints: body?.checkpoints } });
  return fullProject(p, u);
});
for (const action of ['archive', 'restore'] as const) {
  on('POST', `/api/admin/projects/:p/${action}`, ({ headers, m }) => {
    const u = who(headers);
    const p = projectById(m[1]);
    if (!p) throw new Fail(404, 'not_found', 'That project does not exist.');
    needAdmin(u, p.org_id);
    p.archived = action === 'archive';
    emit({ kind: action === 'archive' ? 'project.archived' : 'project.restored', org_id: p.org_id, project_id: p.id, actor: userRef(u), data: { project: p.name } });
    return fullProject(p, u);
  });
}
on('GET', '/api/admin/claims', ({ headers, q }) => {
  const u = who(headers);
  const ids = adminOrgIds(u, q.get('org'));
  const hours = Number(q.get('older_than_hours') || 0);
  return claims
    .filter((c) => ids.includes(projectById(c.project_id)!.org_id))
    .filter((c) => !hours || Date.now() - Date.parse(c.created_at) > hours * 3600_000 || userById(c.user.id)?.status === 'suspended')
    .sort((a, b) => Date.parse(a.created_at) - Date.parse(b.created_at))
    .map((c) => ({ ...stripProject(c), project: { id: c.project_id, name: projectById(c.project_id)!.name } }));
});
on('POST', '/api/admin/claims/release', ({ headers, body }) => {
  const u = who(headers);
  let released = 0;
  for (const r of (body?.claims || []) as Array<{ project: string; branch: string; path: string }>) {
    const i = claims.findIndex((c) => c.project_id === r.project && c.path === r.path);
    if (i >= 0) {
      const c = claims[i];
      needAdmin(u, projectById(c.project_id)!.org_id);
      claims.splice(i, 1);
      released += 1;
      emit({ kind: 'claim.released', org_id: projectById(c.project_id)!.org_id, project_id: c.project_id, actor: userRef(u), data: { paths: [c.path], holder: c.user.name, by_admin: true } });
    }
  }
  return { released };
});
on('POST', '/api/admin/storage/check', async ({ headers, body }) => {
  const u = who(headers);
  needAdmin(u, body?.project ? projectById(body.project)?.org_id : CINDER.id);
  await wait(1200);
  const ps = body?.project ? projects.filter((p) => p.id === body.project) : projects;
  const checked = ps.reduce((a, p) => a + filesOf(p, p.branches[0].id).length * 37, 0);
  const broken = !body?.project || body.project === TIDE_ID;
  emit({ kind: 'admin.storage.check', actor: userRef(u), data: { checked, ok: !broken } });
  return broken
    ? { checked, ok: false, missing: [{ hash: '9f2c4e1a7b3d5f60c8e2a4b6d8f0a1c3', changes: [1240, 1288] }] }
    : { checked, ok: true, missing: [] };
});
const TIDE_ID = 'pr000002-0000-4000-8000-000000000002';
let gcBytes = Math.round(3.7 * 1024 * MB);
on('GET', '/api/admin/storage/gc', async ({ headers }) => {
  needAdmin(who(headers), CINDER.id);
  await wait(500);
  advanceMoves();
  const by_location = locations
    .map((l) => {
      const left = leftovers.get(l.name) || { chunks: 0, bytes: 0 };
      return { location: l.name, chunks: left.chunks + (l.main && gcBytes ? 2318 : 0), bytes: left.bytes + (l.main ? gcBytes : 0) };
    })
    .filter((x) => x.bytes > 0);
  return {
    unreferenced_chunks: by_location.reduce((a, x) => a + x.chunks, 0),
    bytes: by_location.reduce((a, x) => a + x.bytes, 0),
    by_location,
  };
});
on('POST', '/api/admin/storage/gc', async ({ headers }) => {
  const u = who(headers);
  if (!u.is_server_admin) throw new Fail(403, 'forbidden', 'Only server admins can reclaim space.');
  await wait(1500);
  advanceMoves();
  let deleted = gcBytes ? 2318 : 0;
  let bytes = gcBytes;
  for (const left of leftovers.values()) {
    deleted += left.chunks;
    bytes += left.bytes;
  }
  leftovers.clear();
  const out = { deleted, bytes };
  gcBytes = 0;
  emit({ kind: 'admin.storage.gc', actor: userRef(u), data: out });
  return out;
});
// Backups: a few nights of history; "Back up now" finishes after a few seconds.
let backupStartedAt: number | null = null;
const backupNights = [1, 2, 3, 4, 5].map((d) => {
  const t = new Date(Date.now() - d * 86400000);
  t.setUTCHours(3, 0, 12, 0);
  return { id: t.toISOString().slice(0, 19).replace(/[-:]/g, '').replace('T', '-'), created_at: t.toISOString(), server_version: '0.1.0', tables: 23, rows: 18412, chunks: 9120 - d * 40, db_bytes: 2_400_000 - d * 20_000, new_chunks: 40, new_chunk_bytes: 180_000_000 };
});
on('GET', '/api/admin/backups', ({ headers }) => {
  const u = who(headers);
  if (!u.is_server_admin) throw new Fail(403, 'forbidden', 'Only server admins can do that.');
  const running = backupStartedAt !== null && Date.now() - backupStartedAt < 5000;
  if (backupStartedAt !== null && !running) {
    const t = new Date(backupStartedAt);
    backupNights.unshift({ ...backupNights[0], id: t.toISOString().slice(0, 19).replace(/[-:]/g, '').replace('T', '-'), created_at: t.toISOString(), new_chunks: 6, new_chunk_bytes: 31_000_000 });
    backupStartedAt = null;
  }
  return {
    configured: true, target: 'mounted path /data/uaivca-drive/MultiParity-backups', target_error: null, schedule: '03:00 UTC',
    next_run: new Date(Date.now() + 6 * 3600000).toISOString(), keep_daily: 7, keep_weekly: 4, running,
    runs: running ? [{ id: 'run-now', started_at: new Date(backupStartedAt!).toISOString(), finished_at: null, status: 'running', error: null, snapshot: null, db_bytes: null, new_chunks: null, new_chunk_bytes: null, total_chunks: null }] :
      backupNights.map((n) => ({ id: n.id, started_at: n.created_at, finished_at: n.created_at, status: 'done', error: null, snapshot: n.id, db_bytes: n.db_bytes, new_chunks: n.new_chunks, new_chunk_bytes: n.new_chunk_bytes, total_chunks: n.chunks })),
    snapshots: backupNights.slice(0, 8),
  };
});
on('POST', '/api/admin/backups/run', ({ headers }) => {
  const u = who(headers);
  if (!u.is_server_admin) throw new Fail(403, 'forbidden', 'Only server admins can do that.');
  if (backupStartedAt !== null) throw new Fail(409, 'conflict', 'A backup is already running.');
  backupStartedAt = Date.now();
  return { started: true };
});
on('GET', '/api/admin/storage', ({ headers }) => {
  const u = who(headers);
  needAdmin(u, u.is_server_admin ? null : adminOrgIds(u)[0]);
  const ids = adminOrgIds(u);
  const mine = (pid: string) => u.is_server_admin || ids.includes(projectById(pid)?.org_id || '');
  return {
    locations: locationsOut(u).map((l) => ({ ...l, projects: l.projects.filter((p) => mine(p.id)) })),
    unavailable: [],
    moves: advanceMoves().filter((mv) => mine(mv.project_id)).slice(0, 20).map(moveOut),
  };
});
on('GET', '/api/admin/projects/:p/storage', ({ headers, m }) => {
  const u = who(headers);
  const p = projectById(m[1]);
  if (!p) throw new Fail(404, 'not_found', 'That project does not exist.');
  needAdmin(u, p.org_id);
  advanceMoves();
  const mv = latestMove(p.id);
  return { storage: p.storage, previous: p.storage_previous, move: mv ? moveOut(mv) : null };
});
on('POST', '/api/admin/projects/:p/storage', ({ headers, m, body }) => {
  const u = who(headers);
  const p = projectById(m[1]);
  if (!p) throw new Fail(404, 'not_found', 'That project does not exist.');
  if (!u.is_server_admin) throw new Fail(403, 'forbidden', 'Only server admins can move file data.');
  advanceMoves();
  const to = String(body?.to || '');
  if (!locations.some((l) => l.name === to)) throw new Fail(422, 'invalid', `The server has no storage location called ${to}.`);
  const last = latestMove(p.id);
  if (last?.status === 'running') throw new Fail(409, 'conflict', `${p.name} is already being moved to ${last.to}.`);
  const finishing = p.storage_previous !== null && p.storage === to;
  if (p.storage === to && !finishing) throw new Fail(422, 'invalid', `${p.name} is already stored in ${to}.`);
  const from = finishing ? p.storage_previous! : p.storage;
  const total_chunks = projectChunks(p);
  const total_bytes = projectStored(p);
  const resumed = finishing && last && last.to === to ? last : undefined;
  const mv: MMove = {
    id: uid('mov00009'), project_id: p.id, project_name: p.name, from, to, status: 'running', total_chunks,
    copied_chunks: resumed ? Math.min(total_chunks, resumed.copied_chunks) : 0, copied_bytes: resumed ? Math.min(total_bytes, resumed.copied_bytes) : 0,
    error: null, started_at: new Date().toISOString(), finished_at: null, ms: Date.now(), total_bytes,
  };
  storageMoves.unshift(mv);
  p.storage = to;
  p.storage_previous = from;
  emit({ kind: 'admin.project.storage_moved', org_id: p.org_id, project_id: p.id, actor: userRef(u), data: { project: p.name, from, to } });
  return moveOut(mv);
});
on('GET', '/api/admin/checkpoints', ({ headers, q }) => {
  const u = who(headers);
  const org = q.get('org');
  needAdmin(u, org || (u.is_server_admin ? null : adminOrgIds(u)[0]));
  const ids = adminOrgIds(u, org);
  const groups = new Map<string, CheckpointUsage>();
  for (const c of serverCheckpoints) {
    const p = projectById(c.project_id);
    const t = userById(c.user_id);
    if (!p || !t || !ids.includes(p.org_id)) continue;
    const k = `${c.user_id}:${c.project_id}`;
    let g = groups.get(k);
    if (!g) {
      const o = orgs.find((x) => x.id === p.org_id)!;
      g = { user: userRef(t), project: { id: p.id, name: p.name }, org: { id: o.id, name: o.name }, checkpoints: 0, automatic: 0, stored: 0, size: 0, oldest: c.created_at, newest: c.created_at };
      groups.set(k, g);
    }
    g.checkpoints += 1;
    if (c.auto) g.automatic += 1;
    g.stored += c.stored;
    g.size += c.size;
    if (c.created_at < g.oldest) g.oldest = c.created_at;
    if (c.created_at > g.newest) g.newest = c.created_at;
  }
  return [...groups.values()].sort((a, b) => b.stored - a.stored);
});
on('POST', '/api/admin/checkpoints/clear', async ({ headers, body }) => {
  const u = who(headers);
  const p = projectById(String(body?.project || ''));
  if (!p) throw new Fail(404, 'not_found', 'That project does not exist.');
  needAdmin(u, p.org_id);
  await wait(400);
  const days = Number(body?.older_than_days || 0);
  let removed = 0;
  for (let i = serverCheckpoints.length - 1; i >= 0; i--) {
    const c = serverCheckpoints[i];
    if (c.project_id !== p.id || c.user_id !== body?.user) continue;
    if (body?.only_automatic && !c.auto) continue;
    if (days && Date.now() - Date.parse(c.created_at) < days * DAY) continue;
    serverCheckpoints.splice(i, 1);
    removed += 1;
  }
  emit({ kind: 'admin.checkpoints.cleared', org_id: p.org_id, project_id: p.id, actor: userRef(u), data: { user: userById(body?.user)?.name, removed, only_automatic: !!body?.only_automatic } });
  return { removed };
});
on('GET', '/api/admin/audit', ({ headers, q }) => {
  const u = who(headers);
  const org = q.get('org');
  needAdmin(u, org || (u.is_server_admin ? null : adminOrgIds(u)[0]));
  const ids = adminOrgIds(u, org);
  const before = Number(q.get('before') || 0);
  const limit = Number(q.get('limit') || 50);
  return [...events]
    .reverse()
    .filter((e) => (u.is_server_admin && !org) || (e.org_id && ids.includes(e.org_id)))
    .filter((e) => !q.get('project') || e.project_id === q.get('project'))
    .filter((e) => !q.get('actor') || e.actor?.id === q.get('actor'))
    .filter((e) => !q.get('kind') || e.kind.startsWith(q.get('kind')!))
    .filter((e) => !before || e.id < before)
    .slice(0, limit);
});

// ---------- entry points ----------

export async function mockFetch(url: string, init: RequestInit = {}): Promise<Response> {
  const u = new URL(url, location.origin);
  const method = (init.method || 'GET').toUpperCase();
  const headers = new Headers(init.headers);
  let body: unknown;
  if (typeof init.body === 'string') {
    try {
      body = JSON.parse(init.body);
    } catch {
      body = undefined;
    }
  }
  await wait(60 + Math.random() * 140);
  for (const [m, re, h] of routes) {
    if (m !== method) continue;
    const match = u.pathname.match(re);
    if (!match) continue;
    try {
      const out = await h({ m: match, body, q: u.searchParams, headers });
      if (out instanceof Response) return out;
      return json(200, out ?? {});
    } catch (e) {
      if (e instanceof Fail) return json(e.status, { error: { code: e.code, message: e.message, details: e.details } });
      console.error('[mock]', e);
      return json(500, { error: { code: 'server', message: 'The mock server crashed.' } });
    }
  }
  return json(404, { error: { code: 'not_found', message: `No mock for ${method} ${u.pathname}.` } });
}

// ---------- mock daemon ----------

function daemonOn(): boolean {
  try {
    const q = new URLSearchParams(location.search).get('daemon');
    if (q === '0') sessionStorage.setItem('mp.mock.nodaemon', '1');
    if (q === '1') sessionStorage.removeItem('mp.mock.nodaemon');
    return sessionStorage.getItem('mp.mock.nodaemon') !== '1';
  } catch {
    return true;
  }
}

const dRoutes: Array<[string, RegExp, Handler]> = [];
const don = (method: string, pattern: string, h: Handler) =>
  dRoutes.push([method, new RegExp('^' + pattern.replace(/:(\w+)/g, '([^/]+)') + '$'), h]);
const getWs = (id: string) => {
  const w = workspaces.find((x) => x.id === id);
  if (!w) throw new Fail(404, 'not_found', 'This folder is not connected on this computer any more.');
  return w;
};

don('GET', '/local/health', () => ({ version: '0.1.0-mock', workspaces: workspaces.length }));
don('GET', '/local/workspaces', () => workspaces.map(wsOut));
don('GET', '/local/workspaces/:id/status', ({ m }) => getWs(m[1]).status);
don('GET', '/local/workspaces/:id/folders', async ({ m }) => {
  await wait(200);
  return folderTree(getWs(m[1]));
});
don('PUT', '/local/workspaces/:id/sync', async ({ m, body }) => {
  const w = getWs(m[1]);
  await wait(400);
  w.include = body?.include || [];
  w.exclude = body?.exclude || [];
  return wsOut(w);
});
don('POST', '/local/workspaces/:id/get-latest', async ({ m }) => {
  const w = getWs(m[1]);
  const p = projectById(w.project_id)!;
  await wait(900);
  const behind = filesOf(p, w.branch_id).filter((f) => f.changes[f.changes.length - 1] > w.synced_change && isIncluded(f.path, w.include, w.exclude));
  w.synced_change = p.head;
  w.status.behind = 0;
  return { downloaded: behind.length, bytes: behind.reduce((a, f) => a + f.size, 0) };
});
don('POST', '/local/workspaces/:id/versions', async ({ m, body }) => {
  const w = getWs(m[1]);
  const p = projectById(w.project_id)!;
  const f = filesOf(p, w.branch_id).find((x) => x.path === body?.path);
  if (!f) throw new Fail(404, 'not_found', 'That file has not been published.');
  await wait(600);
  if (body?.to === 'folder') {
    if (!w.status.edited.some((x) => x.path === f.path)) w.status.edited.push({ path: f.path, size: f.size } as (typeof w.status.edited)[number]);
    return { done: 'folder', undo: null };
  }
  const name = f.path.split('/').pop() || 'file';
  const dot = name.lastIndexOf('.');
  return { done: 'copy', saved_to: `${w.root}/${f.path.split('/').slice(0, -1).join('/')}/${dot > 0 ? `${name.slice(0, dot)} (change ${body?.change})${name.slice(dot)}` : name}` };
});
don('POST', '/local/workspaces/:id/publish', async ({ m, body }) => {
  const w = getWs(m[1]);
  const p = projectById(w.project_id)!;
  const b = getBranch(p, w.branch_id);
  const want = (body?.paths as string[] | undefined) || [...w.status.new, ...w.status.edited, ...w.status.deleted].map((f) => f.path);
  if (!String(body?.message || '').trim()) throw new Fail(422, 'invalid', 'Say what you changed before publishing.');
  const held = claimsFor(p, b).filter((c) => want.includes(c.path) && c.user.id !== ME.id);
  if (held.length) throw new Fail(409, 'conflict', `${held[0].user.name} holds ${held[0].path.split('/').pop()}.`, { stale: [], claimed: held });
  await wait(1200);
  const entries = [
    ...w.status.new.filter((f) => want.includes(f.path)).map((f) => ({ path: f.path, size: f.size, op: 'add' as const })),
    ...w.status.edited.filter((f) => want.includes(f.path)).map((f) => ({ path: f.path, size: f.size, op: 'edit' as const })),
    ...w.status.deleted.filter((f) => want.includes(f.path)).map((f) => ({ path: f.path, size: 0, op: 'delete' as const })),
  ];
  const ch = publishChange(p, b, ME, String(body.message), entries);
  w.status.new = w.status.new.filter((f) => !want.includes(f.path));
  w.status.edited = w.status.edited.filter((f) => !want.includes(f.path));
  w.status.deleted = w.status.deleted.filter((f) => !want.includes(f.path));
  w.status.checkpoints = w.status.checkpoints.filter((c) => c.auto && (w.status.new.length || w.status.edited.length));
  for (let i = claims.length - 1; i >= 0; i--) if (claims[i].project_id === p.id && claims[i].user.id === ME.id && want.includes(claims[i].path)) claims.splice(i, 1);
  if (w.synced_change === ch.number - 1) w.synced_change = ch.number;
  return { change: { number: ch.number } };
});
don('POST', '/local/workspaces/:id/checkpoint', ({ m, body }) => {
  const w = getWs(m[1]);
  const c = { id: uid('chk00009'), note: body?.note, auto: false, created_at: new Date().toISOString(), files: w.status.new.length + w.status.edited.length };
  w.status.checkpoints.unshift(c);
  return c;
});
don('POST', '/local/workspaces/:id/draft', async ({ m, body }) => {
  const w = getWs(m[1]);
  const p = projectById(w.project_id)!;
  const want = (body?.paths as string[] | undefined) || [...w.status.new, ...w.status.edited].map((f) => f.path);
  const files = [...w.status.new, ...w.status.edited].filter((f) => want.includes(f.path));
  await wait(700);
  const d = {
    id: uid('drf00009'), project_id: p.id, user_id: ME.id, title: String(body?.title || 'Untitled draft'), branch_id: w.branch_id,
    created_at: new Date().toISOString(), file_count: files.length, size: files.reduce((a, f) => a + f.size, 0),
    entries: files.map((f) => ({ path: f.path, op: 'put' as const, prev_file_hash: null, manifest: { hash: hash(), size: f.size, file_hash: hash(), chunks: [] as Array<[string, number]> } })),
  };
  drafts.unshift(d);
  emit({ kind: 'draft.saved', org_id: p.org_id, project_id: p.id, actor: userRef(ME), data: { draft: d.id } });
  return { id: d.id };
});
don('POST', '/local/workspaces/:id/no-checkpoint', ({ m, body }) => {
  const w = getWs(m[1]);
  return { file: `${w.root}/${body?.personal ? '.mp/nocheckpoint' : '.mpnocheckpoint'}` };
});
don('GET', '/local/workspaces/:id/ignore/suggest', ({ q }) => {
  const path = q.get('path') || '';
  const name = path.split('/').pop() || path;
  const dir = path.split('/').slice(0, -1).join('/');
  const ext = name.includes('.') ? name.split('.').pop() : '';
  return [
    { label: 'Only this file', pattern: path, matches: 1, bytes: 11 * MB, tracked: 0 },
    { label: 'Files named like this', pattern: name.replace(/\d+/g, '*'), matches: 37, bytes: 410 * MB, tracked: 0 },
    { label: 'This kind of file, in this folder', pattern: `${dir}/*.${ext}`, matches: 41, bytes: 452 * MB, tracked: 0 },
    { label: 'This kind of file, everywhere', pattern: `*.${ext}`, matches: 240, bytes: 2900 * MB, tracked: 0 },
    { label: 'Everything in this folder', pattern: `${dir}/`, matches: 54, bytes: 1900 * MB, tracked: 6 },
  ];
});

export async function mockDaemonFetch(url: string, init: RequestInit = {}): Promise<Response> {
  if (!daemonOn()) throw new TypeError('Failed to fetch');
  const u = new URL(url, 'http://127.0.0.1:18519');
  const method = (init.method || 'GET').toUpperCase();
  let body: unknown;
  if (typeof init.body === 'string') {
    try {
      body = JSON.parse(init.body);
    } catch {
      body = undefined;
    }
  }
  await wait(30 + Math.random() * 60);
  for (const [m, re, h] of dRoutes) {
    if (m !== method) continue;
    const match = u.pathname.match(re);
    if (!match) continue;
    try {
      return json(200, (await h({ m: match, body, q: u.searchParams, headers: new Headers() })) ?? {});
    } catch (e) {
      if (e instanceof Fail) return json(e.status, { error: { code: e.code, message: e.message, details: e.details } });
      console.error('[mock daemon]', e);
      return json(500, { error: { code: 'daemon', message: 'The mock daemon crashed.' } });
    }
  }
  return json(404, { error: { code: 'not_found', message: 'Not found' } });
}

// ---------- live events ----------

type Sub = { project: string; fn: (e: MpEvent) => void };
const subs = new Set<Sub>();
let scripted = false;

function runScript() {
  if (scripted) return;
  scripted = true;
  const P = projectById('emberfall')!;
  const main = P.branches[0];
  const T = users.find((x) => x.name.startsWith('Teo'))!;
  const M = users.find((x) => x.name.startsWith('Mara'))!;
  const S = users.find((x) => x.name.startsWith('Sana'))!;
  setTimeout(() => {
    const i = claims.findIndex((c) => c.project_id === P.id && c.path === 'Environments/Forge/forge_door_rust.png');
    if (i >= 0) {
      claims.splice(i, 1);
      emit({ kind: 'claim.released', org_id: P.org_id, project_id: P.id, actor: userRef(T), data: { paths: ['Environments/Forge/forge_door_rust.png'], branch: main.id } });
    }
  }, 30_000);
  setTimeout(() => {
    const f = filesOf(P, main.id).find((x) => x.path === 'Characters/Hero/hero_cape_sim.abc')!;
    publishChange(P, main, M, 'Cape sim: pin the hem so it stops clipping the legs', [{ path: f.path, size: f.size + 3 * MB, op: 'edit' }]);
    const i = claims.findIndex((c) => c.project_id === P.id && c.path === f.path);
    if (i >= 0) {
      claims.splice(i, 1);
      emit({ kind: 'claim.released', org_id: P.org_id, project_id: P.id, actor: userRef(M), data: { paths: [f.path], branch: main.id } });
    }
  }, 60_000);
  setTimeout(() => {
    if (claims.some((c) => c.project_id === P.id && c.path === 'Characters/Hero/hero_body.fbx' && c.user.id === ME.id))
      emit({ kind: 'claim.nudged', org_id: P.org_id, project_id: P.id, actor: userRef(S), data: { path: 'Characters/Hero/hero_body.fbx', holder: ME.id } });
  }, 90_000);
}

export function mockSubscribe(projectId: string, fn: (e: MpEvent) => void): () => void {
  const p = projectById(projectId);
  const s: Sub = { project: p?.id || projectId, fn };
  subs.add(s);
  runScript();
  return () => subs.delete(s);
}

// handy in the console while testing
(window as unknown as { __mpMock: unknown }).__mpMock = { users, projects, claims, workspaces, events };
