// Admin > Policies: named sets of access rules given to people, e.g. "Audio team" or "Northwind
// reviewers". A rule gives no access, read or write on the whole organization, a client, a
// production or one project. For each project the most specific rules win (project, then
// production, then client, then organization), and among those the highest level. Owners and
// admins always have everything; members also have the organization's "All members" policy.
import { useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { api } from '../../api/client';
import type { Member, OrgStructure, Policy, PolicyRule, ProjectSummary, RuleLevel, ScopeKind } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { plural, ROLE_LABEL } from '../../lib/format';
import { Dialog, Empty, ErrorNote, Loading, useAction, useUi } from '../../components/ui';
import { AdminHead, useAdmin } from './AdminLayout';

export const LEVEL_TEXT: Record<RuleLevel, string> = { none: 'No access', read: 'Can look', write: 'Can change' };
const LEVEL_HELP: Record<RuleLevel, string> = {
  none: 'Hidden from them, even if a broader rule gives access',
  read: 'See, download and get latest; no publishing or claiming',
  write: 'Publish, claim and save drafts too',
};

type Scope = { kind: ScopeKind; id: string; label: string };

/** Everything a rule can cover in one organization, broadest first. */
export function scopesOf(orgId: string, orgName: string, s: OrgStructure | undefined, projects: ProjectSummary[]): Scope[] {
  const clientName = (id: string | null) => s?.clients.find((c) => c.id === id)?.name;
  return [
    { kind: 'org', id: orgId, label: `Everything in ${orgName}` },
    ...(s?.clients || []).map((c) => ({ kind: 'client' as const, id: c.id, label: c.name })),
    ...(s?.productions || []).map((pr) => ({ kind: 'production' as const, id: pr.id, label: [clientName(pr.client_id), pr.name].filter(Boolean).join(' › ') })),
    ...projects.filter((p) => p.org.id === orgId && !p.archived).map((p) => ({ kind: 'project' as const, id: p.id, label: [p.client?.name, p.production?.name, p.name].filter(Boolean).join(' › ') })),
  ];
}

const KIND_LABEL: Record<ScopeKind, string> = { org: 'Organization', client: 'Client', production: 'Production', project: 'Project' };

export default function AdminPolicies() {
  const { scope, orgs } = useAdmin();
  const [params, setParams] = useSearchParams();
  const org = params.get('org') || scope || orgs[0]?.id || '';
  const orgName = orgs.find((o) => o.id === org)?.name || '';
  const list = useAsync(() => api.admin.policies(org), [org], !!org);
  const structure = useAsync(() => api.admin.structure(org), [org], !!org);
  const projects = useAsync(() => api.admin.projects(org), [org], !!org);
  const members = useAsync(() => api.admin.members(org), [org], !!org);
  const scopes = useMemo(() => scopesOf(org, orgName, structure.data, projects.data || []), [org, orgName, structure.data, projects.data]);
  const [edit, setEdit] = useState<Policy | 'new' | null>(null);
  const [people, setPeople] = useState<Policy | null>(null);
  const { confirm, toast } = useUi();
  const { run } = useAction();

  async function remove(p: Policy) {
    const ok = await confirm({
      title: `Delete the policy ${p.name}?`,
      what: <>{p.people.length ? `${plural(p.people.length, 'person', 'people')} lose what it gives them, straight away.` : 'Nobody has it.'} Their own work, claims and files are not touched.</>,
      confirm: 'Delete policy',
      danger: true,
    });
    if (!ok) return;
    if (await run('rm:' + p.id, 'delete the policy', () => api.admin.deletePolicy(p.id))) {
      toast(`Deleted ${p.name}.`);
      list.reload();
    }
  }

  return (
    <>
      <AdminHead title="Policies" sub={`Who can see and change what in ${orgName || 'an organization'}.`}>
        {org && <button className="btn pri sm" onClick={() => setEdit('new')}>New policy</button>}
      </AdminHead>
      <div className="view">
        {!scope && orgs.length > 1 && (
          <div className="toolbar">
            <label className="field inline">
              <span>Organization</span>
              <select className="select" value={org} onChange={(e) => { const n = new URLSearchParams(params); n.set('org', e.target.value); setParams(n, { replace: true }); }}>
                {orgs.map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
              </select>
            </label>
          </div>
        )}
        <div className="notice policy-how">
          <b>How access is worked out.</b> Owners and admins can do everything. Everyone else gets what their policies give; members also get <i>All members</i>.
          For each project the most specific rule wins (a project rule over a production rule, over a client rule, over the whole organization), so you can give a team everything and still keep one project from them.
        </div>
        {list.loading && !list.data && <Loading what="Loading policies" />}
        {list.error !== undefined && <ErrorNote error={list.error} doing="load policies" retry={list.reload} />}
        {list.data && !list.data.length && <Empty title="No policies">Make one with New policy.</Empty>}
        <ul className="policies">
          {(list.data || []).map((p) => (
            <li key={p.id}>
              <div className="pol-head">
                <h3 data-ic="shield-check">{p.name}</h3>
                {p.is_default && <span className="badge">Every member</span>}
                <span className="grow" />
                {!p.is_default && <button className="btn sm" onClick={() => setPeople(p)}>People…</button>}
                <button className="btn sm" onClick={() => setEdit(p)}>Edit…</button>
                {!p.is_default && <button className="btn sm danger" onClick={() => remove(p)}>Delete</button>}
              </div>
              {p.description && <p className="fine">{p.description}</p>}
              {p.rules.length ? (
                <ul className="rules">
                  {p.rules.map((r) => (
                    <li key={r.scope_kind + r.scope_id} className={`lvl-${r.level}`}>
                      <b>{LEVEL_TEXT[r.level]}</b>
                      <span>{r.scope_kind === 'org' ? `everything in ${r.scope_name}` : r.scope_name}</span>
                      <small>{KIND_LABEL[r.scope_kind]}</small>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="fine">No rules: it gives nothing.</p>
              )}
              <p className="fine pol-people">
                {p.is_default
                  ? `${plural(p.member_count || 0, 'member')} ${p.member_count === 1 ? 'has' : 'have'} it. Guests do not.`
                  : p.people.length
                    ? <>Given to {p.people.map((u) => u.name).join(', ')}</>
                    : 'Not given to anyone yet.'}
              </p>
            </li>
          ))}
        </ul>
      </div>
      {edit && (
        <PolicyDialog
          org={org}
          policy={edit === 'new' ? null : edit}
          scopes={scopes}
          onClose={() => setEdit(null)}
          onSaved={() => { setEdit(null); list.reload(); }}
        />
      )}
      {people && members.data && (
        <PeopleDialog policy={people} members={members.data} onClose={() => setPeople(null)} onSaved={() => { setPeople(null); list.reload(); }} />
      )}
    </>
  );
}

function PolicyDialog({ org, policy, scopes, onClose, onSaved }: { org: string; policy: Policy | null; scopes: Scope[]; onClose: () => void; onSaved: () => void }) {
  const { run, busy } = useAction();
  const { toast } = useUi();
  const [name, setName] = useState(policy?.name || '');
  const [description, setDescription] = useState(policy?.description || '');
  const [rules, setRules] = useState<PolicyRule[]>(policy?.rules.map(({ scope_kind, scope_id, level }) => ({ scope_kind, scope_id, level })) || []);
  const [err, setErr] = useState('');
  const key = (r: { scope_kind: ScopeKind; scope_id: string }) => `${r.scope_kind}:${r.scope_id}`;
  const unused = scopes.filter((s) => !rules.some((r) => r.scope_kind === s.kind && r.scope_id === s.id));

  function add() {
    const s = unused[0];
    if (s) setRules((x) => [...x, { scope_kind: s.kind, scope_id: s.id, level: 'write' }]);
  }
  function change(i: number, patch: Partial<PolicyRule>) {
    setRules((x) => x.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  }

  async function save() {
    setErr('');
    if (!name.trim()) return setErr('Give the policy a name.');
    const body = { name: name.trim(), description: description.trim(), rules };
    const r = policy
      ? await run('save', 'save the policy', () => api.admin.updatePolicy(policy.id, body))
      : await run('save', 'make the policy', () => api.admin.createPolicy(org, body));
    if (!r) return;
    toast(policy ? `Saved ${name.trim()}. It applies straight away.` : `Made ${name.trim()}. Next, give it to people with the People… button.`);
    onSaved();
  }

  const group = (k: ScopeKind) => scopes.filter((s) => s.kind === k);
  return (
    <Dialog title={policy ? `Edit ${policy.name}` : 'New policy'} onClose={onClose} wide>
      <div className="grid2" style={{ gap: 14 }}>
        <label className="field">
          <span>Name</span>
          <input className="input" autoFocus={!policy} value={name} onChange={(e) => setName(e.target.value)} placeholder="Audio team" disabled={policy?.is_default} />
        </label>
        <label className="field">
          <span>What it is for <small>(optional)</small></span>
          <input className="input" value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Sound designers, in-house and contract" />
        </label>
      </div>
      <div className="field">
        <span>Rules</span>
        {!rules.length && <p className="fine">No rules yet: this policy gives nothing.</p>}
        <ul className="rule-edit">
          {rules.map((r, i) => (
            <li key={i}>
              <select className="select" aria-label="Covers" value={key(r)} onChange={(e) => { const [k, id] = e.target.value.split(':'); change(i, { scope_kind: k as ScopeKind, scope_id: id }); }}>
                {(['org', 'client', 'production', 'project'] as ScopeKind[]).map((k) => group(k).length > 0 && (
                  <optgroup key={k} label={KIND_LABEL[k] + (k === 'org' ? '' : 's')}>
                    {group(k).map((s) => (
                      <option key={key({ scope_kind: s.kind, scope_id: s.id })} value={key({ scope_kind: s.kind, scope_id: s.id })} disabled={rules.some((o, j) => j !== i && o.scope_kind === s.kind && o.scope_id === s.id)}>
                        {s.label}
                      </option>
                    ))}
                  </optgroup>
                ))}
                {!scopes.some((s) => s.kind === r.scope_kind && s.id === r.scope_id) && <option value={key(r)}>Something since removed</option>}
              </select>
              <select className="select lvl" aria-label="Access" value={r.level} onChange={(e) => change(i, { level: e.target.value as RuleLevel })} title={LEVEL_HELP[r.level]}>
                {(['write', 'read', 'none'] as RuleLevel[]).map((l) => <option key={l} value={l}>{LEVEL_TEXT[l]}</option>)}
              </select>
              <button className="btn sm" aria-label="Remove this rule" onClick={() => setRules((x) => x.filter((_, j) => j !== i))}>Remove</button>
            </li>
          ))}
        </ul>
        <div><button className="btn sm" onClick={add} disabled={!unused.length}>Add rule</button></div>
        <small>{LEVEL_TEXT.write}: {LEVEL_HELP.write.toLowerCase()}. {LEVEL_TEXT.read}: {LEVEL_HELP.read.toLowerCase()}. {LEVEL_TEXT.none}: {LEVEL_HELP.none.toLowerCase()}.</small>
      </div>
      {err && <div className="notice bad">{err}</div>}
      <div className="actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn pri" onClick={save} disabled={busy === 'save'}>{policy ? 'Save' : 'Make policy'}</button>
      </div>
    </Dialog>
  );
}

function PeopleDialog({ policy, members, onClose, onSaved }: { policy: Policy; members: Member[]; onClose: () => void; onSaved: () => void }) {
  const { run, busy } = useAction();
  const { toast } = useUi();
  const [chosen, setChosen] = useState<Set<string>>(new Set(policy.people.map((p) => p.id)));
  const [q, setQ] = useState('');
  const shown = members.filter((m) => !q.trim() || `${m.user.name} ${m.user.email}`.toLowerCase().includes(q.trim().toLowerCase()));
  const toggle = (id: string) => setChosen((s) => { const n = new Set(s); if (n.has(id)) n.delete(id); else n.add(id); return n; });

  async function save() {
    if (!(await run('save', 'save who has the policy', () => api.admin.setPolicyPeople(policy.id, [...chosen])))) return;
    toast(`${policy.name} is now given to ${plural(chosen.size, 'person', 'people')}. It applies straight away.`);
    onSaved();
  }

  return (
    <Dialog title={`Who has ${policy.name}`} onClose={onClose}>
      <input className="input" type="search" placeholder="Find someone" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find someone" />
      <ul className="people-pick">
        {shown.map((m) => {
          const boss = m.role === 'owner' || m.role === 'admin';
          return (
            <li key={m.user.id}>
              <label className="check-label">
                <input type="checkbox" checked={chosen.has(m.user.id)} onChange={() => toggle(m.user.id)} />
                <span>{m.user.name} <small className="fine">{m.user.email}</small></span>
                <span className="fine">{ROLE_LABEL[m.role]}{boss ? ': already has everything' : ''}</span>
              </label>
            </li>
          );
        })}
      </ul>
      <div className="actions">
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn pri" onClick={save} disabled={busy === 'save'}>Save</button>
      </div>
    </Dialog>
  );
}

const WHO_LEVEL: Record<string, string> = { admin: 'Everything', write: 'Can change', read: 'Can look', none: 'No access' };

/** Who in the organization can see a project, at what level, and why. */
export function WhoDialog({ p, onClose }: { p: ProjectSummary; onClose: () => void }) {
  const who = useAsync(() => api.admin.whoCanSee(p.id), [p.id]);
  const [hideNone, setHideNone] = useState(true);
  const rows = (who.data || []).filter((w) => !hideNone || w.level !== 'none');
  const hidden = (who.data || []).filter((w) => w.level === 'none').length;
  return (
    <Dialog title={`Who can see ${p.name}`} onClose={onClose} wide>
      <p className="fine">Worked out from each person's role and policies, as they are now. Change it on the Policies page.</p>
      {who.loading && <Loading what="Working it out" />}
      {who.error !== undefined && <ErrorNote error={who.error} doing="work out who can see it" retry={who.reload} />}
      {who.data && (
        <>
          <table className="t who-t">
            <thead><tr><th>Person</th><th>Role</th><th>Access</th><th>Because</th></tr></thead>
            <tbody>
              {rows.map((w) => (
                <tr key={w.user.id}>
                  <td>{w.user.name}<br /><span className="fine">{w.user.email}</span></td>
                  <td>{ROLE_LABEL[w.role]}</td>
                  <td><span className={`badge ${w.level === 'none' ? 'mute' : w.level === 'read' ? 'hold' : 'good'}`}>{WHO_LEVEL[w.level]}</span></td>
                  <td className="fine">{w.because}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {hidden > 0 && (
            <label className="check-label fine" style={{ marginTop: 10 }}>
              <input type="checkbox" checked={!hideNone} onChange={(e) => setHideNone(!e.target.checked)} /> Also show the {plural(hidden, 'person', 'people')} who cannot see it
            </label>
          )}
        </>
      )}
      <div className="actions"><button className="btn pri" onClick={onClose}>Done</button></div>
    </Dialog>
  );
}
