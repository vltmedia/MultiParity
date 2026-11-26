import { useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../../api/client';
import type { CheckpointSetting, ProjectSummary } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { plural, since, size } from '../../lib/format';
import { Dialog, Empty, ErrorNote, Loading, useAction, useUi } from '../../components/ui';
import { AdminHead, useAdmin } from './AdminLayout';

const KEEP: Array<[CheckpointSetting, string]> = [
  ['auto', 'Automatic (on the server from 20 GB)'],
  ['server', 'On the server'],
  ['local', 'On each computer'],
];
const KEPT = { server: 'on the server', local: 'on each computer' };

type Edit = { kind: 'rename' | 'move'; p: ProjectSummary } | { kind: 'create' };

export default function AdminProjects() {
  const { scope, orgs, scopeName } = useAdmin();
  const list = useAsync(() => api.admin.projects(scope || undefined), [scope]);
  const { confirm, toast } = useUi();
  const { run, busy } = useAction();
  const [edit, setEdit] = useState<Edit | null>(null);
  const [name, setName] = useState('');
  const [org, setOrg] = useState('');
  const [err, setErr] = useState('');
  const [showArchived, setShowArchived] = useState(false);

  const open = (e: Edit) => {
    setErr('');
    setEdit(e);
    setName(e.kind === 'rename' ? e.p.name : '');
    setOrg(e.kind === 'move' ? '' : e.kind === 'create' ? scope || orgs[0]?.id || '' : '');
  };

  async function save() {
    if (!edit) return;
    setErr('');
    if (edit.kind === 'create') {
      if (!name.trim()) return setErr('Give the project a name.');
      if (!org) return setErr('Choose which organization owns it.');
      const r = await run('save', 'make the project', () => api.createProject(org, name.trim()));
      if (r) toast(`Made ${r.name}. It starts with one branch, Main.`);
      if (!r) return;
    } else if (edit.kind === 'rename') {
      if (!name.trim()) return setErr('The project needs a name.');
      if (name.trim() === edit.p.name) return setEdit(null);
      const r = await run('save', 'rename the project', () => api.admin.updateProject(edit.p.id, { name: name.trim() }));
      if (!r) return;
      toast(`Renamed ${edit.p.name} to ${r.name}. Folders on people's computers keep their names.`);
    } else {
      if (!org) return setErr('Choose the organization to move it to.');
      const target = orgs.find((o) => o.id === org)!;
      const ok = await confirm({
        title: `Move ${edit.p.name} to ${target.name}?`,
        what: <>Members of {edit.p.org.name} who are not in {target.name} lose access to {edit.p.name}. Members of {target.name} gain it. History, claims and files stay exactly as they are.</>,
        confirm: `Move to ${target.name}`,
        danger: true,
      });
      if (!ok) return;
      const r = await run('save', 'move the project', () => api.admin.updateProject(edit.p.id, { org }));
      if (!r) return;
      toast(`${edit.p.name} now belongs to ${target.name}.`);
    }
    setEdit(null);
    list.reload();
  }

  async function archive(p: ProjectSummary) {
    const ok = await confirm({
      title: `Archive ${p.name}?`,
      what: <>{p.name} is hidden from everyone's Projects page and nobody can publish to it. All {plural(p.file_count, 'file')} and its history are kept, and you can restore it at any time.{p.claims ? ` Its ${plural(p.claims, 'claim')} stay in place.` : ''}</>,
      confirm: 'Archive project',
      danger: true,
    });
    if (!ok) return;
    const r = await run('arch:' + p.id, `archive ${p.name}`, () => api.admin.archiveProject(p.id));
    if (r) {
      toast(`Archived ${p.name}.`);
      list.reload();
    }
  }
  async function restore(p: ProjectSummary) {
    const r = await run('arch:' + p.id, `restore ${p.name}`, () => api.admin.restoreProject(p.id));
    if (r) {
      toast(`Restored ${p.name}. It is back on people's Projects page.`);
      list.reload();
    }
  }

  async function keep(p: ProjectSummary, setting: CheckpointSetting) {
    if (setting === p.checkpoint_setting) return;
    const r = await run('keep:' + p.id, `change where ${p.name} keeps checkpoints`, () => api.admin.updateProject(p.id, { checkpoints: setting }));
    if (r) {
      toast(
        r.checkpoint_setting === 'auto'
          ? `${p.name} now keeps checkpoints on the server from 20 GB. For now they are kept ${KEPT[r.checkpoint_storage]}.`
          : `${p.name} now keeps checkpoints ${KEPT[r.checkpoint_storage]}.`,
      );
      list.reload();
    }
  }

  async function automatic(p: ProjectSummary, on: boolean) {
    const r = await run('auto:' + p.id, `change automatic checkpoints for ${p.name}`, () => api.admin.updateProject(p.id, { auto_checkpoints: on }));
    if (r) {
      toast(on ? `${p.name} now takes checkpoints automatically while people work.` : `${p.name} takes checkpoints only when someone saves one.`);
      list.reload();
    }
  }

  const rows = (list.data || []).filter((p) => showArchived || !p.archived);
  const archivedCount = (list.data || []).filter((p) => p.archived).length;

  return (
    <>
      <AdminHead title="Projects" sub={`Every project in ${scopeName}, including archived ones.`}>
        <button className="btn pri sm" onClick={() => open({ kind: 'create' })}>New project</button>
      </AdminHead>
      <div className="view">
        <div className="toolbar">
          {archivedCount > 0 && (
            <label className="check-label">
              <input type="checkbox" checked={showArchived} onChange={(e) => setShowArchived(e.target.checked)} /> Show {plural(archivedCount, 'archived project')}
            </label>
          )}
        </div>
        {list.loading && !list.data && <Loading what="Loading projects" />}
        {list.error !== undefined && <ErrorNote error={list.error} doing="load projects" retry={list.reload} />}
        {list.data && !rows.length && <Empty title="No projects">Make the first one with New project.</Empty>}
        {rows.length > 0 && (
          <table className="t">
            <thead>
              <tr><th>Project</th><th>Organization</th><th className="r">Size</th><th className="r">Files</th><th className="r">Change</th><th>Last change</th><th>Keep checkpoints</th><th /></tr>
            </thead>
            <tbody>
              {rows.map((p) => (
                <tr key={p.id} style={p.archived ? { opacity: 0.65 } : undefined}>
                  <td>
                    {p.archived ? p.name : <Link className="rowlink" to={`/p/${p.id}`}>{p.name}</Link>}
                    {p.archived && <span className="badge mute" style={{ marginLeft: 8 }}>Archived</span>}
                  </td>
                  <td>{p.org.name}</td>
                  <td className="r">{size(p.size)}</td>
                  <td className="r">{p.file_count.toLocaleString()}</td>
                  <td className="r">{p.head_change}</td>
                  <td>{p.last_change ? since(p.last_change.created_at) : <span className="mute">Never</span>}</td>
                  <td>
                    <select className="select sm" aria-label={`Keep checkpoints for ${p.name}`} value={p.checkpoint_setting} disabled={busy === 'keep:' + p.id} onChange={(e) => keep(p, e.target.value as CheckpointSetting)}>
                      {KEEP.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
                    </select>
                    {p.checkpoint_setting === 'auto' && <><br /><span className="fine">Now {KEPT[p.checkpoint_storage]}</span></>}
                    <br />
                    <label className="check-label fine" title="Off by default: most art tools already autosave, and copying big files every few minutes costs disk, battery and upload.">
                      <input type="checkbox" checked={p.auto_checkpoints} disabled={busy === 'auto:' + p.id} onChange={(e) => automatic(p, e.target.checked)} />
                      Also take them automatically
                    </label>
                  </td>
                  <td className="r">
                    <span className="row" style={{ justifyContent: 'flex-end', flexWrap: 'nowrap' }}>
                      <button className="btn sm" onClick={() => open({ kind: 'rename', p })}>Rename</button>
                      {orgs.length > 1 && <button className="btn sm" onClick={() => open({ kind: 'move', p })}>Move</button>}
                      {p.archived ? (
                        <button className="btn sm" disabled={busy === 'arch:' + p.id} onClick={() => restore(p)}>Restore</button>
                      ) : (
                        <button className="btn sm danger" disabled={busy === 'arch:' + p.id} onClick={() => archive(p)}>Archive</button>
                      )}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      {edit && (
        <Dialog
          title={edit.kind === 'create' ? 'New project' : edit.kind === 'rename' ? `Rename ${edit.p.name}` : `Move ${edit.p.name} to another organization`}
          onClose={() => setEdit(null)}
        >
          {edit.kind !== 'move' && (
            <label className="field">
              <span>Name</span>
              <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Emberfall" onKeyDown={(e) => e.key === 'Enter' && save()} />
            </label>
          )}
          {edit.kind !== 'rename' && (
            <label className="field">
              <span>{edit.kind === 'create' ? 'Organization' : 'Move to'}</span>
              <select className="select" value={org} onChange={(e) => setOrg(e.target.value)}>
                <option value="">Choose an organization</option>
                {orgs.filter((o) => edit.kind === 'create' || o.id !== edit.p.org.id).map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
              </select>
            </label>
          )}
          {edit.kind === 'create' && <p className="fine">The project starts with one branch, Main, and the starter ignore rules for OS junk, autosaves and engine caches.</p>}
          {err && <div className="notice bad">{err}</div>}
          <div className="actions">
            <button className="btn" onClick={() => setEdit(null)}>Cancel</button>
            <button className="btn pri" onClick={save} disabled={busy === 'save'}>
              {edit.kind === 'create' ? 'Make project' : edit.kind === 'rename' ? 'Rename' : 'Move project'}
            </button>
          </div>
        </Dialog>
      )}
    </>
  );
}
