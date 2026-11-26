import { Link } from 'react-router-dom';
import { api } from '../../api/client';
import { useAsync } from '../../lib/useAsync';
import { plural, size } from '../../lib/format';
import { ErrorNote, Loading } from '../../components/ui';
import { AdminHead, useAdmin } from './AdminLayout';

export default function AdminOverview() {
  const { scope, scopeName } = useAdmin();
  const o = useAsync(() => api.admin.overview(scope || undefined), [scope]);
  const d = o.data;
  return (
    <>
      <AdminHead title="Overview" sub={`How things stand for ${scopeName}.`} />
      <div className="view">
        {o.loading && !d && <Loading what="Counting" />}
        {o.error !== undefined && <ErrorNote error={o.error} doing="load the overview" retry={o.reload} />}
        {d && (
          <>
            <div className="stats">
              <Link className="stat" to="/admin/people"><b>{d.users.toLocaleString()}</b><span>People</span></Link>
              <Link className={`stat ${d.suspended ? 'bad' : ''}`} to="/admin/people?status=suspended"><b>{d.suspended}</b><span>Suspended</span></Link>
              <Link className="stat" to="/admin/orgs"><b>{d.orgs}</b><span>Organizations</span></Link>
              <Link className="stat" to="/admin/projects"><b>{d.projects}</b><span>Projects</span></Link>
              <div className="stat"><b>{size(d.storage_bytes)}</b><span>Stored, after dedupe and compression</span></div>
              <div className="stat"><b>{d.chunks.toLocaleString()}</b><span>Chunks</span></div>
              <Link className="stat" to="/admin/fixing"><b>{d.active_claims}</b><span>Files claimed right now</span></Link>
              <Link className={`stat ${d.stale_claims ? 'warn' : ''}`} to="/admin/fixing"><b>{d.stale_claims}</b><span>Claims that look stuck</span></Link>
            </div>
            <div className="section" style={{ maxWidth: 760 }}>
              <h2>Needs a look</h2>
              {!d.stale_claims && !d.suspended && <p className="mute">Nothing. Claims are moving and nobody is suspended.</p>}
              {d.stale_claims > 0 && (
                <div className="notice acc">
                  <b>{plural(d.stale_claims, 'claim')} older than three days.</b> Someone may have gone on holiday with files claimed.
                  <div className="row"><Link className="btn sm" to="/admin/fixing">Review stuck claims</Link></div>
                </div>
              )}
              {d.suspended > 0 && (
                <div className="notice hold">
                  <b>{plural(d.suspended, 'suspended account')}.</b> Their claims stay so nobody's work is overwritten. Hand them over or release them when you are ready.
                  <div className="row"><Link className="btn sm" to="/admin/people?status=suspended">See suspended people</Link></div>
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </>
  );
}
