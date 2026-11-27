import { useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { api } from '../../api/client';
import type { Invite, Role } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { firstName, plural, ROLE_LABEL, since, until } from '../../lib/format';
import { CopyButton, Empty, ErrorNote, Loading, useAction, useUi } from '../../components/ui';
import { AdminHead, useAdmin } from './AdminLayout';

const ROLES: Role[] = ['member', 'guest', 'admin', 'owner'];
const ROLE_HELP: Record<Role, string> = {
  owner: 'Can do everything in the organization, including managing other owners.',
  admin: 'Can manage people and projects in the organization.',
  member: 'Can see and work on every project in the organization.',
  guest: 'Sees only the projects they are invited to. Good for outsourcers.',
};

export default function AdminInvites() {
  const { scope, orgs } = useAdmin();
  const [params, setParams] = useSearchParams();
  const org = params.get('org') || scope || orgs[0]?.id || '';
  const orgName = orgs.find((o) => o.id === org)?.name || '';
  const list = useAsync(() => api.admin.invites(org), [org], !!org);
  const { confirm, toast } = useUi();
  const { run, busy } = useAction();
  const [mode, setMode] = useState<'email' | 'link'>('email');
  const [emails, setEmails] = useState('');
  const [role, setRole] = useState<Role>('member');
  const [days, setDays] = useState(7);
  const [made, setMade] = useState<Invite[]>([]);
  const [err, setErr] = useState('');

  const setOrg = (id: string) => {
    const n = new URLSearchParams(params);
    n.set('org', id);
    setParams(n, { replace: true });
    setMade([]);
  };

  async function create() {
    setErr('');
    const list2 = mode === 'email' ? emails.split(/[\s,;]+/).map((e) => e.trim()).filter(Boolean) : [];
    if (mode === 'email') {
      if (!list2.length) return setErr('Paste one or more email addresses, separated by commas or new lines.');
      const bad = list2.filter((e) => !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(e));
      if (bad.length) return setErr(`${bad.slice(0, 3).join(', ')} ${bad.length === 1 ? 'does not look like an email address' : 'do not look like email addresses'}. Fix ${bad.length === 1 ? 'it' : 'them'} and try again.`);
    }
    const r = await run('create', 'make the invites', () => api.admin.createInvites(org, { emails: list2.length ? list2 : undefined, role, expires_in_days: days || undefined }));
    if (!r) return;
    setMade(r);
    setEmails('');
    toast(mode === 'email' ? `Made ${plural(r.length, 'invite')}. Send each person their link.` : 'Made an invite link. Anyone with it can join, once.');
    list.reload();
  }

  async function revoke(i: Invite) {
    const ok = await confirm({
      title: 'Revoke this invite?',
      what: <>The link {i.email ? `for ${i.email} ` : ''}stops working now. Nobody has joined with it yet, so nobody loses access.</>,
      confirm: 'Revoke invite',
      danger: true,
    });
    if (!ok) return;
    const r = await run('revoke:' + i.id, 'revoke the invite', () => api.admin.revokeInvite(i.id));
    if (r) {
      toast('Invite revoked.');
      list.reload();
    }
  }

  const state = (i: Invite) =>
    i.accepted_at ? <span className="badge good">Joined {since(i.accepted_at)}</span>
      : i.expires_at && Date.parse(i.expires_at) < Date.now() ? <span className="badge mute">Expired</span>
        : <span className="badge hold">Waiting</span>;

  return (
    <>
      <AdminHead title="Invites" sub="Invite by email or with a link. A link carries the organization, role and an expiry, and sets up the desktop app in one click.">
        {orgs.length > 1 && (
          <label>
            <span className="sr">Organization</span>
            <select className="select sm" value={org} onChange={(e) => setOrg(e.target.value)}>
              {orgs.map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
            </select>
          </label>
        )}
      </AdminHead>
      <div className="view">
        <div className="grid2 inv">
          <div className="panel">
            <h2>Invite people to {orgName}</h2>
            <div className="seg" role="group" aria-label="How to invite">
              <button aria-pressed={mode === 'email'} onClick={() => setMode('email')}>By email</button>
              <button aria-pressed={mode === 'link'} onClick={() => setMode('link')}>By link</button>
            </div>
            {mode === 'email' ? (
              <label className="field">
                <span>Email addresses</span>
                <textarea className="textarea" rows={4} value={emails} onChange={(e) => setEmails(e.target.value)} placeholder={'lena@studio.example\nomar@studio.example'} />
                <small>One per line, or separated by commas. Each person gets their own link, which works only for that address.</small>
              </label>
            ) : (
              <p className="fine">One link that anyone can use once. Handy for a chat message to a freelancer.</p>
            )}
            <label className="field">
              <span>Role</span>
              <select className="select" value={role} onChange={(e) => setRole(e.target.value as Role)}>
                {ROLES.map((r) => <option key={r} value={r}>{ROLE_LABEL[r]}</option>)}
              </select>
              <small>{ROLE_HELP[role]}</small>
            </label>
            <label className="field">
              <span>Link stops working after</span>
              <select className="select" value={days} onChange={(e) => setDays(Number(e.target.value))}>
                <option value={1}>1 day</option>
                <option value={7}>7 days</option>
                <option value={14}>14 days</option>
                <option value={30}>30 days</option>
                <option value={0}>Never</option>
              </select>
            </label>
            {err && <div className="notice bad">{err}</div>}
            <button className="btn pri" onClick={create} disabled={!org || busy === 'create'}>
              {mode === 'email' ? 'Make invites' : 'Make invite link'}
            </button>
            {made.length > 0 && (
              <div className="result good">
                <b>{made.length === 1 ? 'Here is the link' : `Here are ${made.length} links`}</b>
                <p className="fine">
                  {made.every((i) => i.emailed)
                    ? `Emailed to ${made.length === 1 ? made[0].email : `all ${made.length}`}. You can copy the links too.`
                    : made.some((i) => i.emailed)
                      ? 'The ones with an address were emailed. Copy the others and send them.'
                      : 'This server does not send email. Copy each link and send it.'}
                </p>
                <ul className="links">
                  {made.map((i) => (
                    <li key={i.id}>
                      <span style={{ minWidth: 0 }}>{i.email || 'Anyone'}</span>
                      <code title={i.url}>{i.url}</code>
                      {i.emailed && <span className="fine">Emailed</span>}
                      <CopyButton text={i.url} />
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
          <section className="section">
            <h2>Invites for {orgName}</h2>
            {list.loading && !list.data && <Loading what="Loading invites" />}
            {list.error !== undefined && <ErrorNote error={list.error} doing="load invites" retry={list.reload} />}
            {list.data && !list.data.length && <Empty title="No invites yet">Invites you make show up here until they are used or revoked.</Empty>}
            {list.data && list.data.length > 0 && (
              <table className="t">
                <thead><tr><th>For</th><th>Role</th><th>State</th><th>Expires</th><th /></tr></thead>
                <tbody>
                  {list.data.map((i) => (
                    <tr key={i.id}>
                      <td className="wrap">
                        {i.email || 'Anyone with the link'}
                        <br />
                        <span className="fine">From {firstName(i.created_by.name)}, {since(i.created_at)}</span>
                      </td>
                      <td>{ROLE_LABEL[i.role]}</td>
                      <td>{state(i)}</td>
                      <td>{i.accepted_at ? '' : until(i.expires_at)}</td>
                      <td className="r">
                        {!i.accepted_at && (
                          <span className="row" style={{ justifyContent: 'flex-end', flexWrap: 'nowrap' }}>
                            <CopyButton text={i.url} />
                            <button className="btn sm danger" disabled={busy === 'revoke:' + i.id} onClick={() => revoke(i)}>Revoke</button>
                          </span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>
        </div>
      </div>
    </>
  );
}
