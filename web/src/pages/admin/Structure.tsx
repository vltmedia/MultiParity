// Clients and productions in Admin > Projects: how an organization files its projects
// (Organization > Client > Production > Project). Both levels are optional; removing one never
// removes projects, it only moves what was in it up a level.
import { useCallback, useEffect, useState } from 'react';
import { api } from '../../api/client';
import type { Client, Org, OrgStructure, Production, ProjectSummary } from '../../api/types';
import { Dialog, useAction, useUi } from '../../components/ui';

/** Where a project is filed, as one value: "pr:<id>", "cl:<id>" or "" (not filed). */
export const placeOf = (p: ProjectSummary) => (p.production ? `pr:${p.production.id}` : p.client ? `cl:${p.client.id}` : '');

export function placeBody(v: string): { client: string | null; production: string | null } {
  if (v.startsWith('pr:')) return { client: null, production: v.slice(3) };
  if (v.startsWith('cl:')) return { client: v.slice(3), production: null };
  return { client: null, production: null };
}

/** The structure of each organization in view, reloaded together. */
export function useStructures(orgIds: string[]) {
  const key = orgIds.join(',');
  const [data, setData] = useState<Record<string, OrgStructure>>({});
  const [n, setN] = useState(0);
  useEffect(() => {
    let live = true;
    Promise.all(key.split(',').filter(Boolean).map((id) => api.admin.structure(id).then((s) => [id, s] as const).catch((e) => { console.warn('Could not load clients and productions', e); return [id, { clients: [], productions: [] }] as const; })))
      .then((pairs) => live && setData(Object.fromEntries(pairs)));
    return () => { live = false; };
  }, [key, n]);
  const reload = useCallback(() => setN((x) => x + 1), []);
  return { data, reload };
}

/** A select of everywhere a project can be filed in one organization. */
export function PlaceSelect({ s, value, onChange, id }: { s?: OrgStructure; value: string; onChange: (v: string) => void; id?: string }) {
  const clients = s?.clients || [];
  const prods = s?.productions || [];
  return (
    <select id={id} className="select" value={value} onChange={(e) => onChange(e.target.value)}>
      <option value="">Not filed under a client or production</option>
      {clients.map((c) => (
        <optgroup key={c.id} label={c.name}>
          <option value={`cl:${c.id}`}>{c.name}, no production</option>
          {prods.filter((pr) => pr.client_id === c.id).map((pr) => <option key={pr.id} value={`pr:${pr.id}`}>{c.name} › {pr.name}</option>)}
        </optgroup>
      ))}
      {prods.some((pr) => !pr.client_id) && (
        <optgroup label="No client">
          {prods.filter((pr) => !pr.client_id).map((pr) => <option key={pr.id} value={`pr:${pr.id}`}>{pr.name}</option>)}
        </optgroup>
      )}
    </select>
  );
}

export type StructEdit =
  | { kind: 'new-client'; org?: string }
  | { kind: 'new-production'; org?: string; client?: string }
  | { kind: 'client'; org: string; c: Client }
  | { kind: 'production'; org: string; pr: Production }
  | { kind: 'file'; p: ProjectSummary };

