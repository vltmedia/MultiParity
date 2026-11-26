import { useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../../api/client';
import { useSession } from '../../lib/session';
import { when } from '../../lib/format';
import { Dialog, Empty, useAction, useUi } from '../../components/ui';
import { AdminHead, useAdmin } from './AdminLayout';

export default function AdminOrgs() {
  const { orgs, isServerAdmin, reloadOrgs, setScope } = useAdmin();
  const { refresh } = useSession();
  const { toast } = useUi();
  const { run, busy } = useAction();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [err, setErr] = useState('');

  async function create() {
    setErr('');
    if (!name.trim()) return setErr('Give the organization a name, like the studio or client it is for.');
    if (slug && !/^[a-z0-9-]+$/.test(slug)) return setErr('The short name can use only lowercase letters, numbers and dashes.');
    const o = await run('create', 'make the organization', () => api.createOrg(name.trim(), slug.trim() || undefined));
    if (!o) return;
    toast(`Made ${o.name}. You are its owner.`);
    setOpen(false);
    setName('');
    setSlug('');
    reloadOrgs();
    refresh();
    setScope(o.id);
  }

  return (
    <>
      <AdminHead title="Organizations" sub="An organization owns projects, members, tags and ignore defaults. A small studio has one.">
        {isServerAdmin && <button className="btn pri sm" onClick={() => setOpen(true)}>New organization</button>}
      </AdminHead>
      <div className="view">
        {!orgs.length && <Empty title="No organizations">Server admins can make the first one.</Empty>}
        {orgs.length > 0 && (
          <table className="t">
            <thead>
              <tr><th>Name</th><th>Short name</th><th className="r">Members</th><th className="r">Projects</th><th>Made</th></tr>
            </thead>
            <tbody>
              {orgs.map((o) => (
                <tr key={o.id}>
                  <td><Link className="rowlink" to={`/admin/orgs/${o.id}`}>{o.name}</Link></td>
                  <td className="mute">{o.slug}</td>
                  <td className="r">{o.member_count ?? ''}</td>
                  <td className="r">{o.project_count ?? ''}</td>
                  <td>{when(o.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      {open && (
        <Dialog title="New organization" onClose={() => setOpen(false)}>
          <label className="field">
            <span>Name</span>
            <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Northlight Games" />
          </label>
          <label className="field">
            <span>Short name, optional</span>
            <input className="input" value={slug} onChange={(e) => setSlug(e.target.value.toLowerCase())} placeholder={name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'northlight'} />
            <small>Used in links and the CLI. Made from the name if you leave it empty.</small>
          </label>
          {err && <div className="notice bad">{err}</div>}
          <div className="actions">
            <button className="btn" onClick={() => setOpen(false)}>Cancel</button>
            <button className="btn pri" onClick={create} disabled={busy === 'create'}>Make organization</button>
          </div>
        </Dialog>
      )}
    </>
  );
}
