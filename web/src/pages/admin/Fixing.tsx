import { useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../../api/client';
import type { CheckpointUsage, ClaimWithProject, GcDryRun, StorageCheckReport } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { baseName, firstName, held, plural, since, size } from '../../lib/format';
import { Empty, ErrorNote, Loading, Who, useAction, useUi } from '../../components/ui';
import { AdminHead, useAdmin } from './AdminLayout';

const AGES: Array<[number, string]> = [
  [24, 'More than a day'],
  [72, 'More than 3 days'],
  [168, 'More than a week'],
  [720, 'More than 30 days'],
];

export default function AdminFixing() {
  const { scope, isServerAdmin } = useAdmin();
  const [hours, setHours] = useState(72);
  const claims = useAsync(() => api.admin.claims({ older_than_hours: hours, org: scope || undefined }), [hours, scope]);
  const users = useAsync(() => api.admin.users({ org: scope || undefined, status: 'suspended' }), [scope]);
  const projects = useAsync(() => api.admin.projects(scope || undefined), [scope]);
  const { confirm, toast } = useUi();
  const { run, busy } = useAction();
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [checkProject, setCheckProject] = useState('');
  const [check, setCheck] = useState<StorageCheckReport | null>(null);
  const [gc, setGc] = useState<GcDryRun | null>(null);
  const [gcDone, setGcDone] = useState<{ deleted: number; bytes: number } | null>(null);
  const kept = useAsync(() => api.admin.checkpoints(scope || undefined), [scope]);
  const [cleared, setCleared] = useState<number | null>(null);

  const suspended = new Set((users.data || []).map((u) => u.id));
  const key = (c: ClaimWithProject) => `${c.project.id}:${c.branch_id}:${c.path}`;
  const list = claims.data || [];
  const picked = list.filter((c) => sel.has(key(c)));

  async function release() {
    const holders = [...new Set(picked.map((c) => firstName(c.user.name)))];
    const ok = await confirm({
      title: `Release ${plural(picked.length, 'claim')}?`,
      what: (
        <>
          {plural(picked.length, 'file')} become free to claim straight away. {holders.join(', ')} {holders.length === 1 ? 'is' : 'are'} told. Any edits they have stay on their computers, but they may not be able to publish them if someone else claims first.
        </>
      ),
      confirm: `Release ${plural(picked.length, 'claim')}`,
      danger: true,
    });
    if (!ok) return;
    const r = await run('release', 'release the claims', () => api.admin.releaseClaims(picked.map((c) => ({ project: c.project.id, branch: c.branch_id, path: c.path }))));
    if (r) {
      toast(`Released ${plural(r.released, 'claim')}.`);
      setSel(new Set());
      claims.reload();
    }
  }

  async function runCheck() {
    setCheck(null);
    const r = await run('check', 'check storage', () => api.admin.storageCheck(checkProject || undefined));
    if (r) setCheck(r);
  }
  async function dryRun() {
    setGc(null);
    setGcDone(null);
    const r = await run('gc', 'work out how much space can be reclaimed', () => api.admin.gcDryRun());
    if (r) setGc(r);
  }
  async function reclaim() {
    if (!gc) return;
    const ok = await confirm({
      title: `Reclaim ${size(gc.bytes)}?`,
      what: <>{plural(gc.unreferenced_chunks, 'chunk')} that no file revision, draft or checkpoint upload refers to are deleted from storage. Nothing anyone can open uses them. This cannot be undone.</>,
      detail: 'It never runs during a publish; if one starts, it waits.',
      confirm: `Reclaim ${size(gc.bytes)}`,
      danger: true,
    });
    if (!ok) return;
    const r = await run('gcrun', 'reclaim space', () => api.admin.gcRun());
    if (r) {
      setGcDone(r);
      setGc(null);
      toast(`Reclaimed ${size(r.bytes)}.`);
    }
  }

  const keptList = kept.data || [];
  const keptKey = (c: CheckpointUsage) => `${c.user.id}:${c.project.id}`;
  const keptCount = keptList.reduce((a, c) => a + c.checkpoints, 0);
  const keptBytes = keptList.reduce((a, c) => a + c.stored, 0);

  async function clearKept(c: CheckpointUsage, onlyAutomatic: boolean) {
    if (!onlyAutomatic) {
      const named = c.checkpoints - c.automatic;
      const ok = await confirm({
        title: `Clear all of ${firstName(c.user.name)}'s checkpoints in ${c.project.name}?`,
        what: (
          <>
            {c.checkpoints === 1 ? 'The 1 checkpoint' : `All ${plural(c.checkpoints, 'checkpoint')}`} {c.user.name} has on this server for {c.project.name} {c.checkpoints === 1 ? 'is' : 'are'} removed. That includes the ones {firstName(c.user.name)} named{named ? ` (${named.toLocaleString()})` : ''}, not only the automatic ones. This cannot be undone.
          </>
        ),
        detail: 'Space is freed the next time unused data is cleaned up.',
        confirm: `Clear ${plural(c.checkpoints, 'checkpoint')}`,
        danger: true,
      });
      if (!ok) return;
    }
    setCleared(null);
    const r = await run(
      `${onlyAutomatic ? 'cpauto' : 'cpall'}:${keptKey(c)}`,
      `clear ${firstName(c.user.name)}'s checkpoints`,
      () => api.admin.clearCheckpoints(onlyAutomatic ? { user: c.user.id, project: c.project.id, only_automatic: true } : { user: c.user.id, project: c.project.id }),
    );
    if (r) {
      setCleared(r.removed);
      toast(`Removed ${plural(r.removed, 'checkpoint')}. Space is freed the next time unused data is cleaned up.`);
      kept.reload();
    }
  }

  return (
    <>
      <AdminHead title="Fixing things" sub="Stuck claims, storage that does not add up, and space to reclaim." />
      <div className="view">
        <div className="stack">
          <section className="section">
            <h2>Stuck claims</h2>
            <p>Claims held for a long time, or by people who are suspended. Releasing one tells the holder.</p>
            <div className="toolbar">
              <label>
                <span className="sr">Held for</span>
                <select className="select sm" value={hours} onChange={(e) => { setHours(Number(e.target.value)); setSel(new Set()); }}>
                  {AGES.map(([h, l]) => <option key={h} value={h}>{l}</option>)}
                </select>
              </label>
              <span className="grow" />
              <button className="btn sm danger" disabled={!picked.length || busy === 'release'} onClick={release}>
                Release {picked.length ? plural(picked.length, 'claim') : 'selected'}
              </button>
            </div>
            {claims.loading && !claims.data && <Loading what="Looking for stuck claims" />}
            {claims.error !== undefined && <ErrorNote error={claims.error} doing="load claims" retry={claims.reload} />}
            {claims.data && !list.length && <Empty title="Nothing stuck">No claims older than that, and nobody suspended is holding files.</Empty>}
            {list.length > 0 && (
              <table className="t">
                <thead>
                  <tr>
                    <th className="c">
                      <input type="checkbox" aria-label="Select all" checked={picked.length === list.length} onChange={() => setSel(picked.length === list.length ? new Set() : new Set(list.map(key)))} />
                    </th>
                    <th>File</th>
                    <th>Project</th>
                    <th>Held by</th>
                    <th>For</th>
                  </tr>
                </thead>
                <tbody>
                  {list.map((c) => (
                    <tr key={key(c)} className={sel.has(key(c)) ? 'sel' : ''}>
                      <td className="c">
                        <input type="checkbox" aria-label={`Select ${baseName(c.path)}`} checked={sel.has(key(c))} onChange={() => { const n = new Set(sel); if (n.has(key(c))) n.delete(key(c)); else n.add(key(c)); setSel(n); }} />
                      </td>
                      <td className="name" title={c.path}>{baseName(c.path)}<br /><span className="fine">{c.path}</span></td>
                      <td>{c.project.name}</td>
                      <td>
                        <span className="row" style={{ gap: 8, flexWrap: 'nowrap' }}>
                          <Who id={c.user.id} name={c.user.name} />
                          <Link className="rowlink" to={`/admin/people/${c.user.id}`}>{c.user.name}</Link>
                          {suspended.has(c.user.id) && <span className="badge bad">Suspended</span>}
                        </span>
                      </td>
                      <td>{held(c.created_at)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>

          <div className="grid2">
            <section className="section">
              <h2>Storage check</h2>
              <p>Makes sure every revision's chunks exist and match their hash. It reads everything, so a whole-server check can take a while.</p>
              <div className="toolbar">
                <label>
                  <span className="sr">What to check</span>
                  <select className="select sm" value={checkProject} onChange={(e) => setCheckProject(e.target.value)}>
                    <option value="">{isServerAdmin && !scope ? 'The whole server' : 'Every project here'}</option>
                    {projects.data?.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
                  </select>
                </label>
                <button className="btn sm" onClick={runCheck} disabled={busy === 'check'}>{busy === 'check' ? 'Checking' : 'Check storage'}</button>
              </div>
              {check && (
                <div className={`result ${check.ok ? 'good' : 'bad'}`}>
                  <b className="big">{check.ok ? 'All good' : `${plural(check.missing.length, 'chunk')} missing`}</b>
                  <p>{check.checked.toLocaleString()} chunks checked.</p>
                  {!check.ok && (
                    <>
                      <p className="fine">These changes have files that cannot be fully read. Restore the storage point from a backup, then check again.</p>
                      <ul className="links">
                        {check.missing.map((m) => (
                          <li key={m.hash}>
                            <code title={m.hash}>{m.hash}</code>
                            <span className="fine">Used by {plural(m.changes.length, 'change')}: {m.changes.join(', ')}</span>
                          </li>
                        ))}
                      </ul>
                    </>
                  )}
                </div>
              )}
            </section>

            <section className="section">
              <h2>Reclaim space</h2>
              <p>Chunks that nothing refers to any more, for example from a deleted draft or an interrupted upload. First see how much, then decide.</p>
              <div className="row">
                <button className="btn sm" onClick={dryRun} disabled={busy === 'gc'}>{busy === 'gc' ? 'Working it out' : 'See how much can be reclaimed'}</button>
              </div>
              {gc && (
                <div className="result">
                  <b className="big">{size(gc.bytes)}</b>
                  <p>in {plural(gc.unreferenced_chunks, 'unused chunk')}.</p>
                  {gc.bytes > 0 ? (
                    isServerAdmin ? (
                      <button className="btn sm danger solid" style={{ alignSelf: 'flex-start' }} onClick={reclaim} disabled={busy === 'gcrun'}>{busy === 'gcrun' ? 'Reclaiming' : `Reclaim ${size(gc.bytes)}`}</button>
                    ) : (
                      <p className="fine">Only server admins can reclaim space. Ask one to run it.</p>
                    )
                  ) : (
                    <p className="fine">Nothing to reclaim.</p>
                  )}
                </div>
              )}
              {gcDone && (
                <div className="result good">
                  <b className="big">Reclaimed {size(gcDone.bytes)}</b>
                  <p>{plural(gcDone.deleted, 'chunk')} deleted.</p>
                </div>
              )}
            </section>
          </div>

          <section className="section">
            <h2>Checkpoint space</h2>
            <p>Checkpoints people keep on this server, by person and project. You see sizes only, never what is in them.</p>
            {kept.loading && !kept.data && <Loading what="Adding up checkpoints" />}
            {kept.error !== undefined && <ErrorNote error={kept.error} doing="load checkpoint space" retry={kept.reload} />}
            {cleared !== null && (
              <div className="result good">
                <b className="big">Removed {plural(cleared, 'checkpoint')}</b>
                <p>Space is freed the next time unused data is cleaned up.</p>
              </div>
            )}
            {kept.data && !keptList.length && (
              <Empty title="No checkpoints are kept on this server.">Projects keep checkpoints on the server when they are set to, or automatically from 20 GB.</Empty>
            )}
            {keptList.length > 0 && (
              <>
                <table className="t">
                  <thead>
                    <tr>
                      <th>Person</th>
                      <th>Project</th>
                      <th className="r">Checkpoints</th>
                      <th className="r">Space used</th>
                      <th>Oldest</th>
                      <th />
                    </tr>
                  </thead>
                  <tbody>
                    {keptList.map((c) => (
                      <tr key={keptKey(c)}>
                        <td>
                          <span className="row" style={{ gap: 8, flexWrap: 'nowrap' }}>
                            <Who id={c.user.id} name={c.user.name} />
                            <span>
                              <Link className="rowlink" to={`/admin/people/${c.user.id}`}>{c.user.name}</Link>
                              <br /><span className="fine">{c.user.email}</span>
                            </span>
                          </span>
                        </td>
                        <td>{c.project.name}<br /><span className="fine">{c.org.name}</span></td>
                        <td className="r">{c.checkpoints.toLocaleString()}<br /><span className="fine">{c.automatic.toLocaleString()} automatic</span></td>
                        <td className="r">{size(c.stored)}</td>
                        <td>{since(c.oldest)}</td>
                        <td className="r">
                          <span className="row" style={{ justifyContent: 'flex-end', flexWrap: 'nowrap' }}>
                            <button className="btn sm" disabled={!c.automatic || busy === 'cpauto:' + keptKey(c)} onClick={() => clearKept(c, true)}>Clear automatic</button>
                            <button className="btn sm danger" disabled={busy === 'cpall:' + keptKey(c)} onClick={() => clearKept(c, false)}>Clear all</button>
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <p className="fine">In total, {plural(keptCount, 'checkpoint')} using {size(keptBytes)}.</p>
              </>
            )}
          </section>
        </div>
      </div>
    </>
  );
}
