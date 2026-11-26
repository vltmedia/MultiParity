import { useEffect, useMemo, useState } from 'react';
import { daemon } from '../../api/daemon';
import type { FolderNode, Workspace } from '../../api/types';
import { useAsync } from '../../lib/useAsync';
import { size, sizeOf } from '../../lib/format';
import { ErrorNote, Loading, TriCheck, useUi } from '../../components/ui';

type State = 'all' | 'some' | 'none';

/** Own files of a folder (not in subfolders). */
const direct = (n: FolderNode) => n.size - n.children.reduce((a, c) => a + c.size, 0);

function initial(n: FolderNode, out: Map<string, State>) {
  out.set(n.path, n.included);
  n.children.forEach((c) => initial(c, out));
  return out;
}

function setDeep(n: FolderNode, v: State, out: Map<string, State>) {
  out.set(n.path, v);
  n.children.forEach((c) => setDeep(c, v, out));
}

/** Recompute a folder from its children after a change below it. */
function derive(n: FolderNode, s: Map<string, State>): State {
  if (!n.children.length) return s.get(n.path) || 'none';
  const kids = n.children.map((c) => derive(c, s));
  let v: State = kids.every((k) => k === 'all') ? 'all' : kids.every((k) => k === 'none') ? 'none' : 'some';
  if (direct(n) > 0 && v === 'none' && s.get(n.path) !== 'none') v = 'some';
  if (direct(n) > 0 && v === 'all' && s.get(n.path) === 'none') v = 'some';
  s.set(n.path, v);
  return v;
}

/**
 * Bytes on this computer once saved, matching what rules() sends: a partly ticked folder keeps its
 * own files and follows its subfolders.
 */
function planned(n: FolderNode, s: Map<string, State>): number {
  const v = s.get(n.path);
  if (v === 'all') return n.size;
  if (v === 'none') return 0;
  return direct(n) + n.children.reduce((a, c) => a + planned(c, s), 0);
}

/** Smallest include/exclude lists that describe the ticks. */
function rules(n: FolderNode, s: Map<string, State>, parentIn: boolean, inc: string[], exc: string[]) {
  const v = s.get(n.path);
  if (v === 'all') {
    if (!parentIn) inc.push(n.path);
    return;
  }
  if (v === 'none') {
    if (parentIn) exc.push(n.path);
    return;
  }
  let here = parentIn;
  if (direct(n) > 0 && !parentIn) {
    inc.push(n.path);
    here = true;
  }
  n.children.forEach((c) => rules(c, s, here, inc, exc));
}

function find(n: FolderNode, path: string): FolderNode | undefined {
  if (n.path === path) return n;
  for (const c of n.children) {
    const f = find(c, path);
    if (f) return f;
  }
  return undefined;
}

export default function SyncPanel({ ws, onSaved, onClose, startAll }: { ws: Workspace; onSaved: (w: Workspace) => void; onClose: () => void; startAll?: boolean }) {
  const tree = useAsync(() => daemon.folders(ws.id), [ws.id]);
  const { confirm, fail, toast } = useUi();
  const [state, setState] = useState<Map<string, State>>(new Map());
  const [open, setOpen] = useState<Set<string>>(new Set(['']));
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!tree.data) return;
    const s = initial(tree.data, new Map());
    if (startAll) setDeep(tree.data, 'all', s);
    setState(s);
  }, [tree.data, startAll]);

  const root = tree.data;
  const after = useMemo(() => (root && state.size ? planned(root, state) : 0), [root, state]);

  function toggle(n: FolderNode, on: boolean) {
    if (!root) return;
    const s = new Map(state);
    setDeep(n, on ? 'all' : 'none', s);
    derive(root, s);
    setState(s);
  }

  async function save() {
    if (!root) return;
    const inc: string[] = [];
    const exc: string[] = [];
    rules(root, state, false, inc, exc);
    const removing = root.local_size - Math.min(root.local_size, after);
    const removedFolders = [...state.entries()].filter(([p, v]) => v === 'none' && (find(root, p)?.local_size || 0) > 0 && root.included !== 'none');
    if (removedFolders.length && removing > 0) {
      const ok = await confirm({
        title: 'Remove folders from this computer?',
        what: (
          <>
            Files in the unticked folders that are the same as on the server are removed from this computer, about {size(removing)}.
            Edited and new files stay where they are, and you can tick the folders again at any time.
          </>
        ),
        confirm: 'Remove and save',
      });
      if (!ok) return;
    }
    setBusy(true);
    try {
      const w = await daemon.setSync(ws.id, inc, exc);
      toast(after > root.local_size ? `Saved. Downloading ${size(after - root.local_size)} in the background.` : 'Saved. Folders on this computer are updated.');
      onSaved(w);
    } catch (e) {
      fail(e, 'save which folders are on this computer');
    } finally {
      setBusy(false);
    }
  }

  function row(n: FolderNode, depth: number): JSX.Element[] {
    const v = state.get(n.path) || n.included;
    const isOpen = open.has(n.path);
    const out = [
      <li key={n.path || '/'} className={`fold ${v}`} style={{ paddingLeft: 10 + depth * 22 }}>
        {n.children.length ? (
          <button
            className="twisty"
            aria-label={isOpen ? `Close ${n.name}` : `Open ${n.name}`}
            aria-expanded={isOpen}
            onClick={() => {
              const o = new Set(open);
              if (isOpen) o.delete(n.path);
              else o.add(n.path);
              setOpen(o);
            }}
          >
            {isOpen ? '−' : '+'}
          </button>
        ) : (
          <span className="twisty" />
        )}
        <TriCheck state={v} label={`Keep ${n.name || 'everything'} on this computer`} onChange={(on) => toggle(n, on)} />
        <span className="el fname">{depth === 0 ? 'Everything' : n.name}</span>
        <span className="fsize">{n.local_size > 0 && n.local_size < n.size ? sizeOf(n.local_size, n.size) : size(n.size)}</span>
      </li>,
    ];
    if (isOpen) n.children.forEach((c) => out.push(...row(c, depth + 1)));
    return out;
  }

  return (
    <div className="syncpanel">
      <div className="sp-head">
        <div>
          <strong>Folders on this computer</strong>
          <span className="fine"> in {ws.root}</span>
        </div>
        {root && <div className="sp-total"><b>{sizeOf(after, root.size)}</b> after saving</div>}
      </div>
      {tree.loading && !root && <Loading what="Reading the folder tree" />}
      {tree.error !== undefined && <ErrorNote error={tree.error} doing="read the folders from the app on this computer" retry={tree.reload} />}
      {root && <ul className="foldtree">{row(root, 0)}</ul>}
      <p className="fine">Ticking a folder ticks everything in it. Unticking removes only files that match the server; edited and new files always stay.</p>
      <div className="row">
        <button className="btn sm" onClick={onClose}>Cancel</button>
        <button className="btn pri sm" disabled={!root || busy} onClick={save}>{busy ? 'Saving' : 'Save folders'}</button>
      </div>
    </div>
  );
}
