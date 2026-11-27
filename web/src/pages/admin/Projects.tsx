import { Fragment, useEffect, useRef, useState, type ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../../api/client';
import type { CheckpointSetting, OrgStructure, Production, ProjectStorage, ProjectSummary, StorageLocation } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { STORAGE_KIND_LABEL, movePercent, moveStatus, plural, since, size } from '../../lib/format';
import { Dialog, Empty, ErrorNote, Loading, Parity, useAction, useUi } from '../../components/ui';
import { AdminHead, useAdmin } from './AdminLayout';
import ThemePicker from '../../components/ThemePicker';
import ExportDialog from './ExportDialog';
import { PlaceSelect, StructureDialog, placeBody, useStructures, type StructEdit } from './Structure';
import { WhoDialog } from './Policies';
import { DEFAULT_THEMES, themeById } from '../../lib/theme';

const KEEP: Array<[CheckpointSetting, string]> = [
  ['auto', 'Automatic'],
  ['server', 'On the server'],
  ['local', 'On each computer'],
];
const KEPT = { server: 'on the server', local: 'on each computer' };

type Edit = { kind: 'rename' | 'move'; p: ProjectSummary } | { kind: 'create' };

export default function AdminProjects() {
  const { scope, orgs, scopeName, isServerAdmin } = useAdmin();
  const list = useAsync(() => api.admin.projects(scope || undefined), [scope]);
  // Storage locations, for where new projects go and for moving file data. If this fails the choices are simply not offered.
  const storage = useAsync(() => api.admin.storage(), []);
  const locations = storage.data?.locations || [];
  const { confirm, toast } = useUi();
  const { run, busy } = useAction();
  const [edit, setEdit] = useState<Edit | null>(null);
  const [name, setName] = useState('');
  const [org, setOrg] = useState('');
  const [where, setWhere] = useState('');
  const [err, setErr] = useState('');
  const [showArchived, setShowArchived] = useState(false);
  const [stor, setStor] = useState<ProjectSummary | null>(null);
  const [exp, setExp] = useState<ProjectSummary | null>(null);
  const [look, setLook] = useState<ProjectSummary | null>(null);
  const [sedit, setSedit] = useState<StructEdit | null>(null);
  const [who, setWho] = useState<ProjectSummary | null>(null);
  const [place, setPlace] = useState('');
  const orgIds = scope ? [scope] : orgs.map((o) => o.id);
  const structures = useStructures(orgIds);

  const open = (e: Edit) => {
    setErr('');
    setEdit(e);
    setName(e.kind === 'rename' ? e.p.name : '');
    setOrg(e.kind === 'move' ? '' : e.kind === 'create' ? scope || orgs[0]?.id || '' : '');
    setWhere(locations.find((l) => l.main)?.name || '');
    setPlace('');
  };

  async function save() {
    if (!edit) return;
    setErr('');
    if (edit.kind === 'create') {
      if (!name.trim()) return setErr('Give the project a name.');
      if (!org) return setErr('Choose which organization owns it.');
      const storeIn = locations.length > 1 ? where || undefined : undefined;
      const pl = placeBody(place);
      const r = await run('save', 'make the project', () => api.createProject(org, name.trim(), undefined, storeIn, { client: pl.client || undefined, production: pl.production || undefined }));
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
    structures.reload();
  }

  async function makeMain(p: ProjectSummary) {
    if (!p.production) return;
    const pr = p.production;
    const r = await run('main:' + p.id, `make ${p.name} the main project`, () => api.admin.updateProduction(pr.id, { main_project: p.id }));
    if (r) {
      toast(`${p.name} is now the main project of ${pr.name}: listed first, and where opening ${pr.name} lands.`);
      list.reload();
      structures.reload();
    }
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
      <AdminHead title="Projects" sub={`Every project in ${scopeName}, by client and production, including archived ones.`}>
        <button className="btn sm" onClick={() => setSedit({ kind: 'new-client', org: scope || undefined })}>New client</button>
        <button className="btn sm" onClick={() => setSedit({ kind: 'new-production', org: scope || undefined })}>New production</button>
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
              <tr><th>Project</th>{orgIds.length > 1 && <th>Organization</th>}<th className="r">Size</th><th className="r">Files</th><th className="r">Change</th><th>Last change</th><th>Stored in</th><th>Keep checkpoints</th><th /></tr>
            </thead>
            <tbody>
              {orgIds.map((oid) => {
                const ps = rows.filter((p) => p.org.id === oid);
                const st: OrgStructure = structures.data[oid] || { clients: [], productions: [] };
                if (!ps.length && !st.clients.length && !st.productions.length) return null;
                const filed = st.clients.length > 0 || st.productions.length > 0;
                const orgName = orgs.find((o) => o.id === oid)?.name || ps[0]?.org.name || '';
                const head = (key: string, depth: number, kind: 'org' | 'client' | 'production' | 'loose', label: string, note: string, acts?: ReactNode) => (
                  <tr key={key} className={`grp ${kind}`}>
                    <td colSpan={orgIds.length > 1 ? 9 : 8} style={{ paddingLeft: 12 + depth * 22 }}>
                      <span className="grp-name" data-ic={{ org: 'building-2', client: 'briefcase', production: 'clapperboard', loose: 'folders' }[kind]}>{label}</span>
                      {note && <span className="fine">{note}</span>}
                      {acts && <span className="grp-acts">{acts}</span>}
                    </td>
                  </tr>
                );
                const renderRow = (p: ProjectSummary, depth: number) => (
                <tr key={p.id} style={p.archived ? { opacity: 0.65 } : undefined}>
                  <td style={{ paddingLeft: 12 + depth * 22 }}>
                    {p.archived ? p.name : <Link className="rowlink" to={`/p/${p.id}`}>{p.name}</Link>}
                    {p.is_main && <span className="badge good" style={{ marginLeft: 8 }} title={`Listed first in ${p.production?.name}, and where opening it lands`}>Main</span>}
                    {p.archived && <span className="badge mute" style={{ marginLeft: 8 }}>Archived</span>}
                  </td>
                  {orgIds.length > 1 && <td>{p.org.name}</td>}
                  <td className="r">{size(p.size)}</td>
                  <td className="r">{p.file_count.toLocaleString()}</td>
                  <td className="r">{p.head_change}</td>
                  <td>{p.last_change ? since(p.last_change.created_at) : <span className="mute">Never</span>}</td>
                  <td>
                    {p.storage}
                    {p.storage_moving && <span className="badge hold" style={{ marginLeft: 8 }}>Moving…</span>}
                  </td>
                  <td>
                    <select className="select sm" title="Automatic: on the server once the project is 20 GB or larger, on each computer before that" aria-label={`Keep checkpoints for ${p.name}`} value={p.checkpoint_setting} disabled={busy === 'keep:' + p.id} onChange={(e) => keep(p, e.target.value as CheckpointSetting)}>
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
                    <span className="row proj-acts" style={{ justifyContent: 'flex-end', flexWrap: 'wrap', gap: 6, minWidth: 300, maxWidth: 420, marginLeft: 'auto' }}>
                      <button className="btn sm" onClick={() => open({ kind: 'rename', p })}>Rename</button>
                      <button className="btn sm" onClick={() => setWho(p)}>Who can see…</button>
                      <button className="btn sm" onClick={() => setSedit({ kind: 'file', p })}>File under…</button>
                      {p.production && !p.is_main && !p.archived && <button className="btn sm" disabled={busy === 'main:' + p.id} onClick={() => makeMain(p)}>Make main</button>}
                      <button className="btn sm" onClick={() => setLook(p)}>Look…</button>
                      {orgs.length > 1 && <button className="btn sm" onClick={() => open({ kind: 'move', p })}>Move</button>}
                      {isServerAdmin && locations.length > 1 && <button className="btn sm" onClick={() => setStor(p)}>Change storage…</button>}
                      <button className="btn sm" onClick={() => setExp(p)}>Export…</button>
                      {p.archived ? (
                        <button className="btn sm" disabled={busy === 'arch:' + p.id} onClick={() => restore(p)}>Restore</button>
                      ) : (
                        <button className="btn sm danger" disabled={busy === 'arch:' + p.id} onClick={() => archive(p)}>Archive</button>
                      )}
                    </span>
                  </td>
                </tr>
                );
                const count = (n: number) => (n ? plural(n, 'project') : 'No projects yet');
                const prodBlock = (pr: Production, depth: number) => {
                  const inside = ps.filter((p) => p.production?.id === pr.id).sort((a, b) => Number(!!b.is_main) - Number(!!a.is_main));
                  return (
                    <Fragment key={pr.id}>
                      {head('pr' + pr.id, depth, 'production', pr.name, `${count(inside.length)}${inside.length && !pr.main_project_id ? ', no main project yet' : ''}`,
                        <button className="btn sm" onClick={() => setSedit({ kind: 'production', org: oid, pr })}>Edit…</button>)}
                      {inside.map((p) => renderRow(p, depth + 1))}
                    </Fragment>
                  );
                };
                // Anything not shown in a group above (also if the structure could not be loaded).
                const shown = (p: ProjectSummary) =>
                  p.production ? st.productions.some((pr) => pr.id === p.production!.id) : p.client ? st.clients.some((c) => c.id === p.client!.id) : false;
                const loose = ps.filter((p) => !shown(p));
                return (
                  <Fragment key={oid}>
                    {orgIds.length > 1 && head('o' + oid, 0, 'org', orgName, '')}
                    {st.clients.map((c) => {
                      const direct = ps.filter((p) => p.client?.id === c.id && !p.production);
                      const prods = st.productions.filter((pr) => pr.client_id === c.id);
                      const note = [prods.length ? plural(prods.length, 'production') : '', direct.length ? plural(direct.length, 'project') : ''].filter(Boolean).join(', ') || 'Nothing filed here yet';
                      return (
                        <Fragment key={c.id}>
                          {head('c' + c.id, 0, 'client', c.name, note,
                            <>
                              <button className="btn sm" onClick={() => setSedit({ kind: 'new-production', org: oid, client: c.id })}>New production</button>
                              <button className="btn sm" onClick={() => setSedit({ kind: 'client', org: oid, c })}>Edit…</button>
                            </>)}
                          {prods.map((pr) => prodBlock(pr, 1))}
                          {direct.map((p) => renderRow(p, 1))}
                        </Fragment>
                      );
                    })}
                    {st.productions.filter((pr) => !pr.client_id).map((pr) => prodBlock(pr, 0))}
                    {filed && loose.length > 0 && head('l' + oid, 0, 'loose', 'Not filed', 'Use File under… to put these in a client or production')}
                    {loose.map((p) => renderRow(p, filed ? 1 : 0))}
                  </Fragment>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
      {look && <LookDialog p={look} onClose={() => setLook(null)} onChanged={(np) => { setLook(np); list.reload(); }} />}
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
          {edit.kind === 'create' && org && ((structures.data[org]?.clients.length || 0) + (structures.data[org]?.productions.length || 0) > 0) && (
            <label className="field">
              <span>File under</span>
              <PlaceSelect s={structures.data[org]} value={place} onChange={setPlace} />
            </label>
          )}
          {edit.kind === 'create' && locations.length > 1 && (
            <label className="field">
              <span>Store file data in</span>
              <select className="select" value={where} onChange={(e) => setWhere(e.target.value)}>
                {locations.map((l) => (
                  <option key={l.name} value={l.name}>
                    {l.name} ({STORAGE_KIND_LABEL[l.kind] || l.kind}{l.main ? ', default' : ''})
                  </option>
                ))}
              </select>
              <small>A server admin can move it to another location later.</small>
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
      {who && <WhoDialog p={who} onClose={() => setWho(null)} />}
      {sedit && (
        <StructureDialog
          edit={sedit}
          orgs={scope ? orgs.filter((o) => o.id === scope) : orgs}
          structures={structures.data}
          onClose={() => setSedit(null)}
          onDone={() => { setSedit(null); list.reload(); structures.reload(); }}
        />
      )}
      {exp && <ExportDialog p={exp} onClose={() => setExp(null)} onArchived={() => list.reload()} />}
      {stor && (
        <StorageDialog
          p={stor}
          locations={locations}
          onClose={() => setStor(null)}
          onChanged={() => {
            list.reload();
            storage.reload();
          }}
        />
      )}
    </>
  );
}

/** Move a project's file data to another storage location, and follow the move while it runs. */
function StorageDialog({ p, locations, onClose, onChanged }: { p: ProjectSummary; locations: StorageLocation[]; onClose: () => void; onChanged: () => void }) {
  const { toast } = useUi();
  const { run, busy } = useAction();
  const info = useAsync(() => api.admin.projectStorage(p.id), [p.id]);
  const [to, setTo] = useState('');
  const [err, setErr] = useState('');
  const d: ProjectStorage | undefined = info.data;
  const move = d?.move || null;
  const running = move?.status === 'running';
  const wasRunning = useRef(false);
  const [finished, setFinished] = useState(false);
  const changed = useRef(onChanged);
  changed.current = onChanged;

  // Poll while the move runs; when it stops, say so and refresh the page behind the dialog.
  useEffect(() => {
    if (running) {
      wasRunning.current = true;
      setFinished(false);
      const t = setTimeout(info.reload, 3000);
      return () => clearTimeout(t);
    }
    if (wasRunning.current && move) {
      wasRunning.current = false;
      setFinished(true);
      changed.current();
    }
  }, [running, move, info.data, info.reload]);

  const current = d?.storage || p.storage;
  const others = locations.filter((l) => l.name !== current);
  const unfinished = !!move && (move.status === 'interrupted' || move.status === 'failed') && !!d?.previous;

  async function start(target: string, finishing: boolean) {
    setErr('');
    if (!target) return setErr('Choose where to move the file data.');
    const r = await run('move', finishing ? 'finish the move' : 'move the file data', () => api.admin.moveProjectStorage(p.id, target));
    if (!r) return;
    toast(finishing ? `Finishing the move of ${p.name} to ${target}. People can keep working.` : `Moving ${p.name} to ${target}. People can keep working.`);
    info.setData((x) => (x ? { ...x, move: r } : x));
    onChanged();
  }

  const pc = move ? movePercent(move) : null;

  return (
    <Dialog title={`Change where ${p.name} is stored`} onClose={onClose}>
      {info.loading && !d && <Loading what="Looking at storage" />}
      {info.error !== undefined && <ErrorNote error={info.error} doing="load where this project is stored" retry={info.reload} />}
      {d && (
        <>
          {!running && (
            <p>
              {p.name} is stored in <b>{current}</b>.
            </p>
          )}
          {move && move.status === 'running' && (
            <div className="result">
              <b className="big">{moveStatus(move)}</b>
              {pc !== null && <Parity total={20} done={Math.floor(pc / 5)} />}
              <p>
                Moving from {move.from} to {move.to}. {size(move.copied_bytes)} copied so far. People can keep working, and you can close this.
              </p>
            </div>
          )}
          {finished && move && move.status === 'done' && (
            <div className="result good">
              <b className="big">Moved to {move.to}</b>
              <p>The old copy in {move.from} stays until you reclaim space on the Fixing page.</p>
            </div>
          )}
          {finished && move && move.status === 'failed' && !d.previous && (
            <div className="result bad">
              <b className="big">The move to {move.to} did not finish</b>
              <p>{moveStatus(move)}</p>
            </div>
          )}
          {unfinished && move && (
            <div className="result bad">
              <b className="big">The move to {move.to} did not finish</b>
              <p>{moveStatus(move)}</p>
              <button className="btn sm pri" style={{ alignSelf: 'flex-start' }} disabled={busy === 'move'} onClick={() => start(move.to, true)}>
                {busy === 'move' ? 'Starting' : 'Finish the move'}
              </button>
            </div>
          )}
          {!running && !unfinished && (
            <>
              {others.length ? (
                <div className="field" role="radiogroup" aria-label="Move to">
                  <span>Move to</span>
                  {others.map((l) => (
                    <label key={l.name} className="check-label">
                      <input type="radio" name="storage-to" value={l.name} checked={to === l.name} onChange={() => setTo(l.name)} />
                      <span>
                        {l.name}
                        {l.main && <span className="badge mute" style={{ marginLeft: 8 }}>Default</span>}
                        <br />
                        <span className="fine">{STORAGE_KIND_LABEL[l.kind] || l.kind}{l.description ? `: ${l.description}` : ''}</span>
                      </span>
                    </label>
                  ))}
                </div>
              ) : (
                <p className="fine">There is no other storage location on this server.</p>
              )}
              <p className="fine">People can keep working while the file data is copied. The old copy is kept until you reclaim space on the Fixing page.</p>
            </>
          )}
          {err && <div className="notice bad">{err}</div>}
        </>
      )}
      <div className="actions">
        <button className="btn" onClick={onClose}>{running || unfinished ? 'Close' : 'Cancel'}</button>
        {d && !running && !unfinished && others.length > 0 && (
          <button className="btn pri" onClick={() => start(to, false)} disabled={busy === 'move'}>
            Move file data
          </button>
        )}
      </div>
    </Dialog>
  );
}

/** A project's own look, or its organization's. */
function LookDialog({ p, onClose, onChanged }: { p: ProjectSummary; onClose: () => void; onChanged: (p: ProjectSummary) => void }) {
  const { orgs } = useAdmin();
  const { run, busy } = useAction();
  const { toast } = useUi();
  const org = orgs.find((o) => o.id === p.org.id);
  const orgName = (which: 'light' | 'dark') => {
    const id = which === 'light' ? org?.theme_light || DEFAULT_THEMES.light : org?.theme_dark || DEFAULT_THEMES.dark;
    return `Same as ${p.org.name} (${themeById(id)?.name ?? id})`;
  };
  async function set(which: 'light' | 'dark', id: string) {
    const r = await run('look', 'change the look', () => api.admin.updateProject(p.id, which === 'light' ? { theme_light: id } : { theme_dark: id }));
    if (r) {
      toast(`${p.name} now uses ${id ? themeById(id)?.name : `${p.org.name}'s look`} in ${which} mode.`);
      onChanged(r as ProjectSummary);
    }
  }
  return (
    <Dialog title={`Look of ${p.name}`} onClose={onClose}>
      <p className="fine">Shown while someone has {p.name} open, unless they chose their own look.</p>
      <ThemePicker light={p.theme?.own_light || ''} dark={p.theme?.own_dark || ''} inherit={{ light: orgName('light'), dark: orgName('dark') }} onChange={set} disabled={busy === 'look'} />
      <div className="actions"><button className="btn pri" onClick={onClose}>Done</button></div>
    </Dialog>
  );
}
