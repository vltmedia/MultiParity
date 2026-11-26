import { useState } from 'react';
import { api } from '../../api/client';
import type { DraftDetail } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { baseName, dayLabel, plural, size, when } from '../../lib/format';
import { Empty, ErrorNote, Loading, Thumb, useAction, useUi } from '../../components/ui';
import { useProject, ViewHead } from './context';

export default function Drafts() {
  const { project, branches, ws, v, bump } = useProject();
  const list = useAsync(() => api.drafts(project.id), [project.id, v.drafts]);
  const [open, setOpen] = useState<Record<string, DraftDetail | 'loading'>>({});
  const { confirm, toast } = useUi();
  const { run, busy } = useAction();

  async function show(id: string) {
    if (open[id]) {
      const n = { ...open };
      delete n[id];
      setOpen(n);
      return;
    }
    setOpen((o) => ({ ...o, [id]: 'loading' }));
    const d = await run('open', 'open the draft', () => api.draft(project.id, id));
    setOpen((o) => {
      const n = { ...o };
      if (d) n[id] = d;
      else delete n[id];
      return n;
    });
  }

  async function remove(id: string, title: string, files: number) {
    const ok = await confirm({
      title: `Delete the draft “${title}”?`,
      what: <>The draft and its {plural(files, 'file')} are removed from the server. Files on your computer are not touched. This cannot be undone from the web.</>,
      confirm: 'Delete draft',
      danger: true,
    });
    if (!ok) return;
    const r = await run('del:' + id, 'delete the draft', () => api.deleteDraft(project.id, id));
    if (r) {
      toast(`Deleted the draft “${title}”.`);
      bump('drafts');
    }
  }

  const branchName = (id: string) => branches.find((b) => b.id === id)?.name || 'a branch';
  return (
    <>
      <ViewHead title="Your drafts" sub="Unfinished work saved on the server without publishing. Only you can see these." />
      <div className="view">
        {list.loading && !list.data && <Loading what="Loading drafts" />}
        {list.error !== undefined && <ErrorNote error={list.error} doing="load your drafts" retry={list.reload} />}
        {list.data && !list.data.length && (
          <Empty title="No drafts">
            {ws ? 'Use Save draft in Your work to keep unfinished work on the server, as a backup and so you can pick it up on another computer.' : 'Save draft in the desktop app keeps unfinished work on the server without publishing it.'}
          </Empty>
        )}
        {list.data?.map((d) => {
          const o = open[d.id];
          return (
            <article key={d.id} className="chg">
              <div className="n">
                Draft
                <small>{dayLabel(d.created_at)}</small>
              </div>
              <div className="body">
                <h2>{d.title}</h2>
                <div className="by">
                  Saved {when(d.created_at)} on {branchName(d.branch_id)}. {plural(d.file_count, 'file')}, {size(d.size)}.
                </div>
                {o && o !== 'loading' && (
                  <>
                    <div className="strip">
                      {o.entries.slice(0, 6).map((e) => (
                        <Thumb key={e.path} seed={e.path} />
                      ))}
                    </div>
                    <ul style={{ marginTop: 10 }}>
                      {o.entries.map((e) => (
                        <li key={e.path} className="fine" title={e.path}>
                          {e.op === 'delete' ? 'Deletes ' : e.prev_file_hash ? 'Edits ' : 'Adds '}
                          <b style={{ color: 'var(--ink)' }}>{baseName(e.path)}</b> in {e.path.split('/').slice(0, -1).join('/') || 'the top folder'}
                          {e.manifest ? `, ${size(e.manifest.size)}` : ''}
                        </li>
                      ))}
                    </ul>
                  </>
                )}
                {o === 'loading' && <Loading what="Opening the draft" />}
                <div className="row" style={{ marginTop: 14 }}>
                  <button className="btn sm" onClick={() => show(d.id)} aria-expanded={!!o}>{o ? 'Hide files' : 'Show files'}</button>
                  <button className="btn sm danger" disabled={busy === 'del:' + d.id} onClick={() => remove(d.id, d.title, d.file_count)}>Delete draft</button>
                </div>
                <p className="fine" style={{ marginTop: 8 }}>To pick it back up, open the draft in the desktop app; it brings the files into your folder.</p>
              </div>
            </article>
          );
        })}
      </div>
    </>
  );
}
