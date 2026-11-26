import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { daemon } from '../api/daemon';
import type { Workspace } from '../api/types';

type DaemonValue = {
  /** null while checking */
  available: boolean | null;
  version?: string;
  workspaces: Workspace[];
  byProject: (projectId: string) => Workspace | undefined;
  refresh: () => Promise<void>;
};

const Ctx = createContext<DaemonValue | null>(null);

export function DaemonProvider({ children }: { children: ReactNode }) {
  const [available, setAvailable] = useState<boolean | null>(null);
  const [version, setVersion] = useState<string>();
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);

  const refresh = useCallback(async () => {
    try {
      const h = await daemon.health();
      setVersion(h.version);
      const ws = await daemon.workspaces();
      setWorkspaces(ws);
      setAvailable(true);
    } catch {
      setAvailable(false);
      setWorkspaces([]);
    }
  }, []);

  useEffect(() => {
    refresh();
    // check again when the window comes back, in case the app was started or quit meanwhile
    const onFocus = () => refresh();
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, [refresh]);

  const value = useMemo<DaemonValue>(
    () => ({
      available,
      version,
      workspaces,
      byProject: (id: string) => workspaces.find((w) => w.project_id === id),
      refresh,
    }),
    [available, version, workspaces, refresh],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useDaemon() {
  const v = useContext(Ctx);
  if (!v) throw new Error('useDaemon outside DaemonProvider');
  return v;
}
