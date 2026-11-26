// Client for the local daemon (mpd) on this computer. It is optional: the web UI works without it,
// and shows sync controls and "Your work" only when it answers.
import { MOCK, loadMock } from './mode';
import { ApiError } from './client';
import type { Checkpoint, DaemonHealth, FolderNode, IgnoreChoice, LocalStatus, Workspace } from './types';

export const DAEMON_ORIGIN = 'http://127.0.0.1:18519';

async function transport(url: string, init: RequestInit): Promise<Response> {
  if (MOCK) {
    const m = await loadMock();
    return m.mockDaemonFetch(url, init);
  }
  return fetch(DAEMON_ORIGIN + url, init);
}

async function call<T>(method: string, path: string, body?: unknown, timeoutMs = 15000): Promise<T> {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeoutMs);
  let res: Response;
  try {
    res = await transport(`/local${path}`, {
      method,
      headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: ctl.signal,
    });
  } catch {
    throw new ApiError(0, 'daemon_unreachable', 'The MultiParity app on this computer is not answering.');
  } finally {
    clearTimeout(timer);
  }
  const text = await res.text();
  let data: unknown;
  try {
    data = text ? JSON.parse(text) : {};
  } catch {
    data = {};
  }
  if (!res.ok) {
    const e = (data as { error?: { code?: string; message?: string } }).error;
    throw new ApiError(res.status, e?.code || (res.status === 404 ? 'not_found' : 'daemon'), e?.message || '');
  }
  return data as T;
}

const w = (id: string) => `/workspaces/${encodeURIComponent(id)}`;

export const daemon = {
  /** Short timeout: if nothing answers quickly, there is no daemon on this computer. */
  health: () => call<DaemonHealth>('GET', '/health', undefined, 900),
  workspaces: () => call<Workspace[]>('GET', '/workspaces'),
  status: (id: string) => call<LocalStatus>('GET', `${w(id)}/status`),
  folders: (id: string) => call<FolderNode>('GET', `${w(id)}/folders`),
  setSync: (id: string, include: string[], exclude: string[]) =>
    call<Workspace>('PUT', `${w(id)}/sync`, { include, exclude }),
  getLatest: (id: string) => call<{ downloaded: number; bytes: number }>('POST', `${w(id)}/get-latest`, undefined, 600000),
  publish: (id: string, message: string, paths?: string[]) =>
    call<{ change: { number: number } }>('POST', `${w(id)}/publish`, { message, paths }, 600000),
  checkpoint: (id: string, note?: string) => call<Checkpoint>('POST', `${w(id)}/checkpoint`, { note: note || undefined }),
  /** Leave files matching this pattern out of checkpoints (shared with the project unless personal). */
  noCheckpoint: (id: string, pattern: string, personal: boolean) =>
    call<{ file: string }>('POST', `${w(id)}/no-checkpoint`, { pattern, personal }),
  ignoreSuggest: (id: string, path: string) =>
    call<IgnoreChoice[]>('GET', `${w(id)}/ignore/suggest?path=${encodeURIComponent(path)}`),
  /**
   * Not in API.md v1. Assumed to mirror publish; a daemon without it answers 404 and the UI says
   * that this version of the app cannot save drafts yet.
   */
  saveDraft: (id: string, title: string, paths?: string[]) =>
    call<{ id: string }>('POST', `${w(id)}/draft`, { title, paths }, 600000),
};
