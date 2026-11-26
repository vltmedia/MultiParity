import type { CSSProperties, ReactNode } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { api } from '../../api/client';
import { useAsync } from '../../lib/useAsync';
import { useSession } from '../../lib/session';
import { baseName, firstName } from '../../lib/format';
import { Empty, ErrorNote, Loading, Thumb } from '../../components/ui';
import { useHref, useProject, ViewHead } from './context';

const SPANS = [10, 20, 40];

export default function HistoryView() {
  const { project, branch, ws, status, claims, v } = useProject();
  const { me } = useSession();
  const href = useHref();
  const [params, setParams] = useSearchParams();
  const prefix = (params.get('prefix') || '').replace(/^\/+|\/+$/g, '');
  const limit = Number(params.get('span')) || 10;
  const h = useAsync(
    () => api.history(project.id, { branch: branch.id, prefix: prefix ? prefix + '/' : undefined, limit }),
    [project.id, branch.id, prefix, limit, v.changes],
  );

  const onBranch = !!ws && ws.branch_id === branch.id;
  const changes = h.data?.changes || [];
  const hasNext = onBranch;
  const cols = changes.length + (hasNext ? 1 : 0);
  const nowIdx = onBranch ? changes.reduce((acc, c, i) => (c <= ws!.synced_change ? i : acc), -1) : -1;
  const base = prefix ? prefix + '/' : '';
  const edited = new Set(onBranch ? status?.edited.map((f) => f.path) : []);
  const news = onBranch ? (status?.new || []).filter((f) => f.path.startsWith(base)) : [];
  const grid = { gridTemplateColumns: `repeat(${Math.max(cols, 1)}, minmax(0, 1fr))` };

  const setSpan = (n: number) => {
    const p = new URLSearchParams(params);
    p.set('span', String(n));
    setParams(p, { replace: true });
  };

  return (
    <>
      <ViewHead
        title={`History of ${prefix ? baseName(prefix) : project.name}`}
        sub={
          onBranch
            ? 'Each column is a change. The dark line is where your copy is; Next is your work that is not published yet.'
            : `Each column is a change on ${branch.name}. Each row is a file, marked where it changed.`
        }
      >
        <div className="seg sm" role="group" aria-label="How many changes">
          {SPANS.map((n) => (
            <button key={n} aria-pressed={limit === n} onClick={() => setSpan(n)}>Last {n}</button>
          ))}
        </div>
      </ViewHead>
      <div className="view">
        {prefix && (
          <div className="crumb">
            <Link to={href('history', { span: String(limit) })}>{project.name}</Link>
            {prefix.split('/').map((c, i, all) => (
              <span key={i} style={{ display: 'contents' }}>
                <span className="sep">/</span>
                {i === all.length - 1 ? <span>{c}</span> : <Link to={href('history', { prefix: all.slice(0, i + 1).join('/'), span: String(limit) })}>{c}</Link>}
              </span>
            ))}
          </div>
        )}
        <div className="key">
          <span><u style={{ background: '#c3b8d3' }} />Changed</span>
          {onBranch && <span><u style={{ border: '1.5px solid var(--ink)' }} />You do not have it yet</span>}
          <span><u style={{ background: '#b497ff' }} />Someone is working</span>
          {onBranch && <span><u style={{ background: '#ff9a5e' }} />Yours</span>}
          {onBranch && <span><u style={{ background: 'var(--new)' }} />New</span>}
        </div>
        {h.loading && !h.data && <Loading what="Loading history" />}
        {h.error !== undefined && <ErrorNote error={h.error} doing="load the history" retry={h.reload} />}
        {h.data && !h.data.files.length && !news.length && <Empty title="No history here yet">Nothing in this folder has been published.</Empty>}
        {h.data && (h.data.files.length > 0 || news.length > 0) && (
          <div className="tl" role="table" aria-label="Files by change">
            <div className="hd l" role="columnheader">File</div>
            <div className="hd ruler" style={grid} role="row">
              {changes.map((c, i) => (
                <Link key={c} to={href(`changes/${c}`)} className={i === nowIdx ? 'now' : ''} title={i === nowIdx ? `Your copy is at ${ws?.synced_change}` : `Change ${c}`}>
                  {c}
                </Link>
              ))}
              {hasNext && <span className="next">Next</span>}
            </div>
            {news.map((f) => (
              <Lane key={'n:' + f.path} path={f.path} label={baseName(f.path)} grid={grid} nowIdx={nowIdx} cols={cols}>
                <i className="new" style={{ gridColumn: `${cols} / ${cols + 1}`, gridRow: 1 }}>New</i>
              </Lane>
            ))}
            {h.data.files.map((f) => {
              const claim = claims.find((c) => c.path === f.path);
              const theirs = claim && claim.user.id !== me?.id;
              const mine = (claim && claim.user.id === me?.id) || edited.has(f.path);
              const idx = f.changes.map((c) => changes.indexOf(c)).filter((i) => i >= 0);
              const last = idx.length ? Math.max(...idx) : -1;
              return (
                <Lane key={f.path} path={f.path} label={baseName(f.path)} grid={grid} nowIdx={nowIdx} cols={cols}>
                  {idx.map((i) => (
                    <i key={i} className={onBranch && changes[i] > ws!.synced_change && !mine ? 'miss' : ''} style={{ gridColumn: i + 1, gridRow: 1 }} title={`Changed in ${changes[i]}`} />
                  ))}
                  {theirs && (
                    <i className="hold" style={{ gridColumn: `${Math.min(last + 2, cols)} / ${cols + 1}`, gridRow: 1 }}>
                      {firstName(claim!.user.name)} is working
                    </i>
                  )}
                  {mine && hasNext && (
                    <i className="mine" style={{ gridColumn: `${cols} / ${cols + 1}`, gridRow: 1 }}>{edited.has(f.path) ? 'Edited' : 'Claimed'}</i>
                  )}
                </Lane>
              );
            })}
          </div>
        )}
      </div>
    </>
  );
}

function Lane({ path, label, grid, nowIdx, cols, children }: { path: string; label: string; grid: CSSProperties; nowIdx: number; cols: number; children: ReactNode }) {
  return (
    <>
      <div className="trk" role="rowheader" title={path}>
        <Thumb seed={path} />
        <span className="el">{label}</span>
      </div>
      <div className="lane" style={grid} role="cell">
        {nowIdx >= 0 && <span className="nowline" style={{ left: `${((nowIdx + 1) / cols) * 100}%` }} aria-hidden="true" />}
        {children}
      </div>
    </>
  );
}
