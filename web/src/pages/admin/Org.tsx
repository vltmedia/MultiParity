import { Link, useParams } from 'react-router-dom';
import { api } from '../../api/client';
import type { Member, Role } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { useSession } from '../../lib/session';
import { firstName, plural, ROLE_LABEL, since, until } from '../../lib/format';
import { Empty, ErrorNote, Loading, Who, useAction, useUi } from '../../components/ui';
import NotFound from '../NotFound';
import { AdminHead, useAdmin } from './AdminLayout';

const ROLES: Role[] = ['owner', 'admin', 'member', 'guest'];

export default function AdminOrg() {
  const { orgId = '' } = useParams();
  const { orgs } = useAdmin();
  const { me } = useSession();
  const org = orgs.find((o) => o.id === orgId);
  const members = useAsync(() => api.admin.members(orgId), [orgId], !!org);
  const { confirm, toast } = useUi();
  const { run, busy } = useAction();
  if (!org) return <NotFound reason="You do not administer this organization, or it does not exist." />;

  async function setRole(m: Member, role: Role) {
    if (m.role === 'owner' && role !== 'owner') {
      const ok = await confirm({
        title: `Make ${firstName(m.user.name)} ${ROLE_LABEL[role]}?`,
        what: <>{m.user.name} stops being an owner of {org!.name}. {m.user.id === me?.id ? 'You may lose the ability to manage owners yourself.' : ''}</>,
        confirm: `Make ${ROLE_LABEL[role]}`,
      });
      if (!ok) return;
    }
    const r = await run('role:' + m.user.id, 'change the role', () => api.admin.updateMember(org!.id, m.user.id, { role }));
    if (r) {
      toast(`${firstName(m.user.name)} is now ${ROLE_LABEL[role]}.`);
      members.reload();
    }
  }
  async function revoke(m: Member) {
    const ok = await confirm({
      title: `Remove ${firstName(m.user.name)} from ${org!.name}?`,
      what: <>{m.user.name} loses access to every {org!.name} project at once. Everything they published stays, still with their name on it. Their claims stay until you release them in Fixing.</>,
      confirm: `Remove from ${org!.name}`,
      danger: true,
    });
    if (!ok) return;
    const r = await run('revoke:' + m.user.id, `remove ${firstName(m.user.name)}`, () => api.admin.removeMember(org!.id, m.user.id));
    if (r) {
      toast(`${firstName(m.user.name)} is no longer in ${org!.name}.`);
      members.reload();
    }
  }

  return (
    <>
      <AdminHead title={org.name} sub={members.data ? `${plural(members.data.length, 'member')}. Change roles here; open a person to suspend, reset or sign them out.` : undefined}>
        <Link className="btn sm" to={`/admin/invites?org=${org.id}`}>Invite people</Link>
        <Link className="btn sm" to="/admin/projects">Projects</Link>
      </AdminHead>
      <div className="view">
        {members.loading && !members.data && <Loading what="Loading members" />}
        {members.error !== undefined && <ErrorNote error={members.error} doing="load the members" retry={members.reload} />}
        {members.data && !members.data.length && <Empty title="No members">Invite the first people.</Empty>}
        {members.data && members.data.length > 0 && (
          <table className="t">
            <thead><tr><th>Name</th><th>Status</th><th>Role</th><th>Access</th><th>Last seen</th><th /></tr></thead>
            <tbody>
              {members.data.map((m) => (
                <tr key={m.user.id}>
                  <td>
                    <span className="row" style={{ gap: 10, flexWrap: 'nowrap' }}>
                      <Who id={m.user.id} name={m.user.name} />
                      <span>
                        <Link className="rowlink" to={`/admin/people/${m.user.id}`}>{m.user.name}</Link>
                        <br />
                        <span className="fine">{m.user.email}</span>
                      </span>
                    </span>
                  </td>
                  <td>{m.user.status === 'suspended' ? <span className="badge bad">Suspended</span> : <span className="badge good">Active</span>}</td>
                  <td>
                    <select className="select sm" aria-label={`Role of ${m.user.name}`} value={m.role} disabled={busy === 'role:' + m.user.id} onChange={(e) => setRole(m, e.target.value as Role)}>
                      {ROLES.map((r) => <option key={r} value={r}>{ROLE_LABEL[r]}</option>)}
                    </select>
                  </td>
                  <td>{m.expires_at ? until(m.expires_at) : <span className="mute">No end date</span>}</td>
                  <td>{since(m.user.last_seen_at)}</td>
                  <td className="r">
                    {m.user.id !== me?.id && <button className="btn sm danger" disabled={busy === 'revoke:' + m.user.id} onClick={() => revoke(m)}>Remove</button>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </>
  );
}
