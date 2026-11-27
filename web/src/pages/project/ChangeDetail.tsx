import { useMemo, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { api, downloadFile } from '../../api/client';
import { useAsync } from '../../lib/useAsync';
import { baseName, dateTime, dirName, firstName, plural, size } from '../../lib/format';
import { Empty, ErrorNote, FileThumb, Loading, Who, useUi } from '../../components/ui';
import { useHref, useProject, ViewHead } from './context';

const OP: Record<string, string> = { add: 'Added', edit: 'Edited', delete: 'Deleted' };

export default function ChangeDetail() {
  const { number = '' } = useParams();
  const n = Number(number);
  const { project, branches, ws } = useProject();
  const href = useHref();
  const { fail } = useUi();
  const c = useAsync(() => api.change(project.id, n), [project.id, n]);
  const [q, setQ] = useState('');

  const files = useMemo(() => {
    const t = q.trim().toLowerCase();
    return (c.data?.files || []).filter((f) => !t || f.path.toLowerCase().includes(t)).sort((a, b) => a.path.localeCompare(b.path));
  }, [c.data, q]);

  if (!Number.isFinite(n)) return <div className="view"><Empty title="That is not a change number" /></div>;
  if (c.loading && !c.data) return <div className="view" style={{ paddingTop: 28 }}><Loading what={`Loading change ${n}`} /></div>;
  if (c.error !== undefined) return <div className="view" style={{ paddingTop: 28 }}><ErrorNote error={c.error} doing={`open change ${n}`} retry={c.reload} /></div>;
  const ch = c.data!;
  const br = branches.find((b) => b.id === ch.branch_id);
  const counts = { add: 0, edit: 0, delete: 0 } as Record<string, number>;
  ch.files.forEach((f) => (counts[f.op] += 1));
  const haveIt = ws && ws.branch_id === ch.branch_id ? ch.number <= ws.synced_change : undefined;

  return (
    <>
      <ViewHead title={`Change ${ch.number}`} sub={ch.message}>
        <Link className="btn sm" to={href('today')}>Back to Today</Link>
      </ViewHead>
      <div className="view">
        <div className="facts">
          <div>
            By
            <b className="row" style={{ gap: 8 }}><Who id={ch.author.id} name={ch.author.name} />{ch.author.name}</b>
          </div>
          <div>When<b>{dateTime(ch.created_at)}</b></div>
          <div>On<b>{br?.name || 'A branch'}</b></div>
          <div>Files<b>{plural(ch.file_count, 'file')}, {size(ch.size)}</b></div>
          {haveIt !== undefined && <div>Your copy<b>{haveIt ? 'You have it' : 'Not yet, get latest'}</b></div>}
        </div>
        {ch.kind === 'merge' && (
          <div className="notice" style={{ marginBottom: 14 }}>
            A merge. It brings in {ch.merged_from?.length ? plural(ch.merged_from.length, 'change') : 'changes'} from another branch
            {ch.merged_from?.length ? <>: {ch.merged_from.map((m, i) => <span key={m}>{i ? ', ' : ' '}<Link className="link" to={href(`changes/${m}`)}>{m}</Link></span>)}.</> : '.'}
          </div>
        )}
        {ch.kind === 'update' && <div className="notice" style={{ marginBottom: 14 }}>An update: brings the parent branch's new changes into {br?.name || 'this branch'}.</div>}
        <div className="filebar">
          <label>
            <span className="sr">Find a file in this change</span>
            <input className="input sm" type="search" placeholder="Find a file in this change" value={q} onChange={(e) => setQ(e.target.value)} />
          </label>
          <span className="fine">
            {[counts.add && `${counts.add} added`, counts.edit && `${counts.edit} edited`, counts.delete && `${counts.delete} deleted`].filter(Boolean).join(', ')}
          </span>
        </div>
        {!files.length && <Empty title={q ? 'No files match' : 'No files listed'} />}
        {files.length > 0 && (
          <table className="t">
            <thead>
              <tr>
                <th>Name</th>
                <th>Folder</th>
                <th>What happened</th>
                <th className="r">Size</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {files.map((f) => (
                <tr key={f.path}>
                  <td className="name" title={f.path}><FileThumb projectId={project.id} hash={f.op === 'delete' ? null : f.file_hash} path={f.path} />{baseName(f.path)}</td>
                  <td className="mute">
                    {dirName(f.path) ? <Link className="rowlink" to={href('files', { prefix: dirName(f.path) })}>{dirName(f.path)}</Link> : 'Top folder'}
                  </td>
                  <td><span className={`op ${f.op}`}>{OP[f.op]}</span></td>
                  <td className="r">{f.op === 'delete' ? '' : size(f.size)}</td>
                  <td className="r">
                    {f.op !== 'delete' && (
                      <button className="link fine" onClick={() => downloadFile(project.id, ch.branch_id, f.path).catch((e) => fail(e, `download ${baseName(f.path)}`))}>
                        Download latest
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <p className="fine" style={{ marginTop: 12 }}>
          Published by {firstName(ch.author.name)}. Downloads give the file as it is now on {br?.name || 'the branch'}; to bring a file back to this change, use Rewind in the desktop app.
        </p>
      </div>
    </>
  );
}
