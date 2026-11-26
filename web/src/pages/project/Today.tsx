import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../../api/client';
import type { Change, ChangeFile } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { useSession } from '../../lib/session';
import { clock, dayLabel, firstName, plural, size } from '../../lib/format';
import { Empty, ErrorNote, Loading, Thumb, Who } from '../../components/ui';
import { useHref, useProject, ViewHead } from './context';

const PAGE = 25;

export default function Today() {
  const { project, branch, ws, v } = useProject();
  const { me } = useSession();
  const href = useHref();
  const first = useAsync(() => api.changes(project.id, { branch: branch.id, limit: PAGE }), [project.id, branch.id, v.changes]);
  const [older, setOlder] = useState<Change[]>([]);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [done, setDone] = useState(false);
  const [files, setFiles] = useState<Record<number, ChangeFile[]>>({});

  useEffect(() => {
    setOlder([]);
    setDone(false);
  }, [project.id, branch.id]);

  const list = [...(first.data || []), ...older];

  // file strips for the newest few changes
  useEffect(() => {
    const want = list.slice(0, 8).filter((c) => !files[c.number]);
    if (!want.length) return;
    let live = true;
    Promise.all(want.map((c) => api.change(project.id, c.number).then((d) => [c.number, d.files] as const).catch(() => [c.number, []] as const))).then((r) => {
      if (live) setFiles((f) => ({ ...f, ...Object.fromEntries(r) }));
    });
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [first.data, project.id]);

  async function more() {
    const last = list[list.length - 1];
    if (!last) return;
    setLoadingOlder(true);
    try {
      const r = await api.changes(project.id, { branch: branch.id, before: last.number, limit: PAGE });
      setOlder((o) => [...o, ...r]);
      if (r.length < PAGE) setDone(true);
    } finally {
      setLoadingOlder(false);
    }
  }

  const onBranch = !ws || ws.branch_id === branch.id;
  const fresh = (c: Change) => !!ws && onBranch && c.number > ws.synced_change && c.author.id !== me?.id;
  const freshCount = list.filter(fresh).length;

  let sub: string;
  if (!list.length) sub = 'Nothing published here yet.';
  else if (ws && onBranch)
    sub = freshCount
      ? `${plural(freshCount, 'change')} since you last got latest.`
      : 'You have everything. Changes from the team show up here as they are published.';
  else sub = `The latest changes on ${branch.name}, newest first.`;

  let lastDay = '';
  return (
    <>
      <ViewHead title={`Today in ${project.name}`} sub={sub} />
      <div className="view">
        {first.loading && !first.data && <Loading what="Loading changes" />}
        {first.error !== undefined && <ErrorNote error={first.error} doing="load the latest changes" retry={first.reload} />}
        {first.data && !list.length && (
          <Empty title="No changes yet">Publish from the desktop app or the CLI and the first change appears here.</Empty>
        )}
        {list.map((c) => {
          const day = dayLabel(c.created_at);
          const showDay = day !== lastDay;
          lastDay = day;
          const fs = files[c.number];
          const isFresh = fresh(c);
          return (
            <div key={c.number}>
              {showDay && <div className="day">{day}</div>}
              <Link to={href(`changes/${c.number}`)} className={`chg ${isFresh ? 'fresh' : ''}`}>
                <div className="n">
                  {c.number}
                  <small>{isFresh ? 'New for you' : ws && onBranch ? 'You have it' : c.kind === 'merge' ? 'Merge' : c.kind === 'update' ? 'Update' : ''}</small>
                </div>
                <div className="body">
                  <h2>{c.message}</h2>
                  <div className="by">
                    <Who id={c.author.id} name={c.author.name} />
                    {c.author.id === me?.id ? 'You' : firstName(c.author.name)} at {clock(c.created_at)}
                    <span>· {plural(c.file_count, 'file')}, {size(c.size)}</span>
                    {c.kind === 'merge' && c.merged_from?.length ? <span className="kindtag">Merge of {plural(c.merged_from.length, 'change')}</span> : null}
                    {c.kind === 'update' && <span className="kindtag">Update from parent</span>}
                  </div>
                  {fs && fs.length > 0 && (
                    <div className="strip">
                      {fs.slice(0, 4).map((f) => (
                        <Thumb key={f.path} seed={f.path} />
                      ))}
                      {fs.length > 4 && <div className="more">{fs.length - 4} more</div>}
                    </div>
                  )}
                </div>
              </Link>
            </div>
          );
        })}
        {list.length >= PAGE && !done && (
          <button className="btn sm more-btn" onClick={more} disabled={loadingOlder}>{loadingOlder ? 'Loading' : 'Show older changes'}</button>
        )}
      </div>
    </>
  );
}
