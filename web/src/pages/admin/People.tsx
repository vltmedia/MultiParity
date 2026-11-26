import { useEffect, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { api } from '../../api/client';
import { useAsync } from '../../lib/useAsync';
import { plural, ROLE_LABEL, since } from '../../lib/format';
import { Empty, ErrorNote, Loading, Who } from '../../components/ui';
import { AdminHead, useAdmin } from './AdminLayout';

export default function AdminPeople() {
  const { scope, orgs, isServerAdmin, scopeName } = useAdmin();
  const [params, setParams] = useSearchParams();
  const status = params.get('status') || '';
  const org = params.get('org') || scope;
  const [q, setQ] = useState(params.get('q') || '');
  const [qLive, setQLive] = useState(q);
  useEffect(() => {
    const t = setTimeout(() => setQLive(q), 250);
    return () => clearTimeout(t);
  }, [q]);
  const list = useAsync(() => api.admin.users({ q: qLive || undefined, org: org || undefined, status: status || undefined }), [qLive, org, status]);

  const set = (k: string, v: string) => {
    const n = new URLSearchParams(params);
    if (v) n.set(k, v);
    else n.delete(k);
    setParams(n, { replace: true });
  };

  return (
    <>
      <AdminHead title="People" sub={`Everyone with an account in ${org ? orgs.find((o) => o.id === org)?.name || scopeName : scopeName}.`}>
        <Link className="btn pri sm" to="/admin/invites">Invite people</Link>
      </AdminHead>
      <div className="view">
        <div className="toolbar">
          <label>
            <span className="sr">Find a person</span>
            <input className="input" type="search" placeholder="Find by name or email" value={q} onChange={(e) => { setQ(e.target.value); set('q', e.target.value); }} />
          </label>
          <div className="seg sm" role="group" aria-label="Status">
            {[['', 'Everyone'], ['active', 'Active'], ['suspended', 'Suspended']].map(([k, l]) => (
              <button key={k} aria-pressed={status === k} onClick={() => set('status', k)}>{l}</button>
            ))}
          </div>
          {(isServerAdmin || orgs.length > 1) && (
            <label>
              <span className="sr">Organization</span>
              <select className="select sm" value={org} onChange={(e) => set('org', e.target.value)}>
                {isServerAdmin && <option value="">All organizations</option>}
                {orgs.map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
              </select>
            </label>
          )}
        </div>
        {list.loading && !list.data && <Loading what="Loading people" />}
        {list.error !== undefined && <ErrorNote error={list.error} doing="load people" retry={list.reload} />}
        {list.data && !list.data.length && <Empty title="Nobody matches">Try a shorter search or another filter.</Empty>}
        {list.data && list.data.length > 0 && (
          <table className="t">
            <thead>
              <tr>
                <th>Name</th>
                <th>Organizations</th>
                <th>Status</th>
                <th className="r">Claims</th>
                <th className="r">Sessions</th>
                <th>Last seen</th>
              </tr>
            </thead>
            <tbody>
              {list.data.map((u) => (
                <tr key={u.id}>
                  <td>
                    <span className="row" style={{ gap: 10, flexWrap: 'nowrap' }}>
                      <Who id={u.id} name={u.name} />
                      <span>
                        <Link className="rowlink" to={`/admin/people/${u.id}`}>{u.name}</Link>
                        {u.is_server_admin && <span className="badge ink" style={{ marginLeft: 8 }}>Server admin</span>}
                        <br />
                        <span className="fine">{u.email}</span>
                      </span>
                    </span>
                  </td>
                  <td className="wrap">
                    {u.orgs.map((o) => (
                      <span key={o.org.id} className="badge" style={{ marginRight: 5 }}>{o.org.name}, {ROLE_LABEL[o.role]}</span>
                    ))}
                    {!u.orgs.length && <span className="mute">None</span>}
                  </td>
                  <td>
                    {u.status === 'suspended' ? <span className="badge bad" title={u.suspended_reason}>Suspended</span> : <span className="badge good">Active</span>}
                  </td>
                  <td className="r">{u.claims || <span className="mute">0</span>}</td>
                  <td className="r">{u.sessions}</td>
                  <td>{since(u.last_seen_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {list.data && list.data.length > 0 && <p className="fine" style={{ marginTop: 10 }}>{plural(list.data.length, 'person', 'people')}.</p>}
      </div>
    </>
  );
}
