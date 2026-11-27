import '../../styles/projects.css';
import { Fragment, useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { api } from '../../api/client';
import { daemon } from '../../api/daemon';
import type { LocalStatus, ProjectSummary, Workspace } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { useDaemon } from '../../lib/daemon';
import { useSession } from '../../lib/session';
import { firstName, plural, ROLE_LABEL, since, size, sizeOf } from '../../lib/format';
import { Empty, ErrorNote, Loading, Thumb, Toggle, useUi } from '../../components/ui';
import TopBar from '../../components/TopBar';
import SyncPanel from './SyncPanel';

type Filter = 'all' | 'here' | 'away' | 'mine';

export default function ProjectsPage() {
  const { me } = useSession();
  const d = useDaemon();
  const { confirm, fail, toast } = useUi();
  const list = useAsync(() => api.projects(), []);
  const [params, setParams] = useSearchParams();
  const filter = (params.get('show') as Filter) || 'all';
  const q = params.get('q') || '';
  const [openSync, setOpenSync] = useState<{ id: string; all?: boolean } | null>(null);
  const [statuses, setStatuses] = useState<Record<string, LocalStatus>>({});

  useEffect(() => {
    if (!d.available) return;
    let live = true;
    Promise.all(d.workspaces.map((w) => daemon.status(w.id).then((s) => [w.id, s] as const).catch(() => null))).then((r) => {
      if (live) setStatuses(Object.fromEntries(r.filter(Boolean) as Array<readonly [string, LocalStatus]>));
    });
    return () => {
      live = false;
    };
  }, [d.available, d.workspaces]);

  const set = (k: string, v: string) => {
    const n = new URLSearchParams(params);
    if (v && v !== 'all') n.set(k, v);
    else n.delete(k);
    setParams(n, { replace: true });
  };

  const unpublished = (w?: Workspace) => {
    const s = w && statuses[w.id];
    return s ? s.new.length + s.edited.length + s.deleted.length : 0;
  };

  const visible = useMemo(() => {
    const t = q.trim().toLowerCase();
    return (list.data || [])
      .filter((p) => !p.archived)
      .filter((p) => !t || [p.name, p.org.name, p.client?.name, p.production?.name].some((x) => x?.toLowerCase().includes(t)) || p.slug.includes(t))
      .filter((p) => {
        const w = d.byProject(p.id);
        if (filter === 'here') return !!w && w.local_bytes > 0;
        if (filter === 'away') return !w || w.local_bytes === 0;
        if (filter === 'mine') return p.my_claims > 0 || unpublished(w) > 0;
        return true;
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [list.data, q, filter, d.workspaces, statuses]);

  const groups = useMemo(() => {
    const m = new Map<string, { org: ProjectSummary['org']; items: ProjectSummary[] }>();
    for (const p of visible) {
      if (!m.has(p.org.id)) m.set(p.org.id, { org: p.org, items: [] });
      m.get(p.org.id)!.items.push(p);
    }
    return [...m.values()].sort((a, b) => a.org.name.localeCompare(b.org.name)).map((g) => ({ ...g, bands: bandsOf(g.items) }));
  }, [visible]);

  const local = d.workspaces.reduce((a, w) => a + w.local_bytes, 0);
  const all = (list.data || []).filter((p) => !p.archived);
  const orgCount = new Set(all.map((p) => p.org.id)).size;

  async function setSwitch(p: ProjectSummary, w: Workspace, on: boolean) {
    if (on) {
      setOpenSync({ id: p.id, all: true });
      return;
    }
    const ok = await confirm({
      title: `Take ${p.name} off this computer?`,
      what: (
        <>
          Files that are the same as on the server are removed from {w.root}, about {size(w.local_bytes)}.
          {unpublished(w) > 0 ? ` Your ${plural(unpublished(w), 'unpublished file')} stay where they are.` : ' Edited and new files would stay where they are.'}
          {' '}The folder stays connected, so you can turn it back on without setting it up again.
        </>
      ),
      confirm: 'Take it off this computer',
    });
    if (!ok) return;
    try {
      await daemon.setSync(w.id, [], []);
      toast(`${p.name} is no longer on this computer.`);
      d.refresh();
    } catch (e) {
      fail(e, `take ${p.name} off this computer`);
    }
  }

  const filters: Array<[Filter, string]> = d.available
    ? [['all', 'All'], ['here', 'On this computer'], ['away', 'Not on this computer'], ['mine', 'Has my work']]
    : [['all', 'All'], ['mine', 'Has my work']];

  return (
    <>
      <TopBar />
      <div className="page">
        <header className="head">
          <div>
            <h1>Projects</h1>
            <p>
              {list.data
                ? `${plural(all.length, 'project')} across ${plural(orgCount, 'organization')}.`
                : 'Everything you can see on this server.'}
              {d.available && d.workspaces.length > 0 && ` ${size(local)} of it is on this computer.`}
            </p>
          </div>
          <div className="tools">
            <label className="search">
              <span className="sr">Find a project</span>
              <input className="input" type="search" placeholder="Find a project or organization" value={q} onChange={(e) => set('q', e.target.value)} />
            </label>
          </div>
        </header>
        <div className="body">
          <div className="row filters">
            <div className="seg" role="group" aria-label="Show">
              {filters.map(([k, label]) => (
                <button key={k} aria-pressed={filter === k} onClick={() => set('show', k)}>{label}</button>
              ))}
            </div>
            {d.available === false && (
              <p className="quiet grow">
                <b>Want these files on this computer?</b> Install the MultiParity desktop app to sync projects, pick folders and publish your work. You can browse everything here without it.
              </p>
            )}
            {d.available && <p className="fine grow" style={{ textAlign: 'right' }}>The MultiParity app on this computer is running{d.version ? `, version ${d.version}` : ''}.</p>}
          </div>

          {list.loading && !list.data && <Loading what="Loading your projects" />}
          {list.error !== undefined && <ErrorNote error={list.error} doing="load your projects" retry={list.reload} />}
          {list.data && !visible.length && (
            <Empty title={all.length ? 'No projects match' : 'No projects yet'}>
              {all.length
                ? 'Try another filter or a shorter search.'
                : me && (me.is_server_admin || me.orgs.some((o) => o.role === 'owner' || o.role === 'admin'))
                  ? <>Make the first one in <Link className="link" to="/admin/projects">Admin, Projects</Link>.</>
                  : 'When someone adds you to a project, it shows up here.'}
            </Empty>
          )}

          {groups.map((g) => {
            const role = me?.orgs.find((o) => o.org.id === g.org.id)?.role;
            return (
              <section key={g.org.id} className="pgroup">
                <h2>
                  {g.org.name}
                  {role && <span>{ROLE_LABEL[role]}</span>}
                </h2>
                <div className="plist">
                  <div className={`prow phead ${d.available ? 'local' : ''}`} aria-hidden="true">
                    <span />
                    <span>Project</span>
                    <span className="r">Change</span>
                    <span className="r">Size</span>
                    <span className="r">Claims</span>
                    <span>Last change</span>
                    {d.available && <span>On this computer</span>}
                  </div>
                  {g.bands.map((band) => (
                  <Fragment key={band.key}>
                  {band.key && (
                    <div className="psub">
                      {band.client && <span className="psub-client" data-ic="briefcase">{band.client}</span>}
                      {band.client && band.production && <span className="psub-sep" aria-hidden="true">›</span>}
                      {band.production && (band.main
                        ? <Link className="psub-prod" data-ic="clapperboard" to={`/p/${band.main}`} title={`Open ${band.production}'s main project`}>{band.production}</Link>
                        : <span className="psub-prod" data-ic="clapperboard">{band.production}</span>)}
                      {!band.client && !band.production && <span className="psub-other">Other projects</span>}
                    </div>
                  )}
                  {band.items.map((p) => {
                    const w = d.byProject(p.id);
                    const behind = w ? Math.max(0, p.head_change - w.synced_change) : 0;
                    const un = unpublished(w);
                    const on = !!w && (w.include.length > 0 || w.local_bytes > 0);
                    return (
                      <div key={p.id} className="pitem">
                        <div className={`prow ${d.available ? 'local' : ''}`}>
                          <Thumb seed={p.id} size={48} />
                          <div className="pname">
                            <Link to={`/p/${p.id}`} className="pl">{p.name}</Link>
                            {p.is_main && <span className="badge good pmain" title={`The main project of ${p.production?.name}`}>Main</span>}
                            <small>
                              {p.org.name}
                              {p.branches > 1 && `, ${plural(p.branches, 'branch', 'branches')}`}
                              {w && (
                                <>
                                  {' · '}
                                  {behind > 0 ? <b className="behind">{behind} newer</b> : <span>Up to date</span>}
                                  {un > 0 && <>, <b className="unpub">{un} unpublished</b></>}
                                </>
                              )}
                            </small>
                          </div>
                          <span className="r num">{p.head_change.toLocaleString()}</span>
                          <span className="r">{size(p.size)}</span>
                          <span className="r">
                            {p.claims ? (
                              <>
                                {p.my_claims > 0 && <b className="mineclaims">{p.my_claims} yours</b>}
                                {p.my_claims > 0 ? ', ' : ''}
                                {p.claims} in all
                              </>
                            ) : (
                              <span className="mute">None</span>
                            )}
                          </span>
                          <span className="plast">
                            {p.last_change ? (
                              <>
                                <Link to={`/p/${p.id}/changes/${p.last_change.number}`} className="el">{p.last_change.message}</Link>
                                <small>{firstName(p.last_change.author.name)}, {since(p.last_change.created_at)}</small>
                              </>
                            ) : (
                              <small>Nothing published yet</small>
                            )}
                          </span>
                          {d.available && (
                            <span className="plocal">
                              {w ? (
                                <>
                                  <span className="pl-size">
                                    <b>{on ? sizeOf(w.local_bytes, w.total_bytes || p.size) : 'Nothing here'}</b>
                                    <span className="bar" aria-hidden="true"><i style={{ width: `${Math.min(100, (w.local_bytes / Math.max(1, w.total_bytes || p.size)) * 100)}%` }} /></span>
                                  </span>
                                  <Toggle on={on} label={`Keep ${p.name} on this computer`} onChange={(v) => setSwitch(p, w, v)} />
                                  <button className="link fine" onClick={() => setOpenSync(openSync?.id === p.id ? null : { id: p.id })} aria-expanded={openSync?.id === p.id}>
                                    Choose folders
                                  </button>
                                </>
                              ) : (
                                <span className="fine">Not connected. Connect it from the desktop app.</span>
                              )}
                            </span>
                          )}
                        </div>
                        {w && openSync?.id === p.id && (
                          <SyncPanel
                            ws={w}
                            startAll={openSync.all}
                            onClose={() => setOpenSync(null)}
                            onSaved={() => {
                              setOpenSync(null);
                              d.refresh();
                            }}
                          />
                        )}
                      </div>
                    );
                  })}
                  </Fragment>
                  ))}
                </div>
              </section>
            );
          })}
        </div>
      </div>
    </>
  );
}

/** An organization's projects in bands by client and production (main project first), the
 * ones not filed under either last. Without any filing there is one band and no headings. */
function bandsOf(items: ProjectSummary[]) {
  const m = new Map<string, { key: string; client?: string; production?: string; main?: string | null; items: ProjectSummary[] }>();
  for (const p of items) {
    const key = p.production ? `pr:${p.production.id}` : p.client ? `cl:${p.client.id}` : '';
    if (!m.has(key)) m.set(key, { key, client: p.client?.name, production: p.production?.name, main: p.production?.main_project_id, items: [] });
    m.get(key)!.items.push(p);
  }
  const bands = [...m.values()];
  for (const b of bands) b.items.sort((x, y) => Number(!!y.is_main) - Number(!!x.is_main) || x.name.localeCompare(y.name));
  bands.sort((a, b) => Number(!a.key) - Number(!b.key) || (a.client || '\uffff').localeCompare(b.client || '\uffff') || (a.production || '').localeCompare(b.production || ''));
  // The unfiled band needs its "Other projects" heading only when there are other bands.
  if (bands.length === 1 && !bands[0].key) return bands;
  return bands.map((b) => (b.key ? b : { ...b, key: 'other' }));
}
