import '../../styles/project.css';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Link, NavLink, Outlet, useParams, useSearchParams } from 'react-router-dom';
import { api } from '../../api/client';
import { daemon } from '../../api/daemon';
import { subscribeProject } from '../../api/events';
import type { Branch, MpEvent } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { useDaemon } from '../../lib/daemon';
import { useSession } from '../../lib/session';
import { baseName, firstName, held, plural } from '../../lib/format';
import { ErrorNote, Loading, Menu, Who, useUi } from '../../components/ui';
import NotFound from '../NotFound';
import { Ctx, type Nudge, type ProjectCtx, type Versions } from './context';
import YourWork from './YourWork';
import { setProjectTheme } from '../../lib/theme';

export default function ProjectLayout() {
  const { projectId = '' } = useParams();
  const [params, setParams] = useSearchParams();
  const { me, canAdmin } = useSession();
  const d = useDaemon();
  const { toast } = useUi();
  const [v, setV] = useState<Versions>({ changes: 0, claims: 0, tags: 0, branches: 0, drafts: 0 });
  const bump = useCallback((k: keyof Versions) => setV((x) => ({ ...x, [k]: x[k] + 1 })), []);

  const proj = useAsync(() => api.project(projectId), [projectId, v.changes, v.branches]);
  // The project's look (or its organization's) while it is open.
  const pt = proj.data?.theme;
  useEffect(() => {
    setProjectTheme(pt ? { light: pt.light, dark: pt.dark } : null);
  }, [pt?.light, pt?.dark]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => () => setProjectTheme(null), []);
  const branchesQ = useAsync(() => api.branches(projectId), [projectId, v.branches, v.changes]);
  const claimsQ = useAsync(() => api.claims(projectId), [projectId, v.claims, v.changes]);
  const drafts = useAsync(() => api.drafts(projectId), [projectId, v.drafts]);

  const ws = d.byProject(projectId);
  const statusQ = useAsync(() => daemon.status(ws!.id), [ws?.id, v.changes, v.claims], !!ws);

  const [nudges, setNudges] = useState<Nudge[]>([]);
  const meRef = useRef(me);
  meRef.current = me;
  const pending = useRef<Set<keyof Versions>>(new Set());
  const flush = useRef<number | undefined>(undefined);

  useEffect(() => {
    const queue = (k: keyof Versions) => {
      pending.current.add(k);
      window.clearTimeout(flush.current);
      flush.current = window.setTimeout(() => {
        const ks = [...pending.current];
        pending.current.clear();
        setV((x) => {
          const n = { ...x };
          for (const key of ks) n[key] += 1;
          return n;
        });
      }, 250);
    };
    const off = subscribeProject(projectId, (e: MpEvent) => {
      const k = e.kind;
      if (k.startsWith('change.')) queue('changes');
      else if (k.startsWith('claim.')) queue('claims');
      else if (k.startsWith('tag.')) queue('tags');
      else if (k.startsWith('branch.')) queue('branches');
      else if (k.startsWith('draft.')) queue('drafts');
      const mine = e.actor?.id === meRef.current?.id;
      if (k === 'claim.nudged' && e.data.holder === meRef.current?.id) {
        const path = String(e.data.path || '');
        setNudges((n) => [...n.filter((x) => x.path !== path), { path, by: e.actor?.name || 'Someone', at: e.created_at }]);
        toast(`${firstName(e.actor?.name || 'Someone')} is waiting for ${baseName(path)}.`, 'hold');
      } else if (k === 'change.published' && !mine && e.actor) {
        toast(`${firstName(e.actor.name)} published change ${e.data.change ?? ''}.`);
      }
    });
    return () => {
      off();
      window.clearTimeout(flush.current);
    };
  }, [projectId, toast]);

  const branches = useMemo(() => branchesQ.data || proj.data?.branches_list || [], [branchesQ.data, proj.data]);
  const main = branches.find((b) => !b.parent_id) || branches[0];
  const wanted = params.get('branch') || (ws && ws.project_id === projectId ? ws.branch_id : '');
  const branch: Branch | undefined = branches.find((b) => b.id === wanted) || main;

  const setBranch = useCallback(
    (id: string) => {
      const n = new URLSearchParams(params);
      if (main && id === main.id) n.delete('branch');
      else n.set('branch', id);
      n.delete('prefix');
      setParams(n);
    },
    [params, setParams, main],
  );

  if (proj.error !== undefined && !proj.data) {
    return <div className="boot"><ErrorNote error={proj.error} doing="open this project" retry={proj.reload} /><p><Link className="link" to="/">Back to your projects</Link></p></div>;
  }
  if (!proj.data || !branch || !main) return <div className="boot"><Loading what="Opening the project" /></div>;
  if (!me) return <NotFound />;

  const project = proj.data;
  const claims = claimsQ.data || [];
  const ctx: ProjectCtx = {
    project, branches, branch, main, setBranch, ws, status: statusQ.data, reloadStatus: statusQ.reload, claims, v, bump,
    reloadProject: proj.reload, reloadBranches: branchesQ.reload, nudges,
    dismissNudge: (path) => setNudges((n) => n.filter((x) => x.path !== path)),
  };

  const q = branch.id !== main.id ? `?branch=${branch.id}` : '';
  const others = new Map<string, { name: string; id: string; paths: string[]; since: string }>();
  for (const c of claims) {
    if (c.user.id === me.id) continue;
    const o = others.get(c.user.id) || { name: c.user.name, id: c.user.id, paths: [], since: c.created_at };
    o.paths.push(c.path);
    if (c.created_at < o.since) o.since = c.created_at;
    others.set(c.user.id, o);
  }
  const working = [...others.values()].sort((a, b) => b.since.localeCompare(a.since)).slice(0, 5);
  const live = branches.filter((b) => !b.archived);
  const showWork = !!ws;

  return (
    <Ctx.Provider value={ctx}>
      <div className={`app ${showWork ? 'with-work' : ''}`}>
        <aside className="side" aria-label="Project">
          <Link to="/" className="back">&larr; All projects</Link>
          <div>
            <div className="proj">
              <small>{[project.org.name, project.client?.name, project.production?.name].filter(Boolean).join(' › ')}</small>
              <strong className="el">{project.name}</strong>
              {ws && ws.account_email !== me.email ? (
                <span className="warn">Desktop app uses {ws.account_email}</span>
              ) : (
                <span title={me.email}>As {me.name}</span>
              )}
            </div>
            <div className="menu-wrap">
              <Menu
                label={`Branch ${branch.name}. Switch branch`}
                className="branch"
                button={<><span className="lbl">Branch</span> <b>{branch.name}</b><em>{live.length > 1 ? `${live.length - 1} more` : 'New'}</em></>}
              >
                {(close) => (
                  <>
                    {live.map((b) => (
                      <button key={b.id} role="menuitemradio" aria-checked={b.id === branch.id} onClick={() => { close(); setBranch(b.id); }}>
                        {b.id === branch.id ? <b>{b.name}</b> : b.name}
                        <small>{b.parent_id ? (b.ahead ? `${b.ahead} to merge` : 'Up to date') : 'Main line'}</small>
                      </button>
                    ))}
                    <hr />
                    <Link role="menuitem" to={`/p/${project.id}/branches${q}`} onClick={close}>See all branches</Link>
                  </>
                )}
              </Menu>
            </div>
          </div>
          <nav className="nav" aria-label="Views">
            <NavLink to={`today${q}`}>Today</NavLink>
            <NavLink to={`files${q}`}>Files<em>{project.file_count.toLocaleString()}</em></NavLink>
            <NavLink to={`history${q}`}>History</NavLink>
            <NavLink to={`people${q}`}>People<em>{claims.length || ''}</em></NavLink>
            <NavLink to={`drafts${q}`}>Drafts<em>{drafts.data?.length || ''}</em></NavLink>
            <NavLink to={`branches${q}`}>Branches<em>{live.length}</em></NavLink>
          </nav>
          {working.length > 0 && (
            <div>
              <h3>Working right now</h3>
              <ul>
                {working.map((w) => (
                  <li key={w.id}>
                    <Who id={w.id} name={w.name} />
                    <p>
                      {firstName(w.name)}
                      <small>
                        {baseName(w.paths[0])}
                        {w.paths.length > 1 ? ` and ${w.paths.length - 1} more` : ''}, {held(w.since)}
                      </small>
                    </p>
                  </li>
                ))}
              </ul>
            </div>
          )}
          <div className="conn">
            Connected to {location.host}
            <br />
            {ws
              ? ws.synced_change >= project.head_change
                ? `Up to date with change ${ws.synced_change}`
                : `Your copy is at ${ws.synced_change}, ${plural(project.head_change - ws.synced_change, 'change')} behind`
              : `Latest change ${project.head_change}`}
            {canAdmin && (
              <>
                <br />
                <Link to="/admin">Admin</Link>
              </>
            )}
          </div>
        </aside>
        <main className="main">
          <Outlet />
        </main>
        {showWork && <YourWork />}
      </div>
    </Ctx.Provider>
  );
}
