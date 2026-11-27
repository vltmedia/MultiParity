import { useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { api, downloadFile, fetchAuthed } from '../../api/client';
import { daemon } from '../../api/daemon';
import type { FileEntry, FileVersion, Tag, TagRef } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { useDaemon } from '../../lib/daemon';
import { useSession } from '../../lib/session';
import { baseName, dateTime, firstName, held, KIND_LABEL, plural, since, size } from '../../lib/format';
import { Dialog, Empty, ErrorNote, FileThumb, Loading, Menu, Thumb, Who, useAction, useUi } from '../../components/ui';
import { DRAWABLE, useThumb } from '../../lib/thumbs';
import { useHref, useProject, ViewHead } from './context';

const COLORS = ['#00806a', '#ff6a13', '#7a3cff', '#1b1324', '#c2185b', '#1f6fd1', '#b88a00', '#6d6378'];

type Row =
  | { type: 'folder'; name: string; path: string; files: number; size: number; change: number; claims: number }
  | { type: 'file'; f: FileEntry }
  | { type: 'new'; path: string; size: number };

type St = { cls: string; label: string };

export default function Files() {
  const { project, branch, ws, status, v, bump, reloadStatus } = useProject();
  const { me } = useSession();
  const d = useDaemon();
  const href = useHref();
  const { toast, fail } = useUi();
  const { busy, run } = useAction();
  const [params, setParams] = useSearchParams();
  const prefix = (params.get('prefix') || '').replace(/^\/+|\/+$/g, '');
  const tag = params.get('tag') || '';
  const [q, setQ] = useState(params.get('q') || '');
  const [qLive, setQLive] = useState(q);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [newTag, setNewTag] = useState(false);
  const [versionsOf, setVersionsOf] = useState<FileEntry | null>(null);
  const [previewOf, setPreviewOf] = useState<FileEntry | null>(null);

  useEffect(() => {
    const t = setTimeout(() => setQLive(q), 250);
    return () => clearTimeout(t);
  }, [q]);
  useEffect(() => setSel(new Set()), [prefix, tag, branch.id]);

  const files = useAsync(
    () => api.files(project.id, branch.id, { prefix: prefix ? prefix + '/' : undefined, tag: tag || undefined, q: qLive || undefined }),
    [project.id, branch.id, prefix, tag, qLive, v.changes, v.claims, v.tags],
  );
  const tags = useAsync(() => api.tags(project.id), [project.id, v.tags]);

  const setParam = (k: string, val: string) => {
    const n = new URLSearchParams(params);
    if (val) n.set(k, val);
    else n.delete(k);
    setParams(n);
  };

  const onBranch = !!ws && ws.branch_id === branch.id;
  const edited = new Set(onBranch ? status?.edited.map((f) => f.path) : []);
  const deleted = new Set(onBranch ? status?.deleted.map((f) => f.path) : []);

  const statusOf = (f: FileEntry): St => {
    const mine = f.claim?.user.id === me?.id;
    if (deleted.has(f.path)) return { cls: 'deleted', label: 'Deleted here' };
    if (edited.has(f.path)) return { cls: 'edited', label: 'Edited' };
    if (f.claim && mine) return { cls: 'mine', label: 'Claimed by you' };
    if (f.claim) return { cls: 'theirs', label: `Claimed by ${firstName(f.claim.user.name)}` };
    if (onBranch && f.change > ws!.synced_change) return { cls: 'behind', label: 'Newer available' };
    if (onBranch) return { cls: 'ok', label: 'Up to date' };
    return { cls: 'free', label: 'Free to claim' };
  };

  const rows: Row[] = useMemo(() => {
    const list = files.data || [];
    const base = prefix ? prefix + '/' : '';
    const folders = new Map<string, Extract<Row, { type: 'folder' }>>();
    const out: Row[] = [];
    const flat = !!qLive;
    for (const f of list) {
      const rest = f.path.startsWith(base) ? f.path.slice(base.length) : f.path;
      const i = rest.indexOf('/');
      if (i >= 0 && !flat) {
        const name = rest.slice(0, i);
        const path = base + name;
        const g = folders.get(path) || { type: 'folder' as const, name, path, files: 0, size: 0, change: 0, claims: 0 };
        g.files += 1;
        g.size += f.size;
        g.change = Math.max(g.change, f.change);
        if (f.claim) g.claims += 1;
        folders.set(path, g);
      } else out.push({ type: 'file', f });
    }
    const news: Row[] =
      onBranch && status && !tag
        ? status.new
            .filter((n) => (flat ? n.path.toLowerCase().includes(qLive.toLowerCase()) : n.path.startsWith(base) && !n.path.slice(base.length).includes('/')))
            .map((n) => ({ type: 'new' as const, path: n.path, size: n.size }))
        : [];
    return [
      ...[...folders.values()].sort((a, b) => a.name.localeCompare(b.name)),
      ...news,
      ...out.sort((a, b) => (a.type === 'file' && b.type === 'file' ? baseName(a.f.path).localeCompare(baseName(b.f.path)) : 0)),
    ];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [files.data, prefix, qLive, status, onBranch, tag]);

  const fileRows = rows.filter((r): r is Extract<Row, { type: 'file' }> => r.type === 'file');
  const selected = fileRows.filter((r) => sel.has(r.f.path)).map((r) => r.f);
  const mineSel = selected.filter((f) => f.claim?.user.id === me?.id);
  const freeSel = selected.filter((f) => !f.claim);
  const theirsSel = selected.filter((f) => f.claim && f.claim.user.id !== me?.id);
  const allTicked = fileRows.length > 0 && fileRows.every((r) => sel.has(r.f.path));

  const toggle = (p: string) => {
    const n = new Set(sel);
    if (n.has(p)) n.delete(p);
    else n.add(p);
    setSel(n);
  };

  async function claim(paths: string[]) {
    const r = await run('claim', 'claim the files', () => api.claim(project.id, branch.id, paths));
    if (!r) return;
    if (r.held.length)
      toast(`${plural(r.claimed.length, 'file')} claimed. ${baseName(r.held[0].path)} is held by ${firstName(r.held[0].user.name)}${r.held.length > 1 ? ` and ${r.held.length - 1} more by others` : ''}.`, 'hold');
    else toast(`Claimed ${r.claimed.length === 1 ? baseName(r.claimed[0]) : plural(r.claimed.length, 'file')}. Nobody else can publish ${r.claimed.length === 1 ? 'it' : 'them'} until you publish or release.`);
    bump('claims');
    reloadStatus();
  }
  async function release(paths: string[]) {
    const r = await run('release', 'release the files', () => api.release(project.id, branch.id, paths));
    if (!r) return;
    toast(`Released ${r.released.length === 1 ? baseName(r.released[0]) : plural(r.released.length, 'file')}.`);
    bump('claims');
  }
  async function nudge(f: FileEntry) {
    const r = await run('nudge', 'ask for the file', () => api.nudge(project.id, branch.id, f.path));
    if (r) toast(`Asked ${firstName(f.claim!.user.name)} to release ${baseName(f.path)}. They get a notification.`);
  }
  async function applyTag(t: TagRef, paths: string[]) {
    const r = await run('tag', `tag the files ${t.name}`, () => api.applyTag(project.id, t.id, paths));
    if (r) {
      toast(`Tagged ${paths.length === 1 ? baseName(paths[0]) : plural(paths.length, 'file')} ${t.name}.`);
      bump('tags');
    }
  }
  async function removeTag(t: TagRef, paths: string[]) {
    const r = await run('tag', `remove the tag ${t.name}`, () => api.removeTag(project.id, t.id, paths));
    if (r) {
      toast(`Removed ${t.name} from ${paths.length === 1 ? baseName(paths[0]) : plural(paths.length, 'file')}.`);
      bump('tags');
    }
  }
  async function getLatest() {
    if (!ws) return;
    const r = await run('latest', 'get latest', () => daemon.getLatest(ws.id));
    if (r) {
      toast(r.downloaded ? `Got latest: ${plural(r.downloaded, 'file')}, ${size(r.bytes)}.` : 'Already up to date.');
      reloadStatus();
      d.refresh();
    }
  }

  const crumbs = prefix ? prefix.split('/') : [];
  const behind = onBranch ? status?.behind || 0 : 0;
  const tagList = tags.data || [];
  const activeTag = tagList.find((t) => t.id === tag);
  const total = rows.length;
  const newCount = rows.filter((r) => r.type === 'new').length;

  return (
    <>
      <ViewHead
        title={prefix ? baseName(prefix) : 'Files'}
        sub={
          files.data
            ? `${plural(fileRows.length + rows.filter((r) => r.type === 'folder').reduce((a, r) => a + (r.type === 'folder' ? r.files : 0), 0), 'file')}${activeTag ? ` tagged ${activeTag.name}` : ''}${qLive ? ` matching “${qLive}”` : ''} on ${branch.name}.${newCount ? ` ${plural(newCount, 'new file')} only on this computer until you publish.` : ''}`
            : undefined
        }
      >
        <label>
          <span className="sr">Find a file</span>
          <input className="input" type="search" placeholder="Find a file by name" value={q} onChange={(e) => { setQ(e.target.value); setParam('q', e.target.value); }} />
        </label>
      </ViewHead>
      <div className="view">
        <div className="crumb">
          <Link to={href('files', { tag })}>{project.name}</Link>
          {crumbs.map((c, i) => (
            <span key={i} style={{ display: 'contents' }}>
              <span className="sep">/</span>
              {i === crumbs.length - 1 ? <span>{c}</span> : <Link to={href('files', { prefix: crumbs.slice(0, i + 1).join('/'), tag })}>{c}</Link>}
            </span>
          ))}
          <div className="tools">
            {behind > 0 && (
              <button className="btn sm" onClick={getLatest} disabled={busy === 'latest'}>{busy === 'latest' ? 'Getting latest' : `Get latest (${behind})`}</button>
            )}
          </div>
        </div>

        <div className="tagbar" role="group" aria-label="Filter by tag">
          <button className="tagf" aria-pressed={!tag} onClick={() => setParam('tag', '')}>All files</button>
          {tagList.map((t) => (
            <button key={t.id} className="tagf dot" aria-pressed={tag === t.id} style={{ ['--t' as string]: t.color }} onClick={() => setParam('tag', tag === t.id ? '' : t.id)} title={t.group ? `${t.group}: one of these per file` : undefined}>
              {t.name}
            </button>
          ))}
          <button className="tagf add" onClick={() => setNewTag(true)}>New tag</button>
        </div>

        {selected.length > 0 && (
          <div className="selbar" role="region" aria-label="Selected files">
            <span>{plural(selected.length, 'file')} selected</span>
            {freeSel.length > 0 && <button className="btn sm" disabled={!!busy} onClick={() => claim(freeSel.map((f) => f.path))}>Claim {freeSel.length > 1 ? plural(freeSel.length, 'file') : ''}</button>}
            {mineSel.length > 0 && <button className="btn sm" disabled={!!busy} onClick={() => release(mineSel.map((f) => f.path))}>Release {mineSel.length > 1 ? plural(mineSel.length, 'file') : ''}</button>}
            {theirsSel.length === 1 && <button className="btn sm" disabled={!!busy} onClick={() => nudge(theirsSel[0])}>Ask {firstName(theirsSel[0].claim!.user.name)} to release</button>}
            <Menu label="Add a tag to the selected files" button="Add tag">
              {(close) => (
                <>
                  {tagList.map((t) => (
                    <button key={t.id} className="tagdot" style={{ ['--t' as string]: t.color }} onClick={() => { close(); applyTag(t, selected.map((f) => f.path)); }}>
                      {t.name}
                      {t.group && <small>{t.group}</small>}
                    </button>
                  ))}
                  {!tagList.length && <p className="fine" style={{ padding: 8 }}>No tags yet.</p>}
                </>
              )}
            </Menu>
            {selected.some((f) => f.tags.length) && (
              <Menu label="Remove a tag from the selected files" button="Remove tag">
                {(close) => (
                  <>
                    {[...new Map(selected.flatMap((f) => f.tags).map((t) => [t.id, t])).values()].map((t) => (
                      <button key={t.id} className="tagdot" style={{ ['--t' as string]: t.color }} onClick={() => { close(); removeTag(t, selected.filter((f) => f.tags.some((x) => x.id === t.id)).map((f) => f.path)); }}>
                        {t.name}
                      </button>
                    ))}
                  </>
                )}
              </Menu>
            )}
            {selected.length === 1 && (
              <>
                <button className="btn sm" onClick={() => downloadFile(project.id, branch.id, selected[0].path).catch((e) => fail(e, 'download the file'))}>Download</button>
                <button className="btn sm" onClick={() => setPreviewOf(selected[0])}>Preview</button>
                <button className="btn sm" onClick={() => setVersionsOf(selected[0])}>Older versions</button>
              </>
            )}
            <button className="link" onClick={() => setSel(new Set())}>Clear selection</button>
          </div>
        )}

        {files.loading && !files.data && <Loading what="Loading files" />}
        {files.error !== undefined && <ErrorNote error={files.error} doing="load the files" retry={files.reload} />}
        {files.data && total === 0 && (
          <Empty title={tag || qLive ? 'No files match' : 'This folder is empty'}>
            {tag || qLive ? 'Try another tag, or clear the search.' : ws ? 'Save files into the folder on your computer; they show up as New by themselves.' : 'Files appear here once someone publishes them.'}
          </Empty>
        )}
        {total > 0 && (
          <table className="t files">
            <thead>
              <tr>
                <th className="c">
                  <input type="checkbox" aria-label="Select all files in this folder" checked={allTicked} onChange={() => setSel(allTicked ? new Set() : new Set(fileRows.map((r) => r.f.path)))} />
                </th>
                <th>Name</th>
                <th>Status</th>
                <th>Claimed by</th>
                <th>Tags</th>
                <th className="kindcol">Kind</th>
                <th className="r">Size</th>
                <th className="r">Change</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => {
                if (r.type === 'folder')
                  return (
                    <tr key={'d:' + r.path} className="folder">
                      <td className="c" />
                      <td className="name">
                        <span className="thumb" aria-hidden="true" style={{ width: 26, height: 26 }} />
                        <Link className="foldbtn" to={href('files', { prefix: r.path, tag })}>{r.name}</Link>
                      </td>
                      <td><span className="mute">{plural(r.files, 'file')}</span></td>
                      <td>{r.claims ? <span className="mute">{plural(r.claims, 'claim')}</span> : null}</td>
                      <td />
                      <td className="kindcol mute">Folder</td>
                      <td className="r">{size(r.size)}</td>
                      <td className="r">{r.change || ''}</td>
                    </tr>
                  );
                if (r.type === 'new')
                  return (
                    <tr key={'n:' + r.path} className="new">
                      <td className="c" />
                      <td className="name"><Thumb seed={r.path} />{baseName(r.path)}</td>
                      <td><span className="st new">New</span></td>
                      <td />
                      <td><span className="fine">Tag after publishing</span></td>
                      <td className="kindcol" />
                      <td className="r">{size(r.size)}</td>
                      <td className="r mute">Not published</td>
                    </tr>
                  );
                const f = r.f;
                const st = statusOf(f);
                return (
                  <tr key={f.path} className={sel.has(f.path) ? 'sel' : ''}>
                    <td className="c">
                      <input type="checkbox" aria-label={`Select ${baseName(f.path)}`} checked={sel.has(f.path)} onChange={() => toggle(f.path)} />
                    </td>
                    <td className="name" title={f.path}>
                      <button className="thumbbtn" aria-label={`Preview ${baseName(f.path)}`} onClick={() => setPreviewOf(f)}>
                        <FileThumb projectId={project.id} hash={f.file_hash} path={f.path} />
                      </button>
                      {qLive ? f.path : baseName(f.path)}
                    </td>
                    <td><span className={`st ${st.cls}`}>{st.label}</span></td>
                    <td>
                      {f.claim && (
                        <span className="row" style={{ gap: 6, flexWrap: 'nowrap' }}>
                          <Who id={f.claim.user.id} name={f.claim.user.name} />
                          <span>{f.claim.user.id === me?.id ? 'You' : firstName(f.claim.user.name)}, {held(f.claim.since)}</span>
                        </span>
                      )}
                    </td>
                    <td className="tagscell">
                      {f.tags.map((t) => (
                        <span key={t.id} className="tag" style={{ ['--t' as string]: t.color }}>
                          {t.name}
                          <button aria-label={`Remove ${t.name} from ${baseName(f.path)}`} title={`Remove ${t.name}`} onClick={() => removeTag(t, [f.path])}>×</button>
                        </span>
                      ))}
                      <Menu label={`Add a tag to ${baseName(f.path)}`} button="Add tag" className="addtag">
                        {(close) => (
                          <>
                            {tagList.filter((t) => !f.tags.some((x) => x.id === t.id)).map((t) => (
                              <button key={t.id} className="tagdot" style={{ ['--t' as string]: t.color }} onClick={() => { close(); applyTag(t, [f.path]); }}>
                                {t.name}
                                {t.group && <small>{t.group}</small>}
                              </button>
                            ))}
                          </>
                        )}
                      </Menu>
                    </td>
                    <td className="kindcol">{KIND_LABEL[f.kind] || f.kind}</td>
                    <td className="r">{size(f.size)}</td>
                    <td className="r"><Link to={href(`changes/${f.change}`)} className="rowlink">{f.change}</Link></td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
        {ws && onBranch && files.data && (
          <div className="drop">
            <b>To add files, save them into {ws.root}{prefix ? '/' + prefix : ''}.</b> They show up here as New by themselves, ticked for your next change.
          </div>
        )}
      </div>
      {previewOf && <PreviewDialog file={previewOf} onClose={() => setPreviewOf(null)} />}
      {versionsOf && (
        <VersionsDialog
          file={versionsOf}
          edited={edited.has(versionsOf.path)}
          onClose={() => setVersionsOf(null)}
          onChanged={() => { bump('changes'); reloadStatus(); }}
        />
      )}
      {newTag && <NewTagDialog onClose={() => setNewTag(false)} onMade={(t) => { setNewTag(false); bump('tags'); toast(`Made the tag ${t.name}.`); }} />}
    </>
  );
}

function NewTagDialog({ onClose, onMade }: { onClose: () => void; onMade: (t: Tag) => void }) {
  const { project } = useProject();
  const { run, busy } = useAction();
  const [name, setName] = useState('');
  const [color, setColor] = useState(COLORS[0]);
  const [group, setGroup] = useState('');
  const [only, setOnly] = useState(false);
  const [err, setErr] = useState('');
  async function make() {
    if (!name.trim()) return setErr('Give the tag a name, like Approved or Needs review.');
    const t = await run('make', 'make the tag', () => api.createTag(project.id, { name: name.trim(), color, group: group.trim() || undefined, project_only: only }));
    if (t) onMade(t);
  }
  return (
    <Dialog title="New tag" onClose={onClose}>
      <label className="field">
        <span>Name</span>
        <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Needs review" onKeyDown={(e) => e.key === 'Enter' && make()} />
      </label>
      <div className="field">
        <span>Colour</span>
        <div className="row" role="radiogroup" aria-label="Colour">
          {COLORS.map((c) => (
            <button key={c} role="radio" aria-checked={color === c} aria-label={c} onClick={() => setColor(c)} style={{ width: 28, height: 28, background: c, outline: color === c ? '2px solid var(--ink)' : undefined, outlineOffset: 2 }} />
          ))}
        </div>
      </div>
      <label className="field">
        <span>Group, optional</span>
        <input className="input" value={group} onChange={(e) => setGroup(e.target.value)} placeholder="Status" />
        <small>A file can carry only one tag from a group, so a group works as a status.</small>
      </label>
      <label className="check-label">
        <input type="checkbox" checked={only} onChange={(e) => setOnly(e.target.checked)} /> Only for {project.name}, not the whole organization
      </label>
      {err && <div className="notice bad">{err}</div>}
      <div className="actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn pri" onClick={make} disabled={!!busy}>Make tag</button>
      </div>
    </Dialog>
  );
}

/** Every published version of one file, newest first, with ways to get one back. */
function VersionsDialog({ file, edited, onClose, onChanged }: { file: FileEntry; edited: boolean; onClose: () => void; onChanged: () => void }) {
  const { project, branch, ws } = useProject();
  const { toast, fail } = useUi();
  const { busy, run } = useAction();
  const [confirm, setConfirm] = useState<FileVersion | null>(null);
  const list = useAsync(() => api.versions(project.id, branch.id, file.path), [project.id, branch.id, file.path]);
  const name = baseName(file.path);
  const inFolder = !!ws && ws.branch_id === branch.id;
  const protectedBranch = branch.protected && project.my_role !== 'admin' && project.my_role !== 'owner';

  async function makeLatest(v: FileVersion) {
    const r = await run('latest', `make change ${v.change}'s version the latest`, () => api.restoreVersion(project.id, branch.id, file.path, v.change));
    if (!r) return;
    setConfirm(null);
    toast(`Change ${r.change.number}: ${name} as of change ${v.change} is the latest again.${inFolder ? ' Get latest to have it in your folder.' : ''}`);
    onChanged();
    list.reload();
  }
  async function toFolder(v: FileVersion, to: 'folder' | 'copy') {
    const r = await run(to + v.change, to === 'folder' ? 'put the version in your folder' : 'save a copy', () => daemon.version(ws!.id, file.path, v.change, to));
    if (!r) return;
    if (to === 'folder') {
      toast(`Put back ${name} as of change ${v.change}. It shows as edited: publish it to make it the latest.${r.undo ? ' Your version is in a checkpoint.' : ''}`);
      onChanged();
      onClose();
    } else toast(`Saved a copy: ${r.saved_to}`);
  }

  const versions = list.data?.versions || [];
  return (
    <Dialog title={`Older versions of ${name}`} onClose={onClose} wide>
      <p className="fine">
        {list.data && !versions.length && 'This file has no published versions yet.'}
        {versions.length > 0 && `${plural(versions.length, 'published version')}, newest first. Going back loses nothing: newer versions stay in the history.`}
        {edited && ' You have edits to this file on this computer: putting a version back saves them in a checkpoint first.'}
      </p>
      {list.loading && !list.data && <Loading what="Loading versions" />}
      {list.error !== undefined && <ErrorNote error={list.error} doing="load the versions" retry={list.reload} />}
      {versions.length > 0 && (
        <ul className="versions">
          {versions.map((v) => {
            const gone = v.op === 'delete';
            return (
              <li key={v.change} className={v.current ? 'cur' : ''}>
                <div className="vwhen">
                  <b>Change {v.change}</b>
                  <small title={dateTime(v.created_at)}>{since(v.created_at)}</small>
                </div>
                <div className="vwhat">
                  <b>{v.message}</b>
                  <small>
                    {v.author ? v.author.name : 'Someone'} · {gone ? 'deleted it' : size(v.size)}
                    {v.branch_id !== branch.id && ` · on ${v.branch_name}`}
                    {v.current && ' · the latest'}
                  </small>
                </div>
                {!gone && (
                  <div className="vacts">
                    <button className="btn sm" onClick={() => downloadFile(project.id, branch.id, file.path, v.change).catch((e) => fail(e, 'download that version'))}>Download</button>
                    {inFolder && !(v.current && !edited) && (
                      <button className="btn sm" disabled={!!busy} title="Put this version in your folder in place of the file. Publish it to make it the latest." onClick={() => toFolder(v, 'folder')}>Put back</button>
                    )}
                    {!v.current && !protectedBranch && (
                      <button className="btn sm" disabled={!!busy} onClick={() => setConfirm(v)}>Make latest</button>
                    )}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {confirm && (
        <div className="notice hold">
          <p>
            Make change {confirm.change}&rsquo;s version of {name} the latest for everyone on {branch.name}? It becomes a new change; the version it replaces stays in the history.
          </p>
          <div className="actions">
            <button className="btn" onClick={() => setConfirm(null)}>Cancel</button>
            <button className="btn pri" disabled={!!busy} onClick={() => makeLatest(confirm)}>{busy === 'latest' ? 'Making it the latest' : 'Make it the latest'}</button>
          </div>
        </div>
      )}
      <div className="actions">
        <button className="btn" onClick={onClose}>Done</button>
      </div>
    </Dialog>
  );
}

const TEXT = /\.(txt|md|json|ya?ml|toml|ini|cfg|xml|csv|html|css|js|ts|tsx|jsx|py|lua|gd|cs|cpp|c|h|hpp|rs|go|java|shader|hlsl|glsl|usf|ush|uproject|uplugin|meta)$/i;

/** A big picture of the file (images and Blender files), or its text. */
function PreviewDialog({ file, onClose }: { file: FileEntry; onClose: () => void }) {
  const { project, branch } = useProject();
  const { fail } = useUi();
  const drawable = DRAWABLE.test(file.path);
  const big = useThumb(project.id, drawable ? file.file_hash : null, 'large');
  const [text, setText] = useState<string | null>(null);
  const [waited, setWaited] = useState(false);
  useEffect(() => {
    const t = setTimeout(() => setWaited(true), 20_000);
    return () => clearTimeout(t);
  }, []);
  useEffect(() => {
    if (!TEXT.test(file.path)) return;
    let live = true;
    fetchAuthed(`/api/projects/${encodeURIComponent(project.id)}/download?path=${encodeURIComponent(file.path)}&branch=${encodeURIComponent(branch.id)}`)
      .then((r) => (r.ok ? r.blob() : null))
      .then(async (b) => { if (live && b) setText(await b.slice(0, 64 * 1024).text() + (b.size > 64 * 1024 ? '\n…' : '')); })
      .catch(() => {});
    return () => { live = false; };
  }, [file.path, project.id, branch.id]);
  return (
    <Dialog title={baseName(file.path)} onClose={onClose} wide>
      <p className="fine">{file.path} · {size(file.size)} · change {file.change}</p>
      <div className="preview">
        {big ? <img src={big} alt={`Preview of ${baseName(file.path)}`} />
          : text !== null ? <pre>{text}</pre>
          : drawable ? <p className="fine">{waited ? 'No picture of this file yet. Blender files show one when they were saved with a preview.' : 'Drawing a preview…'}</p>
          : TEXT.test(file.path) ? <p className="fine">Loading…</p>
          : <p className="fine">No preview for this kind of file yet. Download it to open it.</p>}
      </div>
      <div className="actions">
        <button className="btn" onClick={() => downloadFile(project.id, branch.id, file.path).catch((e) => fail(e, 'download the file'))}>Download</button>
        <button className="btn pri" onClick={onClose}>Done</button>
      </div>
    </Dialog>
  );
}
