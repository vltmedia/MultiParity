import { useEffect, useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { api } from '../../api/client';
import type { Branch, MergeReport } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { baseName, firstName, plural, when } from '../../lib/format';
import { ErrorNote, Loading, Thumb, useAction, useUi } from '../../components/ui';
import { useProject, ViewHead } from './context';

export default function Branches() {
  const { project, branches, branch, setBranch, ws, reloadBranches, bump } = useProject();
  const [params, setParams] = useSearchParams();
  const [showGone, setShowGone] = useState(false);
  const [making, setMaking] = useState(false);
  const live = branches.filter((b) => !b.archived);
  const gone = branches.filter((b) => b.archived);
  const lookId = params.get('look') || (branch.parent_id ? branch.id : live.find((b) => b.parent_id)?.id || '');
  const look = branches.find((b) => b.id === lookId);
  const byId = (id: string | null) => branches.find((b) => b.id === id);
  const kids = (b: Branch) => live.filter((x) => x.parent_id === b.id);

  const pick = (id: string) => {
    const n = new URLSearchParams(params);
    n.set('look', id);
    setParams(n, { replace: true });
  };

  // show the tree: Main, then its children, depth-first
  const ordered: Array<{ b: Branch; depth: number }> = [];
  const walk = (b: Branch, depth: number) => {
    ordered.push({ b, depth });
    kids(b).forEach((k) => walk(k, depth + 1));
  };
  live.filter((b) => !b.parent_id || !byId(b.parent_id) || byId(b.parent_id)?.archived).forEach((b) => walk(b, 0));

  return (
    <>
      <ViewHead title={`Branches of ${project.name}`} sub="A branch is a copy that changes on its own until you merge it back. Making one takes a second and no extra space." />
      <div className="view">
        <div className="split">
          <div className="panel">
            <ul className="brs">
              {ordered.map(({ b, depth }) => {
                const parent = byId(b.parent_id);
                const here = ws?.branch_id === b.id;
                return (
                  <li key={b.id} className={`${b.id === branch.id ? 'here' : ''} ${look?.id === b.id ? 'picked' : ''}`} style={{ paddingLeft: 14 + depth * 18 }}>
                    <div>
                      <strong>{b.name}</strong>
                      <small>
                        {parent
                          ? `From ${parent.name}, ${when(b.created_at)}. Made by ${firstName(b.created_by.name)}.`
                          : `Where finished work lives. At change ${b.head_change}.`}
                      </small>
                      {parent && (
                        <div className="chips">
                          <span><b>{b.ahead}</b> {b.ahead === 1 ? 'file' : 'files'} to merge</span>
                          <span>{parent.name} has <b>{b.behind}</b> new</span>
                          <span>{b.shares_claims ? `Shares claims with ${parent.name}` : 'Own claims'}</span>
                          {b.protected && <span>Protected</span>}
                        </div>
                      )}
                      {!parent && b.protected && <div className="chips"><span>Protected: work arrives by merge</span></div>}
                    </div>
                    <div className="row">
                      {b.id === branch.id ? <em>Viewing</em> : <button className="btn sm" onClick={() => setBranch(b.id)} title="Show this branch here. Your folder stays where it is until you switch in the desktop app.">Switch</button>}
                      {parent && look?.id !== b.id && <button className="btn sm" onClick={() => pick(b.id)}>Merge</button>}
                      {here && <em>Your folder</em>}
                    </div>
                  </li>
                );
              })}
              {showGone &&
                gone.map((b) => (
                  <li key={b.id} className="gone">
                    <div>
                      <strong>{b.name}</strong>
                      <small>Archived. From {byId(b.parent_id)?.name || 'Main'}, still in history.</small>
                    </div>
                    <button className="btn sm" onClick={() => setBranch(b.id)}>Show</button>
                  </li>
                ))}
            </ul>
            <div className="row">
              <button className="btn sm" onClick={() => setMaking((m) => !m)} aria-expanded={making}>New branch from {branch.name}</button>
              {gone.length > 0 && (
                <button className="btn sm ghost" onClick={() => setShowGone((s) => !s)}>{showGone ? 'Hide archived' : `Show ${plural(gone.length, 'archived branch', 'archived branches')}`}</button>
              )}
            </div>
            {making && <NewBranch from={branch} onDone={(b) => { setMaking(false); reloadBranches(); bump('branches'); setBranch(b.id); }} onCancel={() => setMaking(false)} />}
          </div>
          {look && look.parent_id ? (
            <MergePanel key={look.id} b={look} parent={byId(look.parent_id)!} />
          ) : (
            <div className="panel">
              <h2>Merging</h2>
              <p>Pick a branch on the left to see what merging it would do. A branch merges only into the branch it came from, and has to be up to date with it first.</p>
            </div>
          )}
        </div>
      </div>
    </>
  );
}

function NewBranch({ from, onDone, onCancel }: { from: Branch; onDone: (b: Branch) => void; onCancel: () => void }) {
  const { project } = useProject();
  const { run, busy } = useAction();
  const { toast } = useUi();
  const [name, setName] = useState('');
  const [shares, setShares] = useState(true);
  const [err, setErr] = useState('');
  async function make() {
    if (!name.trim()) return setErr('Give the branch a name, like Wolf rework or Demo build.');
    const b = await run('make', 'make the branch', () => api.createBranch(project.id, name.trim(), from.id, shares));
    if (b) {
      toast(`Made the branch ${b.name} from ${from.name}.`);
      onDone(b);
    }
  }
  return (
    <div className="newbranch">
      <label className="field">
        <span>Branch name</span>
        <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Wolf rework" autoFocus onKeyDown={(e) => e.key === 'Enter' && make()} />
      </label>
      <label className="check-label">
        <input type="checkbox" checked={shares} onChange={(e) => setShares(e.target.checked)} /> Share claims with {from.name}
      </label>
      <p className="fine">
        {shares
          ? `A file claimed here is claimed on ${from.name} too, so two people cannot edit the same model on both. Recommended.`
          : `Claims are separate. Good for a release or demo that must keep moving; when merging, files changed on both sides need a choice per file.`}
      </p>
      {err && <div className="notice bad">{err}</div>}
      <div className="row">
        <button className="btn sm" onClick={onCancel}>Cancel</button>
        <button className="btn pri sm" onClick={make} disabled={!!busy}>Make branch</button>
      </div>
    </div>
  );
}

function MergePanel({ b, parent }: { b: Branch; parent: Branch }) {
  const { project, v, bump, reloadBranches, main } = useProject();
  const navigate = useNavigate();
  const { toast, confirm } = useUi();
  const { run, busy } = useAction();
  const preview = useAsync(() => api.mergePreview(project.id, b.id), [project.id, b.id, v.changes, v.branches]);
  const [report, setReport] = useState<MergeReport | undefined>();
  const [choice, setChoice] = useState<Record<string, 'mine' | 'parent'>>({});
  const [message, setMessage] = useState(`Merge ${b.name} into ${parent.name}`);
  useEffect(() => setReport(preview.data), [preview.data]);

  const r = report;
  const conflicts = r?.conflicts || [];
  const binary = conflicts.filter((c) => c.kind === 'binary');
  const allChosen = conflicts.every((c) => choice[c.path]);

  async function update() {
    const out = await run('update', `update from ${parent.name}`, () => api.updateBranch(project.id, b.id, conflicts.length ? choice : undefined));
    if (!out) return;
    setReport(out);
    if (out.conflicts.length) toast(`${plural(out.conflicts.length, 'file')} changed on both sides. Choose which to keep for each.`, 'hold');
    else toast(`Updated ${b.name} from ${parent.name}.`);
    bump('changes');
    reloadBranches();
  }

  async function merge() {
    if (!r) return;
    const ok = await confirm({
      title: `Merge ${b.name} into ${parent.name}?`,
      what: (
        <>
          {plural(r.only_here, 'file')} from {b.name} go into {parent.name} as one new change, {project.head_change + 1}, listing the changes it came from.
          Everyone on {parent.name} gets them the next time they get latest.
        </>
      ),
      detail: 'The branch stays as it is. You can archive it afterwards; it stays in history.',
      confirm: `Merge into ${parent.name}`,
    });
    if (!ok) return;
    const out = await run('merge', `merge into ${parent.name}`, () => api.merge(project.id, b.id, message.trim() || `Merge ${b.name} into ${parent.name}`));
    if (!out) return;
    toast(`Merged into ${parent.name} as change ${out.change.number}.`);
    bump('changes');
    bump('branches');
    navigate(`/p/${project.id}/changes/${out.change.number}${parent.id === main.id ? '' : `?branch=${parent.id}`}`);
  }

  async function archive() {
    const ok = await confirm({
      title: `Archive ${b.name}?`,
      what: <>{b.name} is hidden from branch lists and nobody can publish to it. Its changes stay in history and can still be opened.</>,
      detail: b.ahead ? `It still has ${plural(b.ahead, 'file')} not merged into ${parent.name}.` : undefined,
      confirm: 'Archive branch',
      danger: b.ahead > 0,
    });
    if (!ok) return;
    const out = await run('archive', 'archive the branch', () => api.archiveBranch(project.id, b.id));
    if (out) {
      toast(`Archived ${b.name}.`);
      bump('branches');
    }
  }

  return (
    <div className="panel">
      <h2>Merge {b.name} into {parent.name}</h2>
      <p>This becomes one change on {parent.name}, {project.head_change + 1}, listing the changes it came from.</p>
      {preview.loading && !r && <Loading what="Working out what would merge" />}
      {preview.error !== undefined && <ErrorNote error={preview.error} doing="work out the merge" retry={preview.reload} />}
      {r && (
        <>
          <ul className="adopt">
            {r.up_to_date ? (
              <li style={{ ['--k' as string]: 'var(--new)' }}>
                <b>Done</b>
                <div><strong>Up to date with {parent.name}</strong><small>Nothing new on {parent.name} to bring in. A branch has to be up to date before it can merge.</small></div>
              </li>
            ) : (
              <li style={{ ['--k' as string]: 'var(--acc)' }}>
                <b>{r.only_parent}</b>
                <div>
                  <strong>New on {parent.name}, not here yet</strong>
                  <small>Update from {parent.name} first, so any clashes are sorted out here and never on {parent.name}.</small>
                  <div className="row" style={{ marginTop: 8 }}>
                    <button className="btn sm" onClick={update} disabled={!!busy || (conflicts.length > 0 && !allChosen)}>
                      {busy === 'update' ? 'Updating' : conflicts.length ? 'Update with these choices' : `Update from ${parent.name}`}
                    </button>
                  </div>
                </div>
              </li>
            )}
            <li>
              <b>{r.only_here}</b>
              <div><strong>Changed only on this branch</strong><small>Go in as they are.</small></div>
            </li>
            <li>
              <b>{r.auto_merged.length}</b>
              <div>
                <strong>Text files changed on both</strong>
                <small>{r.auto_merged.length ? `Merged automatically: ${r.auto_merged.map(baseName).slice(0, 3).join(', ')}${r.auto_merged.length > 3 ? ' and more' : ''}.` : 'None.'}</small>
              </div>
            </li>
            <li style={{ ['--k' as string]: binary.length ? 'var(--bad)' : 'var(--line)' }}>
              <b>{binary.length}</b>
              <div>
                <strong>Models and textures changed on both</strong>
                <small>{b.shares_claims ? `Cannot happen here: this branch shares claims with ${parent.name}.` : binary.length ? 'Pick which one to keep for each. The other stays in history.' : 'None. This branch has its own claims, so this could happen.'}</small>
              </div>
            </li>
          </ul>
          {conflicts.map((c) => (
            <div key={c.path} className="versus">
              <Thumb seed={c.path + b.id} />
              <div>
                <strong>{baseName(c.path)}</strong>
                <small>{c.kind === 'binary' ? 'Changed on both sides. Keep one; the other stays in history.' : 'Overlapping edits. Keep one side, or merge them by hand in the desktop app.'}</small>
                <div className="seg sm" role="radiogroup" aria-label={`Which ${baseName(c.path)} to keep`}>
                  <button role="radio" aria-pressed={choice[c.path] === 'mine'} aria-checked={choice[c.path] === 'mine'} onClick={() => setChoice({ ...choice, [c.path]: 'mine' })}>Keep this branch's</button>
                  <button role="radio" aria-pressed={choice[c.path] === 'parent'} aria-checked={choice[c.path] === 'parent'} onClick={() => setChoice({ ...choice, [c.path]: 'parent' })}>Keep {parent.name}'s</button>
                </div>
              </div>
              <Thumb seed={c.path + parent.id} />
            </div>
          ))}
          {r.up_to_date && (
            <label className="field">
              <span>Message for the merge</span>
              <input className="input" value={message} onChange={(e) => setMessage(e.target.value)} />
            </label>
          )}
          {b.protected && <p className="fine">{b.name} is protected. Merging out of it is fine; publishing into it needs permission.</p>}
          {parent.protected && <p className="fine">{parent.name} is protected: only people with permission can merge into it.</p>}
          <div className="row">
            <Link className="btn" to={`/p/${project.id}/today?branch=${b.id}`}>See its changes</Link>
            <button className="btn pri grow" disabled={!r.up_to_date || r.only_here === 0 || !!busy} onClick={merge}>
              {busy === 'merge' ? 'Merging' : `Merge into ${parent.name}`}
            </button>
          </div>
          {!r.up_to_date && <p className="fine">Merge into {parent.name} unlocks once the branch is up to date.</p>}
          {r.up_to_date && r.only_here === 0 && <p className="fine">Nothing to merge: {b.name} has no changes that {parent.name} does not have.</p>}
          <button className="btn sm ghost" style={{ alignSelf: 'flex-start' }} onClick={archive} disabled={!!busy}>Archive this branch</button>
        </>
      )}
    </div>
  );
}
