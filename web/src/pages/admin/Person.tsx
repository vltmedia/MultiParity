import { useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { api } from '../../api/client';
import type { Role } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { useSession } from '../../lib/session';
import { baseName, dateTime, firstName, plural, ROLE_LABEL, since } from '../../lib/format';
import { Dialog, ErrorNote, Loading, useAction, useUi } from '../../components/ui';
import { AdminHead, useAdmin } from './AdminLayout';

const ROLES: Role[] = ['owner', 'admin', 'member', 'guest'];

export default function AdminPerson() {
  const { userId = '' } = useParams();
  const { me } = useSession();
  const { isServerAdmin, orgs } = useAdmin();
  const navigate = useNavigate();
  const u = useAsync(() => api.admin.user(userId), [userId]);
  const { confirm, toast } = useUi();
  const { run, busy } = useAction();
  const [suspendOpen, setSuspendOpen] = useState(false);
  const [reason, setReason] = useState('');
  const [pwOpen, setPwOpen] = useState(false);
  const [pw, setPw] = useState('');
  const [pw2, setPw2] = useState('');
  const [pwErr, setPwErr] = useState('');
  const [delOpen, setDelOpen] = useState(false);
  const [sel, setSel] = useState<Set<string>>(new Set());

  if (u.loading && !u.data) return <div className="view" style={{ paddingTop: 28 }}><Loading what="Loading this person" /></div>;
  if (u.error !== undefined && !u.data) return <div className="view" style={{ paddingTop: 28 }}><ErrorNote error={u.error} doing="load this person" retry={u.reload} /></div>;
  const p = u.data!;
  const self = p.id === me?.id;
  const first = firstName(p.name);
  const adminOf = (orgId: string) => isServerAdmin || orgs.some((o) => o.id === orgId);
  const key = (c: { project: { id: string }; path: string }) => `${c.project.id}:${c.path}`;

  async function suspend() {
    const r = await run('suspend', `suspend ${first}`, () => api.admin.suspend(p.id, reason.trim()));
    if (r) {
      toast(`${first} is suspended. Their claims stay in place.`);
      setSuspendOpen(false);
      setReason('');
      u.reload();
    }
  }
  async function reactivate() {
    const ok = await confirm({
      title: `Reactivate ${first}?`,
      what: <>{p.name} can sign in and sync again straight away. They will need to sign in again on each device.</>,
      confirm: `Reactivate ${first}`,
    });
    if (!ok) return;
    const r = await run('reactivate', `reactivate ${first}`, () => api.admin.reactivate(p.id));
    if (r) {
      toast(`${first} is active again.`);
      u.reload();
    }
  }
  async function signOutAll() {
    const ok = await confirm({
      title: `Sign ${first} out everywhere?`,
      what: <>All {plural(p.sessions.length, 'session')} and API tokens for {p.name} stop working now, including the desktop app and any CI agents using their tokens. Their claims and unpublished work stay as they are.</>,
      confirm: 'Sign out everywhere',
      danger: true,
    });
    if (!ok) return;
    const r = await run('signout', `sign ${first} out`, () => api.admin.signOut(p.id));
    if (r) {
      toast(`Signed ${first} out of ${plural(r.revoked, 'session')}.`);
      u.reload();
    }
  }
  async function setPassword() {
    setPwErr('');
    if (pw.length < 8) return setPwErr('Choose a password with at least 8 characters.');
    if (pw !== pw2) return setPwErr('The two passwords are not the same.');
    const r = await run('pw', 'set the password', () => api.admin.setPassword(p.id, pw));
    if (r) {
      toast(`New password set for ${first}. Tell them in person or over a private channel.`);
      setPwOpen(false);
      setPw('');
      setPw2('');
    }
  }
  async function changeRole(orgId: string, orgName: string, role: Role) {
    const r = await run('role', 'change the role', () => api.admin.updateMember(orgId, p.id, { role }));
    if (r) {
      toast(`${first} is now ${ROLE_LABEL[role]} in ${orgName}.`);
      u.reload();
    }
  }
  async function revoke(orgId: string, orgName: string) {
    const ok = await confirm({
      title: `Remove ${first} from ${orgName}?`,
      what: <>{p.name} loses access to every {orgName} project at once. Everything they published stays, still with their name on it. Their claims in {orgName} stay until you release them.</>,
      confirm: `Remove from ${orgName}`,
      danger: true,
    });
    if (!ok) return;
    const r = await run('revoke', `remove ${first} from ${orgName}`, () => api.admin.removeMember(orgId, p.id));
    if (r) {
      toast(`${first} is no longer in ${orgName}.`);
      u.reload();
    }
  }
  async function releaseClaims(list: typeof p.claims) {
    const ok = await confirm({
      title: `Release ${plural(list.length, 'claim')} held by ${first}?`,
      what: <>Anyone can claim {list.length === 1 ? baseName(list[0].path) : 'these files'} straight away. {first} is told. If they had edits, those stay on their computer, but they may not be able to publish them.</>,
      confirm: `Release ${plural(list.length, 'claim')}`,
      danger: true,
    });
    if (!ok) return;
    const r = await run('release', 'release the claims', () => api.admin.releaseClaims(list.map((c) => ({ project: c.project.id, branch: c.branch_id, path: c.path }))));
    if (r) {
      toast(`Released ${plural(r.released, 'claim')}.`);
      setSel(new Set());
      u.reload();
    }
  }

  return (
    <>
      <AdminHead
        title={p.name}
        sub={<>{p.email}{p.is_server_admin && ' · Server admin'}{self && ' · This is you'}</>}
      >
        {p.status === 'suspended' ? <span className="badge bad">Suspended</span> : <span className="badge good">Active</span>}
      </AdminHead>
      <div className="view">
        {p.status === 'suspended' && (
          <div className="notice bad" style={{ marginBottom: 20, maxWidth: 760 }}>
            <b>{first} cannot sign in or sync.</b> {p.suspended_reason ? `Reason: ${p.suspended_reason}` : 'No reason was given.'} Their claims are kept so nobody's work is overwritten.
          </div>
        )}
        <div className="grid2">
          <div className="stack">
            <section className="section">
              <h2>Account</h2>
              <dl className="dl">
                <dt>Joined</dt><dd>{dateTime(p.created_at)}</dd>
                <dt>Last seen</dt><dd>{since(p.last_seen_at)}</dd>
                <dt>Claims</dt><dd>{p.claims.length}</dd>
                <dt>Sessions</dt><dd>{p.sessions.length}</dd>
              </dl>
              <div className="row">
                {p.status === 'active' ? (
                  <button className="btn sm" disabled={self || !!busy} onClick={() => setSuspendOpen(true)}>Suspend {first}</button>
                ) : (
                  <button className="btn sm pri" disabled={!!busy} onClick={reactivate}>Reactivate {first}</button>
                )}
                <button className="btn sm" disabled={!!busy} onClick={() => setPwOpen(true)}>Set a new password</button>
                <button className="btn sm" disabled={!p.sessions.length || !!busy} onClick={signOutAll}>Sign out everywhere</button>
              </div>
              {self && <p className="fine">You cannot suspend your own account.</p>}
            </section>

            <section className="section">
              <h2>Organizations</h2>
              {!p.orgs.length && <p className="mute">{first} is not in any organization.</p>}
              {p.orgs.length > 0 && (
                <table className="t">
                  <thead><tr><th>Organization</th><th>Role</th><th /></tr></thead>
                  <tbody>
                    {p.orgs.map((o) => (
                      <tr key={o.org.id}>
                        <td><Link className="rowlink" to={`/admin/orgs/${o.org.id}`}>{o.org.name}</Link></td>
                        <td>
                          {adminOf(o.org.id) ? (
                            <select className="select sm" aria-label={`Role in ${o.org.name}`} value={o.role} disabled={!!busy} onChange={(e) => changeRole(o.org.id, o.org.name, e.target.value as Role)}>
                              {ROLES.map((r) => <option key={r} value={r}>{ROLE_LABEL[r]}</option>)}
                            </select>
                          ) : (
                            ROLE_LABEL[o.role]
                          )}
                        </td>
                        <td className="r">
                          {adminOf(o.org.id) && !self && <button className="link fine" onClick={() => revoke(o.org.id, o.org.name)}>Remove from {o.org.name}</button>}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
              <p className="fine">Guests see only the projects they are invited to. Owners and admins can manage people and projects in their organization.</p>
            </section>

            <section className="section">
              <h2>Sessions and tokens</h2>
              {!p.sessions.length && <p className="mute">Not signed in anywhere.</p>}
              {p.sessions.length > 0 && (
                <table className="t">
                  <thead><tr><th>Where</th><th>Started</th><th>Last used</th></tr></thead>
                  <tbody>
                    {p.sessions.map((s) => (
                      <tr key={s.id}>
                        <td>{s.kind === 'token' ? <><span className="badge">API token</span> {s.label}</> : s.device || s.label || 'Session'}</td>
                        <td>{dateTime(s.created_at)}</td>
                        <td>{since(s.last_used_at)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </section>
          </div>

          <div className="stack">
            <section className="section">
              <h2>Files {first} has claimed</h2>
              <p>Releasing tells {first}. Nobody's published work is affected.</p>
              {!p.claims.length && <p className="mute">Nothing claimed.</p>}
              {p.claims.length > 0 && (
                <>
                  <table className="t">
                    <thead>
                      <tr>
                        <th className="c">
                          <input type="checkbox" aria-label="Select all claims" checked={sel.size === p.claims.length} onChange={() => setSel(sel.size === p.claims.length ? new Set() : new Set(p.claims.map(key)))} />
                        </th>
                        <th>File</th>
                        <th>Project</th>
                        <th>Since</th>
                      </tr>
                    </thead>
                    <tbody>
                      {p.claims.map((c) => (
                        <tr key={key(c)} className={sel.has(key(c)) ? 'sel' : ''}>
                          <td className="c">
                            <input type="checkbox" aria-label={`Select ${baseName(c.path)}`} checked={sel.has(key(c))} onChange={() => { const n = new Set(sel); if (n.has(key(c))) n.delete(key(c)); else n.add(key(c)); setSel(n); }} />
                          </td>
                          <td className="name" title={c.path}>{baseName(c.path)}</td>
                          <td><Link className="rowlink" to={`/p/${c.project.id}/people`}>{c.project.name}</Link></td>
                          <td>{since(c.created_at)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  <div className="row">
                    <button className="btn sm" disabled={!sel.size || !!busy} onClick={() => releaseClaims(p.claims.filter((c) => sel.has(key(c))))}>
                      Release {sel.size ? plural(sel.size, 'claim') : 'selected'}
                    </button>
                  </div>
                </>
              )}
            </section>

            {isServerAdmin && !self && (
              <section className="danger-zone">
                <h3>Delete this account</h3>
                <p className="fine">For people who have left for good. Their published changes stay in history with their name. You can hand their claims and drafts to someone first.</p>
                <button className="btn sm danger" style={{ alignSelf: 'flex-start' }} onClick={() => setDelOpen(true)}>Delete {first}'s account</button>
              </section>
            )}
          </div>
        </div>
      </div>

      {suspendOpen && (
        <Dialog title={`Suspend ${first}?`} onClose={() => setSuspendOpen(false)}>
          <div className="what">
            {p.name} is signed out of every device now and cannot sign in or sync. Their {plural(p.claims.length, 'claim')} stay in place so nobody's work is overwritten. You can reactivate them at any time.
          </div>
          <label className="field">
            <span>Reason, optional</span>
            <input className="input" value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Left the studio, handing over next week" />
            <small>Shown to other admins on this page.</small>
          </label>
          <div className="actions">
            <button className="btn" onClick={() => setSuspendOpen(false)}>Cancel</button>
            <button className="btn danger solid" onClick={suspend} disabled={busy === 'suspend'}>Suspend {first}</button>
          </div>
        </Dialog>
      )}
      {pwOpen && (
        <Dialog title={`Set a new password for ${first}`} onClose={() => setPwOpen(false)}>
          <div className="what calm">The old password stops working and {first} is signed out everywhere. Give them the new one in person or over a private channel.</div>
          <label className="field">
            <span>New password</span>
            <input className="input" type="password" autoComplete="new-password" value={pw} onChange={(e) => setPw(e.target.value)} />
            <small>At least 8 characters.</small>
          </label>
          <label className="field">
            <span>New password again</span>
            <input className="input" type="password" autoComplete="new-password" value={pw2} onChange={(e) => setPw2(e.target.value)} />
          </label>
          {pwErr && <div className="notice bad">{pwErr}</div>}
          <div className="actions">
            <button className="btn" onClick={() => setPwOpen(false)}>Cancel</button>
            <button className="btn pri" onClick={setPassword} disabled={busy === 'pw'}>Set password</button>
          </div>
        </Dialog>
      )}
      {delOpen && <DeleteDialog user={p} onClose={() => setDelOpen(false)} onDone={() => { toast(`Deleted ${p.name}'s account.`); navigate('/admin/people'); }} />}
    </>
  );
}

function DeleteDialog({ user, onClose, onDone }: { user: { id: string; name: string; claims: unknown[] }; onClose: () => void; onDone: () => void }) {
  const people = useAsync(() => api.admin.users({ status: 'active' }), []);
  const { run, busy } = useAction();
  const [typed, setTyped] = useState('');
  const [to, setTo] = useState('');
  const first = firstName(user.name);
  const toName = people.data?.find((x) => x.id === to)?.name;
  async function go() {
    const r = await run('delete', `delete ${first}'s account`, () => api.admin.deleteUser(user.id, typed.trim(), to || undefined));
    if (r) onDone();
  }
  return (
    <Dialog title={`Delete ${first}'s account?`} onClose={onClose}>
      <div className="what">
        The account {user.name} is removed from this server and cannot sign in again. Their published changes stay in history under their name.{' '}
        {to ? `Their ${plural(user.claims.length, 'claim')} and drafts go to ${toName}.` : `Their ${plural(user.claims.length, 'claim')} are released and their drafts are deleted.`} This cannot be undone.
      </div>
      <label className="field">
        <span>Hand claims and drafts to</span>
        <select className="select" value={to} onChange={(e) => setTo(e.target.value)}>
          <option value="">Nobody, release and delete them</option>
          {people.data?.filter((x) => x.id !== user.id).map((x) => <option key={x.id} value={x.id}>{x.name}</option>)}
        </select>
      </label>
      <label className="field">
        <span>Type {user.name} to confirm</span>
        <input className="input" value={typed} onChange={(e) => setTyped(e.target.value)} autoComplete="off" spellCheck={false} />
      </label>
      <div className="actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn danger solid" disabled={typed.trim() !== user.name || busy === 'delete'} onClick={go}>Delete account</button>
      </div>
      <p className="fine">Want to keep the account but stop access? Suspend instead; it is reversible.</p>
    </Dialog>
  );
}