/** Make, rename or move a client or production, or file a project. */
export function StructureDialog({ edit, orgs, structures, onClose, onDone }: {
  edit: StructEdit; orgs: Org[]; structures: Record<string, OrgStructure>; onClose: () => void; onDone: () => void;
}) {
  const { run, busy } = useAction();
  const { toast, confirm } = useUi();
  const startOrg = edit.kind === 'file' ? edit.p.org.id : edit.org || (orgs.length === 1 ? orgs[0].id : '');
  const [org, setOrg] = useState(startOrg);
  const [name, setName] = useState(edit.kind === 'client' ? edit.c.name : edit.kind === 'production' ? edit.pr.name : '');
  const [client, setClient] = useState(edit.kind === 'production' ? edit.pr.client_id || '' : edit.kind === 'new-production' ? edit.client || '' : '');
  const [place, setPlace] = useState(edit.kind === 'file' ? placeOf(edit.p) : '');
  const [err, setErr] = useState('');
  const s = structures[org];
  const pickOrg = (edit.kind === 'new-client' || edit.kind === 'new-production') && !edit.org && orgs.length > 1;

  async function save() {
    setErr('');
    if (edit.kind === 'file') {
      if (place === placeOf(edit.p)) return onClose();
      const r = await run('s', 'file the project', () => api.admin.updateProject(edit.p.id, placeBody(place)));
      if (!r) return;
      toast(r.production ? `${r.name} is now in ${r.client ? `${r.client.name} › ` : ''}${r.production.name}.` : r.client ? `${r.name} is now under ${r.client.name}.` : `${r.name} is no longer filed under a client or production.`);
      return onDone();
    }
    if (!name.trim()) return setErr(`Give the ${edit.kind.includes('client') ? 'client' : 'production'} a name.`);
    if (!org) return setErr('Choose an organization.');
    if (edit.kind === 'new-client') {
      const r = await run('s', 'make the client', () => api.admin.createClient(org, name.trim()));
      if (!r) return;
      toast(`Made the client ${r.name}. File projects or productions under it.`);
    } else if (edit.kind === 'new-production') {
      const r = await run('s', 'make the production', () => api.admin.createProduction(org, name.trim(), client || undefined));
      if (!r) return;
      toast(`Made the production ${r.name}. File its projects under it, then choose its main project.`);
    } else if (edit.kind === 'client') {
      if (name.trim() !== edit.c.name && !(await run('s', 'rename the client', () => api.admin.updateClient(edit.c.id, name.trim())))) return;
      toast(`Saved ${name.trim()}.`);
    } else {
      const body: { name?: string; client?: string | null } = {};
      if (name.trim() !== edit.pr.name) body.name = name.trim();
      if ((client || null) !== edit.pr.client_id) body.client = client || null;
      if (!Object.keys(body).length) return onClose();
      if (!(await run('s', 'change the production', () => api.admin.updateProduction(edit.pr.id, body)))) return;
      toast(body.client !== undefined && edit.pr.projects ? `Saved. Its ${edit.pr.projects === 1 ? 'project moves' : 'projects move'} with it.` : `Saved ${name.trim()}.`);
    }
    onDone();
  }

  async function remove() {
    if (edit.kind === 'client') {
      const c = edit.c;
      const ok = await confirm({
        title: `Remove the client ${c.name}?`,
        what: <>Only the client goes. {c.productions || c.projects ? <>Its {[c.productions ? `${c.productions} production${c.productions === 1 ? '' : 's'}` : '', c.projects ? `${c.projects} project${c.projects === 1 ? '' : 's'}` : ''].filter(Boolean).join(' and ')} stay, no longer under a client, with all their files and history.</> : 'Nothing is filed under it.'}</>,
        confirm: 'Remove client',
        danger: true,
      });
      if (!ok || !(await run('rm', 'remove the client', () => api.admin.deleteClient(c.id)))) return;
      toast(`Removed the client ${c.name}.`);
    } else if (edit.kind === 'production') {
      const pr = edit.pr;
      const ok = await confirm({
        title: `Remove the production ${pr.name}?`,
        what: <>Only the production goes. {pr.projects ? <>Its {pr.projects === 1 ? 'project stays' : `${pr.projects} projects stay`}, with all files and history{pr.client_id ? ', still under the same client' : ''}.</> : 'It has no projects.'}</>,
        confirm: 'Remove production',
        danger: true,
      });
      if (!ok || !(await run('rm', 'remove the production', () => api.admin.deleteProduction(pr.id)))) return;
      toast(`Removed the production ${pr.name}.`);
    }
    onDone();
  }

  const title = {
    'new-client': 'New client',
    'new-production': 'New production',
    client: edit.kind === 'client' ? `Client ${edit.c.name}` : '',
    production: edit.kind === 'production' ? `Production ${edit.pr.name}` : '',
    file: edit.kind === 'file' ? `File ${edit.p.name} under…` : '',
  }[edit.kind];

  return (
    <Dialog title={title} onClose={onClose}>
      {edit.kind === 'new-client' && <p className="fine">Who the work is for. Projects and productions can be filed under a client; the client itself holds no files.</p>}
      {edit.kind === 'new-production' && <p className="fine">A show, game or campaign. Each project in it (Art, Audio, Code…) keeps its own history and access; one can be its main project.</p>}
      {pickOrg && (
        <label className="field">
          <span>Organization</span>
          <select className="select" value={org} onChange={(e) => { setOrg(e.target.value); setClient(''); }}>
            <option value="">Choose an organization</option>
            {orgs.map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
          </select>
        </label>
      )}
      {edit.kind !== 'file' && (
        <label className="field">
          <span>Name</span>
          <input className="input" autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder={edit.kind.includes('client') ? 'Northwind Games' : 'Emberfall'} onKeyDown={(e) => e.key === 'Enter' && save()} />
        </label>
      )}
      {(edit.kind === 'new-production' || edit.kind === 'production') && (
        <label className="field">
          <span>Client</span>
          <select className="select" value={client} onChange={(e) => setClient(e.target.value)} disabled={!org}>
            <option value="">No client (in-house)</option>
            {(s?.clients || []).map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
          </select>
          {edit.kind === 'production' && edit.pr.projects > 0 && <small>Its projects move with it.</small>}
        </label>
      )}
      {edit.kind === 'file' && (
        <label className="field">
          <span>File under</span>
          <PlaceSelect s={s} value={place} onChange={setPlace} />
          <small>Nothing about the project changes but where it is listed: files, history, branches and access stay as they are.</small>
        </label>
      )}
      {err && <div className="notice bad">{err}</div>}
      <div className="actions">
        {(edit.kind === 'client' || edit.kind === 'production') && <button className="btn danger" style={{ marginRight: 'auto' }} disabled={busy === 'rm'} onClick={remove}>Remove…</button>}
        <button className="btn" onClick={onClose}>Cancel</button>
        <button className="btn pri" onClick={save} disabled={busy === 's'}>
          {edit.kind === 'new-client' ? 'Make client' : edit.kind === 'new-production' ? 'Make production' : edit.kind === 'file' ? 'File it here' : 'Save'}
        </button>
      </div>
    </Dialog>
  );
}
