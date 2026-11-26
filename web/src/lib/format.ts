export function size(bytes: number | undefined | null): string {
  if (bytes == null || !isFinite(bytes)) return '';
  const KB = 1024;
  const MB = KB * 1024;
  const GB = MB * 1024;
  const TB = GB * 1024;
  if (bytes >= TB) return (bytes / TB).toFixed(1) + ' TB';
  if (bytes >= GB) return (bytes / GB).toFixed(1) + ' GB';
  if (bytes >= MB) {
    const v = bytes / MB;
    return (v >= 100 ? Math.round(v) : v.toFixed(1)) + ' MB';
  }
  if (bytes >= KB) return Math.round(bytes / KB) + ' KB';
  return bytes + ' bytes';
}

/** "12 of 40 GB" with a shared unit when it reads well. */
export function sizeOf(part: number, whole: number): string {
  const w = size(whole);
  const unit = w.split(' ')[1];
  const p = size(part);
  if (p.split(' ')[1] === unit) return `${p.split(' ')[0]} of ${w}`;
  return `${p} of ${w}`;
}

export const plural = (n: number, one: string, many = one + 's') => `${n.toLocaleString()} ${n === 1 ? one : many}`;

const sameDay = (a: Date, b: Date) =>
  a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();

export function clock(iso: string): string {
  return new Date(iso).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
}

export function dayLabel(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  if (sameDay(d, now)) return 'Today';
  const y = new Date(now);
  y.setDate(now.getDate() - 1);
  if (sameDay(d, y)) return 'Yesterday';
  const diff = (now.getTime() - d.getTime()) / 86_400_000;
  if (diff < 6) return d.toLocaleDateString(undefined, { weekday: 'long' });
  return d.toLocaleDateString(undefined, { day: 'numeric', month: 'long', year: d.getFullYear() === now.getFullYear() ? undefined : 'numeric' });
}

export function when(iso: string | undefined | null): string {
  if (!iso) return '';
  const d = new Date(iso);
  const day = dayLabel(iso);
  if (day === 'Today') return clock(iso);
  if (day === 'Yesterday') return `Yesterday ${clock(iso)}`;
  return d.toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: d.getFullYear() === new Date().getFullYear() ? undefined : 'numeric' });
}

export function since(iso: string | undefined | null): string {
  if (!iso) return 'never';
  const s = (Date.now() - new Date(iso).getTime()) / 1000;
  if (s < 0) return 'just now';
  if (s < 60) return 'just now';
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  if (s < 86400) return plural(Math.round(s / 3600), 'hour') + ' ago';
  if (s < 86400 * 14) return plural(Math.round(s / 86400), 'day') + ' ago';
  return when(iso);
}

/** Duration like "40 min" or "2 hours". */
export function held(iso: string): string {
  const s = (Date.now() - new Date(iso).getTime()) / 1000;
  if (s < 3600) return `${Math.max(1, Math.round(s / 60))} min`;
  if (s < 86400) return plural(Math.round(s / 3600), 'hour');
  return plural(Math.round(s / 86400), 'day');
}

export function dateTime(iso: string | undefined | null): string {
  if (!iso) return '';
  return new Date(iso).toLocaleString(undefined, { day: 'numeric', month: 'short', year: 'numeric', hour: 'numeric', minute: '2-digit' });
}

export function until(iso: string | undefined | null): string {
  if (!iso) return 'Never expires';
  const d = new Date(iso);
  if (d.getTime() < Date.now()) return 'Expired';
  return `Until ${d.toLocaleDateString(undefined, { day: 'numeric', month: 'short' })}`;
}

export const baseName = (path: string) => path.split('/').pop() || path;
export const dirName = (path: string) => path.split('/').slice(0, -1).join('/');

export function hue(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0;
  return h % 360;
}

const PEOPLE = [
  ['#c9a6ff', '#1c0a36'],
  ['#7dffcf', '#00241a'],
  ['#ffd23f', '#2a2000'],
  ['#ff9a5e', '#3a1500'],
  ['#9fd4ff', '#00213a'],
  ['#ffb3d1', '#3a0019'],
  ['#d7f36b', '#1f2600'],
  ['#ece9f1', '#1b1324'],
];
const WALL = ['#e6dbff', '#d3f3e8', '#fbeeb8', '#ffe1cf', '#dceeff', '#ffe0ec', '#eef7c9', '#ece9f1'];

export function personColors(id: string): { bg: string; fg: string; wall: string } {
  const i = hue(id) % PEOPLE.length;
  return { bg: PEOPLE[i][0], fg: PEOPLE[i][1], wall: WALL[i] };
}

export const firstName = (name: string) => name.split(/\s+/)[0] || name;
export const initial = (name: string) => (name.trim()[0] || '?').toUpperCase();

export const ROLE_LABEL: Record<string, string> = { owner: 'Owner', admin: 'Admin', member: 'Member', guest: 'Guest' };

export const KIND_LABEL: Record<string, string> = {
  model: 'Model',
  texture: 'Texture',
  image: 'Image',
  audio: 'Audio',
  video: 'Video',
  level: 'Level',
  scene: 'Scene',
  code: 'Code',
  text: 'Text',
  cache: 'Sim cache',
  other: 'Other',
};

/** Storage location kinds in plain words. */
export const STORAGE_KIND_LABEL: Record<string, string> = {
  local: 'Folder on the server',
  mount: 'Network share',
  s3: 'S3 bucket',
};

/** How far a move of file data has got, 0 to 100, or null when the total is not known yet. */
export function movePercent(m: { total_chunks: number; copied_chunks: number }): number | null {
  if (!m.total_chunks) return null;
  return Math.min(100, Math.floor((m.copied_chunks / m.total_chunks) * 100));
}

/** A move's status in words, for example "Moving… 40%". */
export function moveStatus(m: { status: string; total_chunks: number; copied_chunks: number; error: string | null }): string {
  switch (m.status) {
    case 'running': {
      const pc = movePercent(m);
      return pc === null ? 'Moving…' : `Moving… ${pc}%`;
    }
    case 'done':
      return 'Done';
    case 'failed':
      return m.error ? `Failed: ${m.error}` : 'Failed';
    case 'interrupted':
      return 'Stopped by a restart — start it again to finish';
    default:
      return m.status;
  }
}

/** Binary kinds are claimed; text merges. */
export const isLockable = (kind: string) => !['code', 'text'].includes(kind);

export function eventLabel(kind: string): string {
  const map: Record<string, string> = {
    'change.published': 'Published a change',
    'change.merged': 'Merged a branch',
    'claim.added': 'Claimed files',
    'claim.released': 'Released files',
    'claim.nudged': 'Asked for a file',
    'tag.applied': 'Tagged files',
    'tag.removed': 'Removed a tag',
    'branch.created': 'Made a branch',
    'branch.archived': 'Archived a branch',
    'draft.saved': 'Saved a draft',
    'member.added': 'Added a member',
    'member.suspended': 'Suspended someone',
    'member.reactivated': 'Reactivated someone',
    'member.removed': 'Removed someone from an organization',
    'project.created': 'Made a project',
    'project.archived': 'Archived a project',
    'project.restored': 'Restored a project',
  };
  if (map[kind]) return map[kind];
  const rest = kind.replace(/^admin\./, '').replace(/[._]/g, ' ');
  return 'Admin: ' + rest;
}
