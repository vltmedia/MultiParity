import { MOCK, loadMock } from './mode';
import { getToken } from './client';
import type { MpEvent } from './types';

export const EVENT_KINDS = [
  'change.published',
  'change.merged',
  'claim.added',
  'claim.released',
  'claim.nudged',
  'tag.applied',
  'tag.removed',
  'branch.created',
  'branch.archived',
  'draft.saved',
  'member.added',
  'member.suspended',
  'member.reactivated',
  'member.removed',
  'project.created',
  'project.archived',
  'project.restored',
] as const;

/**
 * Subscribe to the live event stream for one project. Returns an unsubscribe function.
 * EventSource reconnects by itself; the token goes in the query because EventSource cannot set headers.
 */
export function subscribeProject(projectId: string, onEvent: (e: MpEvent) => void): () => void {
  let closed = false;
  let stop: () => void = () => {};

  if (MOCK) {
    loadMock().then((m) => {
      if (!closed) stop = m.mockSubscribe(projectId, onEvent);
    });
    return () => {
      closed = true;
      stop();
    };
  }

  const token = getToken() || '';
  const url = `/api/events?project=${encodeURIComponent(projectId)}&token=${encodeURIComponent(token)}`;
  let es: EventSource | null = null;
  try {
    es = new EventSource(url);
  } catch {
    return () => {};
  }
  const handle = (msg: MessageEvent) => {
    try {
      const data = JSON.parse(msg.data) as MpEvent;
      if (!data.kind && msg.type && msg.type !== 'message') data.kind = msg.type;
      onEvent(data);
    } catch {
      /* ignore malformed events */
    }
  };
  for (const k of EVENT_KINDS) es.addEventListener(k, handle as EventListener);
  es.addEventListener('message', handle as EventListener);
  return () => {
    closed = true;
    es?.close();
  };
}
