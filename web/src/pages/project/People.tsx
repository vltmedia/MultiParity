import { api } from '../../api/client';
import type { Claim } from '../../api/types';
import { useSession } from '../../lib/session';
import { baseName, clock, firstName, held, personColors, plural, when } from '../../lib/format';
import { Empty, Thumb, useAction, useUi, Who } from '../../components/ui';
import { useProject, ViewHead } from './context';

export default function People() {
  const { project, branches, claims, status, ws, nudges, bump } = useProject();
  const { me } = useSession();
  const { toast, confirm } = useUi();
  const { busy, run } = useAction();

  const byUser = new Map<string, { id: string; name: string; claims: Claim[] }>();
  for (const c of claims) {
    const g = byUser.get(c.user.id) || { id: c.user.id, name: c.user.name, claims: [] };
    g.claims.push(c);
    byUser.set(c.user.id, g);
  }
  const mine = byUser.get(me?.id || '');
  byUser.delete(me?.id || '');
  const others = [...byUser.values()].sort((a, b) => b.claims.length - a.claims.length);
  const unpublished = ws && status ? status.new.length + status.edited.length + status.deleted.length : 0;
  const branchName = (id: string) => branches.find((b) => b.id === id)?.name;
  const where = (c: Claim) => {
    const n = branchName(c.branch_id);
    return n && n !== 'Main' ? ` on ${n}` : '';
  };

  async function nudge(c: Claim) {
    const r = await run('nudge:' + c.path, 'ask for the file', () => api.nudge(project.id, c.branch_id, c.path));
    if (r) toast(`Asked ${firstName(c.user.name)} to release ${baseName(c.path)}.`);
  }
  async function nudgeAll(list: Claim[]) {
    for (const c of list) await nudge(c);
  }
  async function release(list: Claim[]) {
    if (list.length > 1) {
      const ok = await confirm({
        title: `Release ${plural(list.length, 'file')}?`,
        what: <>Your claims on {plural(list.length, 'file')} go back, so anyone can claim them. Your edits stay on your computer; you may not be able to publish them if someone else claims a file first.</>,
        confirm: `Release ${plural(list.length, 'file')}`,
      });
      if (!ok) return;
    }
    const groups = new Map<string, string[]>();
    for (const c of list) groups.set(c.branch_id, [...(groups.get(c.branch_id) || []), c.path]);
    let n = 0;
    for (const [b, paths] of groups) {
      const r = await run('release', 'release the files', () => api.release(project.id, b, paths));
      if (r) n += r.released.length;
    }
    if (n) {
      toast(`Released ${plural(n, 'file')}.`);
      bump('claims');
    }
  }

  const holders = others.length + (mine ? 1 : 0);
  return (
    <>
      <ViewHead
        title="Who has what"
        sub={holders ? `${plural(holders, 'person', 'people')} ${holders === 1 ? 'has' : 'have'} files claimed. Everything else is free to pick up.` : 'Nobody has anything claimed. Every file is free to pick up.'}
      />
      <div className="view">
        {!holders && !unpublished && <Empty title="Nothing claimed">When someone starts editing a model or a texture, it shows up here with their name.</Empty>}
        {(holders > 0 || unpublished > 0) && (
          <div className="wall">
            {(mine || unpublished > 0) && (
              <section className="p" style={{ ['--c' as string]: personColors(me?.id || '').bg }}>
                <div className="p-head">
                  {me && <Who id={me.id} name={me.name} size={34} />}
                  <div>
                    <h2>You</h2>
                    <p>
                      {unpublished ? `${plural(unpublished, 'file')} not published` : 'Nothing unpublished'}
                      {mine ? `, ${plural(mine.claims.length, 'claim')}` : ''}
                    </p>
                  </div>
                </div>
                <ul>
                  {mine?.claims.map((c) => (
                    <li key={c.path}>
                      <Thumb seed={c.path} size={42} />
                      <div>
                        <strong className="el" title={c.path}>{baseName(c.path)}</strong>
                        <small>Claimed since {held(c.created_at) === '1 min' ? 'just now' : clock(c.created_at)}{where(c)}</small>
                      </div>
                      <button className="link" disabled={!!busy} onClick={() => release([c])}>Release</button>
                    </li>
                  ))}
                </ul>
                {nudges.map((n) => (
                  <div key={n.path} className="wait">{firstName(n.by)} is waiting for {baseName(n.path)}</div>
                ))}
                {mine && mine.claims.length > 1 && <button className="btn sm" disabled={!!busy} onClick={() => release(mine.claims)}>Release all {mine.claims.length}</button>}
              </section>
            )}
            {others.map((o) => {
              const oldest = o.claims.reduce((a, c) => (c.created_at < a ? c.created_at : a), o.claims[0].created_at);
              return (
                <section key={o.id} className="p" style={{ ['--c' as string]: personColors(o.id).bg }}>
                  <div className="p-head">
                    <Who id={o.id} name={o.name} size={34} />
                    <div>
                      <h2>{firstName(o.name)}</h2>
                      <p>Working for {held(oldest)}</p>
                    </div>
                  </div>
                  <ul>
                    {o.claims.map((c) => (
                      <li key={c.path}>
                        <Thumb seed={c.path} size={42} />
                        <div>
                          <strong className="el" title={c.path}>{baseName(c.path)}</strong>
                          <small>Since {when(c.created_at)}{c.device ? `, on ${c.device}` : ''}{where(c)}</small>
                        </div>
                        <button className="link" disabled={busy === 'nudge:' + c.path} onClick={() => nudge(c)}>Ask</button>
                      </li>
                    ))}
                  </ul>
                  <button className="btn sm" disabled={!!busy} onClick={() => nudgeAll(o.claims)}>
                    {o.claims.length > 1 ? `Ask to release all ${o.claims.length}` : 'Ask to release'}
                  </button>
                </section>
              );
            })}
          </div>
        )}
      </div>
    </>
  );
}
