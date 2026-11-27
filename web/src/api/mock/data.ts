// Realistic in-memory data for a small studio. Only loaded in mock mode.
import type {
  Branch,
  Change,
  ChangeFile,
  Checkpoint,
  CheckpointSetting,
  Claim,
  Draft,
  Invite,
  LocalStatus,
  MpEvent,
  Org,
  PublishEntry,
  Role,
  Session,
  StorageKind,
  StorageMove,
  Tag,
  Workspace,
} from '../types';

const NOW = Date.now();
const MIN = 60_000;
const HOUR = 60 * MIN;
export const DAY = 24 * HOUR;
export const MB = 1024 * 1024;
export const GB = 1024 * MB;
export const ago = (ms: number) => new Date(NOW - ms).toISOString();
export const nowIso = () => new Date().toISOString();

let seed = 7;
export function rand() {
  seed |= 0;
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const pick = <T,>(a: T[]) => a[Math.floor(rand() * a.length)];

let idn = 1000;
export const uid = (prefix = '') => {
  idn += 1;
  const hex = idn.toString(16).padStart(12, '0');
  return `${prefix || '00000000'}-0000-4000-8000-${hex}`.slice(0, 36);
};
export const hash = () => Array.from({ length: 16 }, () => Math.floor(rand() * 16).toString(16)).join('');

// ---------- people ----------

export type MUser = {
  id: string;
  name: string;
  email: string;
  status: 'active' | 'suspended';
  is_server_admin: boolean;
  created_at: string;
  last_seen_at?: string;
  suspended_reason?: string;
};

const U = (key: string, name: string, email: string, extra: Partial<MUser> = {}): MUser => ({
  id: `0000${key}-0000-4000-8000-00000000000${key.slice(-1)}`.slice(0, 36),
  name,
  email,
  status: 'active',
  is_server_admin: false,
  created_at: ago(400 * DAY),
  last_seen_at: ago(Math.floor(rand() * 5 * HOUR)),
  ...extra,
});

export const users: MUser[] = [
  U('0001', 'Justin', 'justin@cinderworks.example', { is_server_admin: true, last_seen_at: ago(MIN) }),
  U('0002', 'Mara Lind', 'mara@cinderworks.example'),
  U('0003', 'Teo Barros', 'teo@cinderworks.example'),
  U('0004', 'Dev Anand', 'dev@cinderworks.example'),
  U('0005', 'Sana Kim', 'sana@cinderworks.example'),
  U('0006', 'Pell Varga', 'pell@cinderworks.example', {
    status: 'suspended',
    suspended_reason: 'Left the studio on 14 Nov. Claims kept for handover.',
    last_seen_at: ago(13 * DAY),
  }),
  U('0007', 'Ines Okafor', 'ines@northlight.example', { created_at: ago(200 * DAY) }),
  U('0008', 'Rowan Hale', 'rowan@northlight.example', { created_at: ago(180 * DAY), last_seen_at: ago(2 * DAY) }),
  U('0009', 'Quinn Ashby', 'quinn@northlight.example', { created_at: ago(30 * DAY), last_seen_at: ago(6 * DAY) }),
];
export const [ME, MARA, TEO, DEV, SANA, PELL, INES, ROWAN] = users;
export const userRef = (u: MUser) => ({ id: u.id, name: u.name, email: u.email });
export const userById = (id: string) => users.find((u) => u.id === id);

// ---------- organizations ----------

export const orgs: Org[] = [
  { id: '0rg00001-0000-4000-8000-000000000001', slug: 'cinder-works', name: 'Cinder Works', created_at: ago(400 * DAY) },
  { id: '0rg00002-0000-4000-8000-000000000002', slug: 'northlight', name: 'Northlight Games', created_at: ago(220 * DAY) },
];
export const [CINDER, NORTH] = orgs;

export type MMember = { org_id: string; user_id: string; role: Role; expires_at?: string; created_at: string };
export const members: MMember[] = [
  { org_id: CINDER.id, user_id: ME.id, role: 'owner', created_at: ago(400 * DAY) },
  { org_id: CINDER.id, user_id: MARA.id, role: 'member', created_at: ago(380 * DAY) },
  { org_id: CINDER.id, user_id: TEO.id, role: 'member', created_at: ago(300 * DAY) },
  { org_id: CINDER.id, user_id: DEV.id, role: 'admin', created_at: ago(390 * DAY) },
  { org_id: CINDER.id, user_id: SANA.id, role: 'member', created_at: ago(120 * DAY) },
  { org_id: CINDER.id, user_id: PELL.id, role: 'member', created_at: ago(260 * DAY) },
  { org_id: NORTH.id, user_id: INES.id, role: 'owner', created_at: ago(220 * DAY) },
  { org_id: NORTH.id, user_id: ROWAN.id, role: 'member', created_at: ago(180 * DAY) },
  { org_id: NORTH.id, user_id: users[8].id, role: 'member', created_at: ago(30 * DAY) },
  { org_id: NORTH.id, user_id: ME.id, role: 'guest', expires_at: new Date(NOW + 18 * DAY).toISOString(), created_at: ago(40 * DAY) },
  { org_id: NORTH.id, user_id: MARA.id, role: 'guest', expires_at: new Date(NOW + 18 * DAY).toISOString(), created_at: ago(40 * DAY) },
];

// ---------- projects ----------

export type MFile = { path: string; size: number; file_hash: string; changes: number[] };
export type MChange = Change & { files: ChangeFile[] };
export type MProject = {
  id: string;
  slug: string;
  name: string;
  org_id: string;
  archived: boolean;
  /** where the project is set to keep checkpoints */
  checkpoints: CheckpointSetting;
  auto_checkpoints?: boolean;
  /** storage location holding the file data */
  storage: string;
  /** where the data was before an unfinished move */
  storage_previous: string | null;
  created_at: string;
  head: number;
  files: Map<string, MFile[]>; // by branch id
  changes: MChange[];
  branches: Branch[];
  merge: Map<string, MergeState>;
  /** where it is filed (both optional; a production decides the client) */
  client_id?: string | null;
  production_id?: string | null;
};
export type MergeState = { updated: boolean; only_here: number; only_parent: number; auto: string[]; conflicts: Array<{ path: string; kind: 'text' | 'binary' }> };

const KIND_BY_EXT: Record<string, string> = {
  fbx: 'model', obj: 'model', blend: 'model', ma: 'model', mb: 'model', gltf: 'model', glb: 'model', prefab: 'scene',
  png: 'texture', tga: 'texture', psd: 'image', exr: 'image', jpg: 'image', tif: 'texture', kra: 'image',
  wav: 'audio', ogg: 'audio', mp3: 'audio', mov: 'video', mp4: 'video',
  umap: 'level', unity: 'level', tscn: 'scene', uasset: 'scene',
  cpp: 'code', h: 'code', cs: 'code', rs: 'code', py: 'code', gd: 'code',
  ini: 'text', md: 'text', json: 'text', txt: 'text', csv: 'text', mpignore: 'text',
  abc: 'cache', vdb: 'cache',
};
export const kindOf = (path: string) => KIND_BY_EXT[path.split('.').pop()?.toLowerCase() || ''] || 'other';

function mkProject(id: string, slug: string, name: string, org: Org, head: number, created: number): MProject {
  const mainId = `${id.slice(0, 8)}-b000-4000-8000-000000000001`;
  return {
    id, slug, name, org_id: org.id, archived: false, checkpoints: 'auto', storage: 'main', storage_previous: null, created_at: ago(created), head,
    files: new Map(), changes: [], merge: new Map(),
    branches: [{
      id: mainId, name: 'Main', parent_id: null, shares_claims: true, protected: false, archived: false,
      head_change: head, created_at: ago(created), created_by: userRef(ME), ahead: 0, behind: 0,
    }],
  };
}

const AUTHORS_CINDER = [MARA, TEO, DEV, SANA, ME, PELL];
const AUTHORS_NORTH = [INES, ROWAN, users[8], ME];

type Seed = { path: string; mb: number; changes: number[] };

function build(p: MProject, seeds: Seed[], fillerFolders: Record<string, string[]>, fillerCount: number,
  named: Record<number, [string, ReturnType<typeof userRef>, number]>, authors: MUser[], msgs: string[], first: number) {
  const main = p.branches[0];
  const files: MFile[] = seeds.map((s) => ({ path: s.path, size: Math.round(s.mb * MB), file_hash: hash(), changes: [...s.changes] }));
  const used = new Set(files.flatMap((f) => f.changes));
  const folders = Object.keys(fillerFolders);
  for (let i = 0; i < fillerCount; i++) {
    const folder = pick(folders);
    const base = pick(fillerFolders[folder]);
    const path = `${folder}/${base.replace('#', String(i + 1).padStart(2, '0'))}`;
    if (files.some((f) => f.path === path)) continue;
    const ext = path.split('.').pop() || '';
    const mb = ['cpp', 'h', 'cs', 'ini', 'json', 'md', 'gd', 'txt'].includes(ext) ? rand() * 0.08 + 0.002
      : ['wav', 'ogg'].includes(ext) ? rand() * 20 + 1 : ['mov', 'mp4'].includes(ext) ? rand() * 900 + 60
      : ['umap', 'unity'].includes(ext) ? rand() * 120 + 20 : rand() * 40 + 2;
    files.push({ path, size: Math.round(mb * MB), file_hash: hash(), changes: [] });
  }
  // every change touches at least one file
  for (let n = first; n <= p.head; n++) {
    if (!used.has(n)) {
      const fillers = files.slice(seeds.length);
      const k = 1 + Math.floor(rand() * 3);
      for (let j = 0; j < k; j++) {
        const f = pick(fillers);
        if (!f.changes.includes(n)) f.changes.push(n);
      }
    }
  }
  for (const f of files) {
    if (!f.changes.length) f.changes.push(first + Math.floor(rand() * (p.head - first)));
    f.changes.sort((a, b) => a - b);
  }
  p.files.set(main.id, files);

  let t = 0;
  for (let n = p.head; n >= first; n--) {
    const nm = named[n];
    t = nm ? nm[2] : t + (1.5 + rand() * 4) * HOUR;
    const touched = files.filter((f) => f.changes.includes(n));
    const ch: MChange = {
      number: n,
      branch_id: main.id,
      message: nm ? nm[0] : pick(msgs),
      author: nm ? nm[1] : userRef(pick(authors)),
      created_at: ago(t),
      kind: 'publish',
      file_count: touched.length,
      size: touched.reduce((a, f) => a + f.size, 0),
      files: touched.map((f) => ({ path: f.path, op: f.changes[0] === n ? 'add' : 'edit', size: f.size })),
    };
    p.changes.push(ch);
  }
}

// --- Emberfall (Unreal) ---
export const EMBER = mkProject('pr000001-0000-4000-8000-000000000001', 'emberfall', 'Emberfall', CINDER, 4812, 330 * DAY);
build(EMBER, [
  { path: 'Characters/Hero/hero_body.fbx', mb: 48.2, changes: [4760, 4790, 4806, 4812] },
  { path: 'Characters/Hero/hero_body_diffuse.png', mb: 22.1, changes: [4775, 4812] },
  { path: 'Characters/Hero/hero_cape_sim.abc', mb: 1434, changes: [4775, 4801, 4809] },
  { path: 'Characters/Hero/hero_rig.blend', mb: 64.5, changes: [4760, 4790] },
  { path: 'Characters/Wolf/ember_wolf.fbx', mb: 31.7, changes: [4760, 4790, 4806] },
  { path: 'Characters/Wolf/ember_wolf_normal.png', mb: 18, changes: [4790, 4806] },
  { path: 'Environments/Forge/forge_door.fbx', mb: 9.3, changes: [4760, 4790] },
  { path: 'Environments/Forge/forge_door_rust.png', mb: 12.6, changes: [4790, 4801] },
  { path: 'Environments/Sky/ash_plains_sky.exr', mb: 212, changes: [4775] },
  { path: 'Cinematics/intro_cinematic.mov', mb: 3891, changes: [4760, 4801, 4811] },
  { path: 'Audio/Creatures/wolf_howl_03.wav', mb: 2.2, changes: [4806, 4812] },
  { path: 'Levels/forge_interior.umap', mb: 96.4, changes: [4790, 4801, 4810] },
  { path: '.mpignore', mb: 0.001, changes: [4700, 4744] },
], {
  'Characters/Villager': ['villager_#.fbx', 'villager_#_diffuse.png', 'villager_#_rig.blend'],
  'Characters/Hero/Anims': ['hero_run_#.fbx', 'hero_idle_#.fbx'],
  'Environments/Forge': ['forge_prop_#.fbx', 'forge_prop_#_albedo.png'],
  'Environments/Ash Plains': ['rock_cluster_#.fbx', 'ash_ground_#.png', 'dead_tree_#.fbx'],
  'Audio/Music': ['ember_theme_#.wav'],
  'Audio/Creatures': ['wolf_growl_#.wav'],
  'Levels': ['ash_plains_#.umap'],
  'Source/Emberfall': ['EmberCharacter#.cpp', 'EmberCharacter#.h', 'WolfAI#.cpp'],
  'Config': ['DefaultGame#.ini'],
  'UI/Icons': ['icon_item_#.png'],
}, 90, {
  4812: ['Hero body retopo, first pass', userRef(SANA), 38 * MIN],
  4811: ['Intro cinematic, colour pass and new end card', userRef(DEV), 60 * MIN],
  4810: ['Forge interior lighting rebuilt after door change', userRef(DEV), 85 * MIN],
  4809: ['Cape sim, longer settle so it stops popping', userRef(MARA), 17 * HOUR],
  4806: ['Wolf normals rebaked at 4k', userRef(TEO), 26 * HOUR],
  4803: ['Merge Lighting test into Main', userRef(DEV), 40 * HOUR],
  4801: ['Forge door rust, cape cache and cinematic timing', userRef(TEO), 46 * HOUR],
}, AUTHORS_CINDER, [
  'Villager rig fixes for the market scene', 'Ash plains rock pass', 'Wolf AI: stop circling at cliff edges',
  'Forge props, second batch', 'Music stems for the forge', 'Item icons, round two', 'Tidy up config for the demo build',
  'Hero run cycle polish', 'Dead trees, LODs', 'Ground textures at 2k', 'Fix crash when the wolf loses its target',
  'Footstep sounds on ash', 'Lighting pass on the market', 'New villager variant',
], 4700);
// a merge on Main
{
  const m = EMBER.changes.find((c) => c.number === 4803)!;
  m.kind = 'merge';
  m.merged_from = [4796, 4799, 4802];
}

export const EMBER_MAIN = EMBER.branches[0];
export const EMBER_WOLF: Branch = {
  id: 'pr000001-b000-4000-8000-000000000002', name: 'Wolf rework', parent_id: EMBER_MAIN.id, shares_claims: true,
  protected: false, archived: false, head_change: 4808, created_at: ago(5 * DAY), created_by: userRef(TEO), ahead: 27, behind: 41,
};
export const EMBER_DEMO: Branch = {
  id: 'pr000001-b000-4000-8000-000000000003', name: 'Vertical slice demo', parent_id: EMBER_MAIN.id, shares_claims: false,
  protected: true, archived: false, head_change: 4805, created_at: ago(9 * DAY), created_by: userRef(DEV), ahead: 2, behind: 0,
};
export const EMBER_LIGHT: Branch = {
  id: 'pr000001-b000-4000-8000-000000000004', name: 'Lighting test', parent_id: EMBER_MAIN.id, shares_claims: true,
  protected: false, archived: true, head_change: 4802, created_at: ago(12 * DAY), created_by: userRef(DEV), ahead: 0, behind: 0,
};
EMBER.branches.push(EMBER_WOLF, EMBER_DEMO, EMBER_LIGHT);
for (const n of [4795, 4798, 4800, 4804, 4807, 4808]) {
  const c = EMBER.changes.find((x) => x.number === n);
  if (c) {
    c.branch_id = EMBER_WOLF.id;
    c.message = pick(['Wolf rework: new fur cards', 'Wolf rework: rig with tail chain', 'Wolf rework: bite anims', 'Wolf rework: LODs and materials']);
    c.author = userRef(pick([TEO, MARA]));
  }
}
for (const n of [4796, 4799, 4802]) {
  const c = EMBER.changes.find((x) => x.number === n);
  if (c) {
    c.branch_id = EMBER_LIGHT.id;
    c.message = 'Lighting test: ' + pick(['GI settings', 'sky light rebuild', 'exposure curve']);
  }
}
for (const n of [4785, 4805]) {
  const c = EMBER.changes.find((x) => x.number === n);
  if (c) {
    c.branch_id = EMBER_DEMO.id;
    c.message = n === 4805 ? 'Demo: lock wolf to the old rig' : 'Demo: trim the forge level';
  }
}
EMBER.merge.set(EMBER_WOLF.id, {
  updated: false, only_here: 27, only_parent: 41,
  auto: ['Source/Emberfall/WolfAI01.cpp', 'Config/DefaultGame01.ini'], conflicts: [],
});
EMBER.merge.set(EMBER_DEMO.id, {
  updated: true, only_here: 2, only_parent: 0, auto: [],
  conflicts: [{ path: 'Characters/Wolf/ember_wolf.fbx', kind: 'binary' }],
});

// --- Tidewrack (Unity) ---
export const TIDE = mkProject('pr000002-0000-4000-8000-000000000002', 'tidewrack', 'Tidewrack', CINDER, 1296, 160 * DAY);
build(TIDE, [
  { path: 'Assets/Art/Props/buoy_rusty.fbx', mb: 6.4, changes: [1190, 1288] },
  { path: 'Assets/Art/Props/net_pile.fbx', mb: 11.2, changes: [1201, 1293] },
  { path: 'Assets/Scenes/Harbor.unity', mb: 54, changes: [1185, 1240, 1296] },
  { path: 'Assets/Art/Characters/deckhand_rig.blend', mb: 88.1, changes: [1199, 1250] },
], {
  'Assets/Art/Props': ['crate_#.fbx', 'crate_#_albedo.png', 'rope_coil_#.fbx'],
  'Assets/Art/Characters': ['deckhand_#.fbx', 'deckhand_#_albedo.png'],
  'Assets/Art/Water': ['foam_#.png', 'wave_normal_#.png'],
  'Assets/Scenes': ['Island_#.unity'],
  'Assets/Scripts': ['BoatController#.cs', 'Tide#.cs', 'Cargo#.cs'],
  'Assets/Audio': ['gulls_#.wav', 'creak_#.wav'],
  'Design': ['tide_tables_#.csv', 'level_notes_#.md'],
}, 70, {
  1296: ['Harbor scene: move the crane, fix lightmap seams', userRef(MARA), 3 * HOUR],
  1295: ['Boat controller: better reverse thrust', userRef(DEV), 5 * HOUR],
}, AUTHORS_CINDER, [
  'Crates, weathered set', 'Tide tables for week two', 'Deckhand rig tweaks', 'Gull sounds', 'Water foam textures',
  'Cargo script cleanup', 'Island blockout', 'Rope coils', 'Fix boat clipping at the pier',
], 1180);

// --- Harbor DLC (Northlight) ---
export const HARBOR = mkProject('pr000003-0000-4000-8000-000000000003', 'harbor-dlc', 'Harbor DLC', NORTH, 312, 70 * DAY);
build(HARBOR, [
  { path: 'Maps/harbor_night.umap', mb: 140.5, changes: [244, 290, 311] },
  { path: 'Art/Ships/schooner_hull.fbx', mb: 41, changes: [250, 312] },
], {
  'Art/Ships': ['ship_part_#.fbx', 'sail_#_albedo.png'],
  'Art/Town': ['house_#.fbx', 'lamp_#.fbx', 'cobble_#.png'],
  'Maps': ['harbor_day_#.umap'],
  'Audio/Ambience': ['harbor_night_#.wav'],
  'Docs': ['dlc_scope_#.md'],
}, 45, {
  312: ['Schooner hull, damage states', userRef(ROWAN), 2 * DAY],
  311: ['Harbor at night: fog volume and lamps', userRef(INES), 3 * DAY],
}, AUTHORS_NORTH, ['Town houses, set B', 'Sail textures', 'Night ambience loop', 'Lamps with flicker', 'Cobbles at 2k', 'DLC scope update'], 240);

// --- archived ---
export const PROTO = mkProject('pr000004-0000-4000-8000-000000000004', 'prototype-2024', 'Prototype 2024', CINDER, 88, 700 * DAY);
build(PROTO, [{ path: 'Proto/test_level.umap', mb: 30, changes: [1, 40, 88] }], { Proto: ['blockout_#.fbx'] }, 12, {}, AUTHORS_CINDER, ['Blockout', 'Test'], 1);
PROTO.archived = true;

export const projects: MProject[] = [EMBER, TIDE, HARBOR, PROTO];

// --- clients and productions: Organization > Client > Production > Project ---
export type MClient = { id: string; org_id: string; name: string; created_at: string };
export type MProduction = { id: string; org_id: string; client_id: string | null; name: string; main_project_id: string | null; created_at: string };
export const clients: MClient[] = [
  { id: 'cl000001-0000-4000-8000-000000000001', org_id: CINDER.id, name: 'Ashgrove Publishing', created_at: ago(300 * DAY) },
  { id: 'cl000002-0000-4000-8000-000000000002', org_id: CINDER.id, name: 'Saltmarsh Studios', created_at: ago(120 * DAY) },
];
export const productions: MProduction[] = [
  { id: 'pd000001-0000-4000-8000-000000000001', org_id: CINDER.id, client_id: clients[0].id, name: 'Emberfall', main_project_id: EMBER.id, created_at: ago(300 * DAY) },
];
EMBER.client_id = clients[0].id;
EMBER.production_id = productions[0].id;
export const projectById = (id: string) => projects.find((p) => p.id === id || p.slug === id);

export function filesOf(p: MProject, branchId: string): MFile[] {
  const own = p.files.get(branchId);
  if (own) return own;
  // branches share Main's files in the mock
  const main = p.files.get(p.branches[0].id)!;
  const copy = main.map((f) => ({ ...f, changes: [...f.changes] }));
  p.files.set(branchId, copy);
  return copy;
}

// ---------- claims ----------

export type MClaim = Claim & { project_id: string };
const scopeOf = (p: MProject, b: Branch) => (b.shares_claims && b.parent_id ? p.branches[0].id : b.id);
export { scopeOf };

export const claims: MClaim[] = [
  { project_id: EMBER.id, path: 'Characters/Hero/hero_body.fbx', branch_id: EMBER_MAIN.id, user: userRef(ME), device: 'Workstation', created_at: ago(70 * MIN) },
  { project_id: EMBER.id, path: 'Characters/Hero/hero_body_diffuse.png', branch_id: EMBER_MAIN.id, user: userRef(ME), device: 'Workstation', created_at: ago(55 * MIN) },
  { project_id: EMBER.id, path: 'Audio/Creatures/wolf_howl_03.wav', branch_id: EMBER_MAIN.id, user: userRef(ME), device: 'Workstation', created_at: ago(30 * MIN) },
  { project_id: EMBER.id, path: 'Characters/Hero/hero_cape_sim.abc', branch_id: EMBER_MAIN.id, user: userRef(MARA), device: 'Mara-PC', created_at: ago(40 * MIN) },
  { project_id: EMBER.id, path: 'Environments/Forge/forge_door_rust.png', branch_id: EMBER_MAIN.id, user: userRef(TEO), device: 'Teo-Mac', created_at: ago(2 * HOUR) },
  { project_id: EMBER.id, path: 'Levels/forge_interior.umap', branch_id: EMBER_MAIN.id, user: userRef(DEV), device: 'Dev-Workstation', created_at: ago(2.5 * HOUR) },
  { project_id: EMBER.id, path: 'Characters/Wolf/ember_wolf_normal.png', branch_id: EMBER_MAIN.id, user: userRef(TEO), device: 'Teo-Mac', created_at: ago(26 * HOUR) },
  { project_id: TIDE.id, path: 'Assets/Art/Characters/deckhand_rig.blend', branch_id: TIDE.branches[0].id, user: userRef(PELL), device: 'Pell-Laptop', created_at: ago(14 * DAY) },
  { project_id: TIDE.id, path: 'Assets/Scenes/Harbor.unity', branch_id: TIDE.branches[0].id, user: userRef(PELL), device: 'Pell-Laptop', created_at: ago(14 * DAY) },
  { project_id: TIDE.id, path: 'Assets/Art/Props/net_pile.fbx', branch_id: TIDE.branches[0].id, user: userRef(MARA), device: 'Mara-PC', created_at: ago(3 * HOUR) },
  { project_id: HARBOR.id, path: 'Maps/harbor_night.umap', branch_id: HARBOR.branches[0].id, user: userRef(INES), device: 'ines-mbp', created_at: ago(4 * DAY) },
];

// ---------- tags ----------

export const tags: Tag[] = [
  { id: 'tag00001-0000-4000-8000-000000000001', name: 'Approved', color: '#00806a', group: 'Status', org_id: CINDER.id },
  { id: 'tag00002-0000-4000-8000-000000000002', name: 'Needs review', color: '#ff6a13', group: 'Status', org_id: CINDER.id },
  { id: 'tag00003-0000-4000-8000-000000000003', name: 'Hero asset', color: '#7a3cff', org_id: CINDER.id },
  { id: 'tag00004-0000-4000-8000-000000000004', name: 'HDRI', color: '#1b1324', org_id: CINDER.id, project_id: EMBER.id },
  { id: 'tag00005-0000-4000-8000-000000000005', name: 'Blocked', color: '#c2185b', org_id: CINDER.id },
  { id: 'tag00006-0000-4000-8000-000000000006', name: 'Approved', color: '#00806a', org_id: NORTH.id },
  { id: 'tag00007-0000-4000-8000-000000000007', name: 'Final', color: '#1b1324', org_id: NORTH.id },
];
export type MPathTag = { project_id: string; path: string; tag_id: string };
const T = (path: string, name: string, p = EMBER): MPathTag => ({
  project_id: p.id, path, tag_id: tags.find((t) => t.name === name && t.org_id === p.org_id)!.id,
});
export const pathTags: MPathTag[] = [
  T('Characters/Hero/hero_body.fbx', 'Hero asset'), T('Characters/Hero/hero_body.fbx', 'Needs review'),
  T('Characters/Hero/hero_body_diffuse.png', 'Hero asset'), T('Characters/Hero/hero_cape_sim.abc', 'Hero asset'),
  T('Characters/Hero/hero_cape_sim.abc', 'Blocked'), T('Environments/Sky/ash_plains_sky.exr', 'HDRI'),
  T('Environments/Sky/ash_plains_sky.exr', 'Approved'), T('Cinematics/intro_cinematic.mov', 'Approved'),
  T('Environments/Forge/forge_door.fbx', 'Approved'), T('Characters/Wolf/ember_wolf.fbx', 'Needs review'),
  T('Assets/Scenes/Harbor.unity', 'Needs review', TIDE), T('Maps/harbor_night.umap', 'Final', HARBOR),
];

// ---------- drafts ----------

export type MDraft = Draft & { project_id: string; user_id: string; entries: PublishEntry[] };
export const drafts: MDraft[] = [
  {
    id: 'drf00001-0000-4000-8000-000000000001', project_id: EMBER.id, user_id: ME.id, title: 'Villager 02, half-finished rig',
    branch_id: EMBER_MAIN.id, created_at: ago(2 * DAY + 3 * HOUR), file_count: 2, size: Math.round(71.4 * MB),
    entries: [
      { path: 'Characters/Villager/villager_02_rig.blend', op: 'put', prev_file_hash: null, manifest: { hash: hash(), size: Math.round(52 * MB), file_hash: hash(), chunks: [] } },
      { path: 'Characters/Villager/villager_02_diffuse.png', op: 'put', prev_file_hash: hash(), manifest: { hash: hash(), size: Math.round(19.4 * MB), file_hash: hash(), chunks: [] } },
    ],
  },
  {
    id: 'drf00002-0000-4000-8000-000000000002', project_id: EMBER.id, user_id: ME.id, title: 'Try a darker forge door',
    branch_id: EMBER_MAIN.id, created_at: ago(6 * DAY), file_count: 1, size: Math.round(12.1 * MB),
    entries: [{ path: 'Environments/Forge/forge_door_rust.png', op: 'put', prev_file_hash: hash(), manifest: { hash: hash(), size: Math.round(12.1 * MB), file_hash: hash(), chunks: [] } }],
  },
];

// ---------- invites and sessions ----------

export type MInvite = Invite & { org_id: string; token: string };
export const invites: MInvite[] = [
  { id: 'inv00001-0000-4000-8000-000000000001', org_id: CINDER.id, token: 'welcome-ash', email: 'lena@cinderworks.example', role: 'member', expires_at: new Date(NOW + 6 * DAY).toISOString(), created_at: ago(DAY), created_by: userRef(ME), url: `${location.origin}/invite/welcome-ash` },
  { id: 'inv00002-0000-4000-8000-000000000002', org_id: CINDER.id, token: 'outsource-pass', role: 'guest', expires_at: new Date(NOW + 13 * DAY).toISOString(), created_at: ago(3 * DAY), created_by: userRef(DEV), url: `${location.origin}/invite/outsource-pass` },
  { id: 'inv00003-0000-4000-8000-000000000003', org_id: CINDER.id, token: 'used-one', email: 'sana@cinderworks.example', role: 'member', created_at: ago(120 * DAY), created_by: userRef(ME), url: `${location.origin}/invite/used-one`, accepted_at: ago(120 * DAY) },
  { id: 'inv00004-0000-4000-8000-000000000004', org_id: NORTH.id, token: 'north-art', email: 'kai@northlight.example', role: 'member', expires_at: new Date(NOW + 2 * DAY).toISOString(), created_at: ago(5 * DAY), created_by: userRef(INES), url: `${location.origin}/invite/north-art` },
];

export const sessions = new Map<string, Session[]>();
for (const u of users) {
  const list: Session[] = [
    { id: uid('ses00001'), kind: 'session', device: pick(['Windows desktop app', 'macOS desktop app', 'Web browser']), created_at: ago(9 * DAY), last_used_at: u.last_seen_at },
  ];
  if (rand() > 0.4) list.push({ id: uid('ses00002'), kind: 'session', device: 'Web browser', created_at: ago(2 * DAY), last_used_at: ago(5 * HOUR) });
  if (u === DEV || u === ME) list.push({ id: uid('tok00001'), kind: 'token', label: 'CI build agent', created_at: ago(90 * DAY), last_used_at: ago(20 * MIN) });
  sessions.set(u.id, list);
}

// ---------- events (audit log) ----------

export const events: MpEvent[] = [];
let evId = 9000;
export function pushEvent(e: Omit<MpEvent, 'id' | 'created_at'> & { created_at?: string }): MpEvent {
  evId += 1;
  const full: MpEvent = { id: evId, created_at: e.created_at || nowIso(), ...e } as MpEvent;
  events.push(full);
  return full;
}
{
  const seedEvents: Array<[string, MUser, string | undefined, MProject | undefined, Record<string, unknown>, number]> = [
    ['admin.user.suspended', ME, CINDER.id, undefined, { user: PELL.name, reason: 'Left the studio on 14 Nov.' }, 12 * DAY],
    ['member.added', ME, CINDER.id, undefined, { user: SANA.name, role: 'member' }, 120 * DAY],
    ['project.archived', ME, CINDER.id, PROTO, { project: PROTO.name }, 30 * DAY],
    ['branch.created', TEO, CINDER.id, EMBER, { branch: 'Wolf rework', from: 'Main' }, 5 * DAY],
    ['change.merged', DEV, CINDER.id, EMBER, { change: 4803, from: 'Lighting test' }, 40 * HOUR],
    ['tag.applied', MARA, CINDER.id, EMBER, { tag: 'Blocked', paths: ['Characters/Hero/hero_cape_sim.abc'] }, 20 * HOUR],
    ['claim.added', MARA, CINDER.id, EMBER, { paths: ['Characters/Hero/hero_cape_sim.abc'] }, 40 * MIN],
    ['change.published', SANA, CINDER.id, EMBER, { change: 4812, files: 3 }, 38 * MIN],
    ['change.published', DEV, CINDER.id, EMBER, { change: 4811, files: 1 }, 60 * MIN],
    ['admin.invite.created', DEV, CINDER.id, undefined, { role: 'guest', link: true }, 3 * DAY],
    ['member.added', INES, NORTH.id, undefined, { user: users[8].name, role: 'member' }, 30 * DAY],
    ['change.published', ROWAN, NORTH.id, HARBOR, { change: 312, files: 4 }, 2 * DAY],
    ['admin.storage.check', ME, undefined, undefined, { checked: 48211, ok: true }, 7 * DAY],
    ['claim.released', TEO, CINDER.id, EMBER, { paths: ['Environments/Forge/forge_door.fbx'] }, 2 * DAY],
  ];
  seedEvents
    .sort((a, b) => b[5] - a[5])
    .forEach(([kind, actor, org, proj, data, t]) =>
      pushEvent({ kind, actor: userRef(actor), org_id: org, project_id: proj?.id, data, created_at: ago(t) }),
    );
}

// ---------- checkpoints kept on the server ----------

// Emberfall and Harbor DLC are set to keep checkpoints on the server; the others decide by size.
EMBER.checkpoints = 'server';
HARBOR.checkpoints = 'server';

export type MServerCheckpoint = { user_id: string; project_id: string; auto: boolean; created_at: string; stored: number; size: number };
export const serverCheckpoints: MServerCheckpoint[] = [];
function kept(u: MUser, p: MProject, named: number, auto: number, oldest: number, newest: number, storedMb: number, sizeMb: number) {
  const n = named + auto;
  for (let i = 0; i < n; i++) {
    serverCheckpoints.push({
      user_id: u.id, project_id: p.id, auto: i % n < auto, created_at: ago(oldest - ((oldest - newest) * i) / Math.max(1, n - 1)),
      stored: Math.round((storedMb * MB) / n), size: Math.round((sizeMb * MB) / n),
    });
  }
}
kept(MARA, EMBER, 4, 19, 9 * DAY, 40 * MIN, 6240, 33100);
kept(PELL, EMBER, 2, 31, 41 * DAY, 13 * DAY, 3870, 12400);
kept(SANA, EMBER, 3, 8, 3 * DAY, 50 * MIN, 912, 1730);
kept(TEO, EMBER, 0, 6, 26 * HOUR, 2 * HOUR, 148, 610);
kept(ROWAN, HARBOR, 1, 12, 6 * DAY, 2 * DAY, 1310, 4480);

// ---------- storage locations ----------

export type MLocation = { name: string; kind: StorageKind; main: boolean; description: string };
export const locations: MLocation[] = [
  { name: 'main', kind: 'local', main: true, description: '/var/lib/multiparity/data' },
  { name: 'nas', kind: 'mount', main: false, description: '/mnt/studio-nas/multiparity' },
  { name: 'archive', kind: 's3', main: false, description: 's3://cinder-archive/multiparity' },
];

/** A move with what the mock needs to play it out over time. */
export type MMove = StorageMove & { ms: number; total_bytes: number };
export const storageMoves: MMove[] = [];
/** old copies left behind by finished moves, by location, until space is reclaimed */
export const leftovers = new Map<string, { chunks: number; bytes: number }>();

// The prototype was moved to the archive bucket a month ago; its old copy was reclaimed since.
PROTO.storage = 'archive';
storageMoves.push({
  id: 'mov00001-0000-4000-8000-000000000001', project_id: PROTO.id, project_name: PROTO.name, from: 'main', to: 'archive', status: 'done',
  total_chunks: 4810, copied_chunks: 4810, copied_bytes: Math.round(612 * MB), error: null, started_at: ago(30 * DAY + 2 * HOUR), finished_at: ago(30 * DAY),
  ms: 0, total_bytes: Math.round(612 * MB),
});
// Harbor DLC was halfway to the NAS when the server restarted.
HARBOR.storage = 'nas';
HARBOR.storage_previous = 'main';
storageMoves.unshift({
  id: 'mov00002-0000-4000-8000-000000000002', project_id: HARBOR.id, project_name: HARBOR.name, from: 'main', to: 'nas', status: 'interrupted',
  total_chunks: 18240, copied_chunks: 7311, copied_bytes: Math.round(2.4 * GB), error: null, started_at: ago(2 * DAY + 40 * MIN), finished_at: ago(2 * DAY),
  ms: 0, total_bytes: Math.round(6.1 * GB),
});

// ---------- local daemon ----------

export type MWorkspace = Workspace & { status: LocalStatus };
const cp = (note: string | undefined, auto: boolean, t: number, files: number): Checkpoint => ({ id: uid('chk00001'), note, auto, created_at: ago(t), files });

export const workspaces: MWorkspace[] = [
  {
    id: 'ws000001-0000-4000-8000-000000000001', root: 'D:/Work/Emberfall', server: location.origin, account_email: ME.email,
    project_id: EMBER.id, project_name: EMBER.name, org_name: CINDER.name, branch_id: EMBER_MAIN.id, branch_name: 'Main',
    synced_change: 4810, include: [''], exclude: ['Cinematics'], local_bytes: 0, total_bytes: 0,
    status: {
      new: [
        { path: 'Characters/Hero/hero_body_normal.png', size: Math.round(19.4 * MB), claimed: true },
        { path: 'Characters/Hero/hero_turnaround.mov', size: Math.round(88 * MB), claimed: false },
      ],
      edited: [
        { path: 'Characters/Hero/hero_body.fbx', size: Math.round(48.2 * MB), claimed: true },
        { path: 'Characters/Hero/hero_body_diffuse.png', size: Math.round(22.1 * MB), claimed: true },
        { path: 'Audio/Creatures/wolf_howl_03.wav', size: Math.round(2.2 * MB), claimed: true },
      ],
      deleted: [],
      behind: 2,
      ignored: 14,
      checkpoints: [cp('before retopo', false, 6 * MIN, 5), cp(undefined, true, 3 * MIN, 1), cp(undefined, true, 22 * MIN, 2)],
    },
  },
  {
    id: 'ws000002-0000-4000-8000-000000000002', root: 'D:/Work/Tidewrack', server: location.origin, account_email: ME.email,
    project_id: TIDE.id, project_name: TIDE.name, org_name: CINDER.name, branch_id: TIDE.branches[0].id, branch_name: 'Main',
    synced_change: 1296, include: ['Assets/Art/Props', 'Assets/Scripts', 'Design'], exclude: [], local_bytes: 0, total_bytes: 0,
    status: { new: [], edited: [], deleted: [], behind: 0, ignored: 3, checkpoints: [] },
  },
];
