// Admin > Projects > Export…: write a branch's files, as plain files or one .zip, to this server
// (then download the .zip) or to a folder, NAS or S3 bucket set up for exports. Earlier exports
// are listed with their progress, and finished .zip files on this server can be downloaded.
import { useEffect, useState } from 'react';
import { api } from '../../api/client';
import type { ProjectExport, ProjectSummary } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { plural, since, size } from '../../lib/format';
import { Dialog, ErrorNote, Loading, useAction, useUi } from '../../components/ui';

const KIND: Record<string, string> = { local: 'Folder on the server', mount: 'Folder or NAS', s3: 'S3 bucket' };

export default function ExportDialog({ p, onClose, onArchived }: { p: ProjectSummary; onClose: () => void; onArchived: () => void }) {
  const { toast } = useUi();
  const { run, busy } = useAction();
  const targets = useAsync(() => api.admin.exportTargets(), []);
  const branches = useAsync(() => api.branches(p.id), [p.id]);
  const list = useAsync(() => api.admin.exports(p.id), [p.id]);
  const [target, setTarget] = useState('server');
  const [format, setFormat] = useState<'zip' | 'files'>('zip');
  const [branch, setBranch] = useState('');
  const [archiveAfter, setArchiveAfter] = useState(false);
  const live = (list.data || []).some((e) => e.status === 'queued' || e.status === 'running');

  // Follow a running export; when one finishes with "archive afterwards", refresh the page behind.
  useEffect(() => {
    if (!live) return;
    const t = setTimeout(() => list.reload(), 1500);
    return () => clearTimeout(t);
  }, [live, list.data, list.reload]);
  const [seenLive, setSeenLive] = useState(false);
  useEffect(() => {
    if (live) setSeenLive(true);
    else if (seenLive) { setSeenLive(false); if ((list.data || [])[0]?.archive_after) onArchived(); }
  }, [live, seenLive, list.data, onArchived]);

  const main = (branches.data || []).find((b) => !b.parent_id);
  const chosen = (branches.data || []).find((b) => b.id === (branch || main?.id));
  const t = (targets.data || []).find((x) => x.name === target);

  async function start() {
    const r = await run('start', 'start the export', () =>
      api.admin.startExport(p.id, { branch: branch || undefined, target, format, archive_after: archiveAfter }));
    if (!r) return;
    toast(`Exporting ${p.name}. You can close this; it keeps going.`);
    list.setData((x) => [r, ...(x || [])]);
  }

  async function download(e: ProjectExport) {
    const r = await run('dl:' + e.id, 'get the download link', () => api.admin.exportLink(e.id));
    if (r) window.location.assign(r.url);
  }

  return (
    <Dialog title={`Export ${p.name}`} onClose={onClose} wide>
      <p className="fine">The files of one branch as they are now, as plain files or one .zip. People can keep working while it runs; their later changes are not included.</p>
      {(targets.loading || branches.loading) && <Loading what="Loading" />}
      {targets.error !== undefined && <ErrorNote error={targets.error} doing="load the export targets" />}
      {targets.data && branches.data && (
        <>
          <div className="grid2" style={{ gap: 14 }}>
            <label className="field">
              <span>Branch</span>
              <select className="select" value={branch || main?.id || ''} onChange={(e) => setBranch(e.target.value)}>
                {branches.data.filter((b) => !b.archived).map((b) => <option key={b.id} value={b.id}>{b.name}{b.parent_id ? '' : ' (main line)'}</option>)}
              </select>
              {chosen && <small>Up to change {chosen.head_change}.</small>}
            </label>
            <label className="field">
              <span>Export to</span>
              <select className="select" value={target} onChange={(e) => setTarget(e.target.value)}>
                {targets.data.map((x) => <option key={x.name} value={x.name}>{x.name === 'server' ? 'This server' : `${x.name} (${KIND[x.kind] || x.kind})`}</option>)}
              </select>
              <small>{t?.description}{targets.data.length === 1 ? '. More places (a NAS, an S3 bucket) can be added in the server settings: MP_EXPORT_TARGETS.' : ''}</small>
            </label>
          </div>
          <fieldset className="field">
            <legend>As</legend>
            <label className="check"><input type="radio" name="fmt" checked={format === 'zip'} onChange={() => setFormat('zip')} /> One .zip file{target === 'server' ? ', to download from here' : ''}</label>
            <label className="check"><input type="radio" name="fmt" checked={format === 'files'} onChange={() => setFormat('files')} /> Plain files in a folder, as in the project</label>
          </fieldset>
          <label className="check">
            <input type="checkbox" checked={archiveAfter} onChange={(e) => setArchiveAfter(e.target.checked)} />
            Archive the project when the export has finished
          </label>
          <div className="actions">
            <button className="btn" onClick={onClose}>Close</button>
            <button className="btn pri" onClick={start} disabled={busy === 'start' || live}>{live ? 'An export is running' : 'Start export'}</button>
          </div>
        </>
      )}
      <h3 style={{ marginTop: 18 }}>Exports of {p.name}</h3>
      {list.data && !list.data.length && <p className="fine">None yet.</p>}
      {list.data && list.data.length > 0 && (
        <ul className="exports">
          {list.data.map((e) => {
            const pct = e.bytes_total ? Math.min(100, Math.floor((e.bytes_done / e.bytes_total) * 100)) : e.status === 'done' ? 100 : 0;
            return (
              <li key={e.id} className={e.status}>
                <div className="what">
                  <strong>{e.name}{e.format === 'zip' ? '.zip' : '/'}</strong>
                  <small>
                    {e.branch_name}, change {e.change} · to {e.target === 'server' ? 'this server' : e.target} · {since(e.created_at)}
                    {e.created_by ? ` by ${e.created_by.name}` : ''}{e.archive_after ? ' · then archive' : ''}
                  </small>
                  {(e.status === 'running' || e.status === 'queued') && (
                    <>
                      <i className="pbar"><b style={{ width: `${pct}%` }} /></i>
                      <small>{e.status === 'queued' ? 'Starting' : `${pct}% · ${size(e.bytes_done)} of ${size(e.bytes_total)} · ${e.files_done} of ${plural(e.files_total, 'file')}`}</small>
                    </>
                  )}
                  {e.status === 'done' && <small>{plural(e.files_total, 'file')}, {size(e.bytes_total)}</small>}
                  {e.status === 'failed' && <small className="bad">{e.error}</small>}
                </div>
                <div className="acts">
                  {e.downloadable && <button className="btn sm" disabled={busy === 'dl:' + e.id} onClick={() => download(e)}>Download</button>}
                  <span className={`badge ${e.status === 'done' ? 'good' : e.status === 'failed' ? 'bad' : 'hold'}`}>{{ queued: 'Starting', running: 'Exporting', done: 'Done', failed: 'Failed' }[e.status]}</span>
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </Dialog>
  );
}
