import { useEffect, useState } from 'react';
import { api } from '../../api/client';
import type { MpEvent } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { dateTime, eventLabel } from '../../lib/format';
import { Empty, ErrorNote, Loading, Who, useAction } from '../../components/ui';
import { AdminHead, useAdmin } from './AdminLayout';

const PAGE = 50;
const KINDS: Array<[string, string]> = [
  ['', 'Everything'],
  ['change.', 'Changes and merges'],
  ['claim.', 'Claims'],
  ['tag.', 'Tags'],
  ['branch.', 'Branches'],
  ['member.', 'Members'],
  ['project.', 'Projects'],
  ['admin.', 'Admin actions'],
];

function details(e: MpEvent): string {
  return Object.entries(e.data || {})
    .filter(([, v]) => v !== undefined && v !== null && v !== '')
    .map(([k, v]) => `${k.replace(/_/g, ' ')}: ${Array.isArray(v) ? v.join(', ') : typeof v === 'object' ? JSON.stringify(v) : String(v)}`)
    .join('; ');
}

export default function AdminAudit() {
  const { scope, scopeName } = useAdmin();
  const [kind, setKind] = useState('');
  const [project, setProject] = useState('');
  const [actor, setActor] = useState('');
  const projects = useAsync(() => api.admin.projects(scope || undefined), [scope]);
  const people = useAsync(() => api.admin.users({ org: scope || undefined }), [scope]);
  const first = useAsync(
    () => api.admin.audit({ org: scope || undefined, project: project || undefined, actor: actor || undefined, kind: kind || undefined, limit: PAGE }),
    [scope, project, actor, kind],
  );
  const [older, setOlder] = useState<MpEvent[]>([]);
  const [done, setDone] = useState(false);
  const { run, busy } = useAction();
  useEffect(() => {
    setOlder([]);
    setDone(false);
  }, [scope, project, actor, kind]);
  const list = [...(first.data || []), ...older];
  const pname = (id?: string) => projects.data?.find((p) => p.id === id)?.name || '';

  async function more() {
    const last = list[list.length - 1];
    const r = await run('more', 'load older entries', () =>
      api.admin.audit({ org: scope || undefined, project: project || undefined, actor: actor || undefined, kind: kind || undefined, before: last.id, limit: PAGE }),
    );
    if (r) {
      setOlder((o) => [...o, ...r]);
      if (r.length < PAGE) setDone(true);
    }
  }

  function exportCsv() {
    const esc = (s: string) => `"${s.replace(/"/g, '""')}"`;
    const rows = [['When', 'Who', 'What', 'Kind', 'Project', 'Details'], ...list.map((e) => [e.created_at, e.actor?.name || '', eventLabel(e.kind), e.kind, pname(e.project_id), details(e)])];
    const blob = new Blob([rows.map((r) => r.map(esc).join(',')).join('\n')], { type: 'text/csv' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `multiparity-audit-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 5000);
  }

  return (
    <>
      <AdminHead title="Audit log" sub={`Who did what and when in ${scopeName}. Entries cannot be edited or removed.`}>
        <button className="btn sm" onClick={exportCsv} disabled={!list.length}>Download as CSV</button>
      </AdminHead>
      <div className="view">
        <div className="toolbar">
          <label>
            <span className="sr">What</span>
            <select className="select sm" value={kind} onChange={(e) => setKind(e.target.value)}>
              {KINDS.map(([k, l]) => <option key={k} value={k}>{l}</option>)}
            </select>
          </label>
          <label>
            <span className="sr">Project</span>
            <select className="select sm" value={project} onChange={(e) => setProject(e.target.value)}>
              <option value="">Every project</option>
              {projects.data?.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </label>
          <label>
            <span className="sr">Who</span>
            <select className="select sm" value={actor} onChange={(e) => setActor(e.target.value)}>
              <option value="">Anyone</option>
              {people.data?.map((u) => <option key={u.id} value={u.id}>{u.name}</option>)}
            </select>
          </label>
        </div>
        {first.loading && !first.data && <Loading what="Loading the audit log" />}
        {first.error !== undefined && <ErrorNote error={first.error} doing="load the audit log" retry={first.reload} />}
        {first.data && !list.length && <Empty title="Nothing logged">No entries match these filters.</Empty>}
        {list.length > 0 && (
          <table className="t">
            <thead><tr><th>When</th><th>Who</th><th>What</th><th>Project</th><th>Details</th></tr></thead>
            <tbody>
              {list.map((e) => (
                <tr key={e.id}>
                  <td>{dateTime(e.created_at)}</td>
                  <td>
                    {e.actor ? (
                      <span className="row" style={{ gap: 8, flexWrap: 'nowrap' }}><Who id={e.actor.id} name={e.actor.name} />{e.actor.name}</span>
                    ) : (
                      <span className="mute">The server</span>
                    )}
                  </td>
                  <td title={e.kind}>{eventLabel(e.kind)}</td>
                  <td>{pname(e.project_id)}</td>
                  <td className="data" title={details(e)}>{details(e)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {list.length >= PAGE && !done && (
          <button className="btn sm" style={{ marginTop: 12 }} onClick={more} disabled={busy === 'more'}>{busy === 'more' ? 'Loading' : 'Show older entries'}</button>
        )}
      </div>
    </>
  );
}
