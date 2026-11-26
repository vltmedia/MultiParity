import { createContext, useCallback, useContext, type ReactNode } from 'react';
import type { Branch, Claim, LocalStatus, Project, Workspace } from '../../api/types';

export type Versions = { changes: number; claims: number; tags: number; branches: number; drafts: number };

export type Nudge = { path: string; by: string; at: string };

export type ProjectCtx = {
  project: Project;
  branches: Branch[];
  branch: Branch;
  main: Branch;
  setBranch: (id: string) => void;
  /** This computer's folder for the project, when the daemon has one. */
  ws?: Workspace;
  status?: LocalStatus;
  reloadStatus: () => void;
  claims: Claim[];
  v: Versions;
  bump: (k: keyof Versions) => void;
  reloadProject: () => void;
  reloadBranches: () => void;
  nudges: Nudge[];
  dismissNudge: (path: string) => void;
};

export const Ctx = createContext<ProjectCtx | null>(null);

export function useProject(): ProjectCtx {
  const v = useContext(Ctx);
  if (!v) throw new Error('useProject outside ProjectLayout');
  return v;
}

/** Links inside a project keep the chosen branch. */
export function useHref() {
  const { project, branch, main } = useProject();
  return useCallback(
    (view: string, extra?: Record<string, string>) => {
      const q = new URLSearchParams();
      if (branch.id !== main.id) q.set('branch', branch.id);
      if (extra) for (const [k, val] of Object.entries(extra)) if (val) q.set(k, val);
      const s = q.toString();
      return `/p/${project.id}/${view}${s ? `?${s}` : ''}`;
    },
    [project.id, branch.id, main.id],
  );
}

export function ViewHead({ title, sub, children }: { title: string; sub?: ReactNode; children?: ReactNode }) {
  return (
    <header className="head">
      <div>
        <h1>{title}</h1>
        {sub && <p>{sub}</p>}
      </div>
      {children && <div className="tools">{children}</div>}
    </header>
  );
}
