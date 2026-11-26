import '../../styles/admin.css';
import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { Link, NavLink, Outlet } from 'react-router-dom';
import { api } from '../../api/client';
import type { Org } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { useSession } from '../../lib/session';
import { firstName } from '../../lib/format';
import { ErrorNote, Loading } from '../../components/ui';

type AdminCtx = {
  /** '' means the whole server (server admins only) */
  scope: string;
  setScope: (id: string) => void;
  /** organizations I can administer */
  orgs: Org[];
  isServerAdmin: boolean;
  scopeName: string;
  reloadOrgs: () => void;
};

const Ctx = createContext<AdminCtx | null>(null);
export function useAdmin() {
  const v = useContext(Ctx);
  if (!v) throw new Error('useAdmin outside AdminLayout');
  return v;
}

const KEY = 'mp.admin.scope';

export default function AdminLayout() {
  const { me } = useSession();
  const orgsQ = useAsync(() => api.orgs(), []);
  const isServerAdmin = !!me?.is_server_admin;
  const orgs = useMemo(() => {
    const all = orgsQ.data || [];
    if (isServerAdmin) return all;
    const mine = new Set(me?.orgs.filter((o) => o.role === 'owner' || o.role === 'admin').map((o) => o.org.id));
    return all.filter((o) => mine.has(o.id));
  }, [orgsQ.data, isServerAdmin, me]);
  const [scope, setScopeState] = useState<string>(() => {
    try {
      return sessionStorage.getItem(KEY) || '';
    } catch {
      return '';
    }
  });
  useEffect(() => {
    if (!orgsQ.data) return;
    if (scope && !orgs.some((o) => o.id === scope)) setScopeState(isServerAdmin ? '' : orgs[0]?.id || '');
    else if (!scope && !isServerAdmin && orgs[0]) setScopeState(orgs[0].id);
  }, [orgsQ.data, orgs, scope, isServerAdmin]);
  const setScope = (id: string) => {
    setScopeState(id);
    try {
      sessionStorage.setItem(KEY, id);
    } catch {
      /* ignore */
    }
  };

  if (!me) return null;
  if (orgsQ.error !== undefined && !orgsQ.data) return <div className="boot"><ErrorNote error={orgsQ.error} doing="open the admin area" retry={orgsQ.reload} /></div>;
  if (!orgsQ.data) return <div className="boot"><Loading what="Opening the admin area" /></div>;

  const scopeName = scope ? orgs.find((o) => o.id === scope)?.name || '' : 'the whole server';
  const ctx: AdminCtx = { scope, setScope, orgs, isServerAdmin, scopeName, reloadOrgs: orgsQ.reload };

  return (
    <Ctx.Provider value={ctx}>
      <div className="app">
        <aside className="side" aria-label="Admin">
          <Link to="/" className="back">&larr; All projects</Link>
          <div>
            <div className="logo">MultiParity</div>
            <p className="fine" style={{ color: 'var(--sidemute)', marginTop: 4 }}>Admin</p>
          </div>
          <label className="scope">
            <span>Looking after</span>
            <select className="select sm" value={scope} onChange={(e) => setScope(e.target.value)}>
              {isServerAdmin && <option value="">The whole server</option>}
              {orgs.map((o) => (
                <option key={o.id} value={o.id}>{o.name}</option>
              ))}
            </select>
          </label>
          <nav className="nav" aria-label="Admin sections">
            <NavLink to="/admin" end>Overview</NavLink>
            <NavLink to="/admin/people">People</NavLink>
            <NavLink to="/admin/invites">Invites</NavLink>
            <NavLink to="/admin/orgs">Organizations</NavLink>
            <NavLink to="/admin/projects">Projects</NavLink>
            <NavLink to="/admin/fixing">Fixing</NavLink>
            <NavLink to="/admin/audit">Audit log</NavLink>
          </nav>
          <div className="conn">
            Signed in as {firstName(me.name)}
            <br />
            {isServerAdmin ? 'Server admin' : 'Organization admin'} on {location.host}
          </div>
        </aside>
        <main className="main">
          <Outlet />
        </main>
      </div>
    </Ctx.Provider>
  );
}

export function AdminHead({ title, sub, children }: { title: string; sub?: ReactNode; children?: ReactNode }) {
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
