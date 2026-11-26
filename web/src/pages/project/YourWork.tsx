import { useEffect, useMemo, useState } from 'react';
import { api, ApiError } from '../../api/client';
import { daemon } from '../../api/daemon';
import type { IgnoreChoice, LocalFile } from '../../api/types';
import { useDaemon } from '../../lib/daemon';
import { useSession } from '../../lib/session';
import { baseName, clock, firstName, plural, since, size } from '../../lib/format';
import { Dialog, Loading, Thumb, useUi } from '../../components/ui';
import { useProject } from './context';

type Item = LocalFile & { kind: 'new' | 'edited' | 'deleted' };

export default function YourWork() {
  const { project, ws, status, reloadStatus, branch, branches, claims, bump, nudges, dismissNudge } = useProject();
  const { me } = useSession();
  const d = useDaemon();
  const { toast, fail } = useUi();
  const [message, setMessage] = useState('');
  const [left, setLeft] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState<string | null>(null);
  const [cpOpen, setCpOpen] = useState(false);
  const [cpNote, setCpNote] = useState('');
  const [draftOpen, setDraftOpen] = useState(false);
  const [draftTitle, setDraftTitle] = useState('');
  // "Don't checkpoint files like this": the file it was opened on, and the patterns on offer
  const [skipFor, setSkipFor] = useState<string | null>(null);
  const [skipChoices, setSkipChoices] = useState<IgnoreChoice[] | null>(null);
  const [skipPersonal, setSkipPersonal] = useState(false);

  const items: Item[] = useMemo(
    () =>
      status
        ? [
            ...status.edited.map((f) => ({ ...f, kind: 'edited' as const })),
            ...status.new.map((f) => ({ ...f, kind: 'new' as const })),
            ...status.deleted.map((f) => ({ ...f, kind: 'deleted' as const })),
          ]
        : [],
    [status],
  );
  useEffect(() => setLeft((l) => new Set([...l].filter((p) => items.some((i) => i.path === p)))), [items]);

  // claimed but not edited files are still "your work": they block others
  const claimedOnly = claims.filter((c) => c.user.id === me?.id && !items.some((i) => i.path === c.path));

  if (!ws) return null;
  const wsBranch = branches.find((b) => b.id === ws.branch_id);
  const on = items.filter((i) => !left.has(i.path));
  const bytes = on.reduce((a, f) => a + f.size, 0);
  const newCount = on.filter((i) => i.kind === 'new').length;
  const lastNamed = status?.checkpoints.filter((c) => !c.auto).sort((a, b) => b.created_at.localeCompare(a.created_at))[0];
  const lastAny = status?.checkpoints.slice().sort((a, b) => b.created_at.localeCompare(a.created_at))[0];
  const nextNumber = project.head_change + 1;

  const toggle = (p: string) => {
    const n = new Set(left);
    if (n.has(p)) n.delete(p);
    else n.add(p);
    setLeft(n);
  };

  async function publish() {
    if (!ws) return;
    if (!message.trim()) {
      fail(new ApiError(422, 'invalid', 'Say what you changed first, so the team knows what this change is.'));
      return;
    }
    setBusy('publish');
    try {
      const r = await daemon.publish(ws.id, message.trim(), on.map((i) => i.path));
      toast(`Published change ${r.change.number}.`);
      setMessage('');
      setLeft(new Set());
      bump('changes');
      bump('claims');
      reloadStatus();
      d.refresh();
    } catch (e) {
      if (e instanceof ApiError && e.code === 'conflict') {
        fail(new ApiError(409, 'conflict', `${e.message || 'Someone else holds one of these files.'} Untick it, or ask them to release it, then publish again.`));
      } else fail(e, 'publish');
    } finally {
      setBusy(null);
    }
  }

  async function getLatest() {
    if (!ws) return;
    setBusy('latest');
    try {
      const r = await daemon.getLatest(ws.id);
      toast(r.downloaded ? `Got latest: ${plural(r.downloaded, 'file')}, ${size(r.bytes)}.` : 'Already up to date.');
      reloadStatus();
      d.refresh();
    } catch (e) {
      fail(e, 'get latest');
    } finally {
      setBusy(null);
    }
  }

  async function checkpoint() {
    if (!ws) return;
    setBusy('checkpoint');
    try {
      await daemon.checkpoint(ws.id, cpNote.trim() || undefined);
      toast(cpNote.trim() ? `Checkpoint saved: ${cpNote.trim()}.` : 'Checkpoint saved.');
      setCpOpen(false);
      setCpNote('');
      reloadStatus();
    } catch (e) {
      fail(e, 'save a checkpoint');
    } finally {
      setBusy(null);
    }
  }

  async function openSkip(path: string) {
    if (!ws) return;
    setSkipFor(path);
    setSkipChoices(null);
    try {
      setSkipChoices(await daemon.ignoreSuggest(ws.id, path));
    } catch (e) {
      setSkipFor(null);
      fail(e, 'read the patterns for this file');
    }
  }

  async function skip(pattern: string) {
    if (!ws) return;
    setBusy('skip');
    try {
      await daemon.noCheckpoint(ws.id, pattern, skipPersonal);
      toast(skipPersonal ? `Checkpoints on this computer now leave out ${pattern}.` : `Checkpoints now leave out ${pattern}. The rule is shared when you publish.`);
      setSkipFor(null);
      reloadStatus();
    } catch (e) {
      fail(e, 'change what checkpoints leave out');
    } finally {
      setBusy(null);
    }
  }

  async function saveDraft() {
    if (!ws) return;
    setBusy('draft');
    try {
      await daemon.saveDraft(ws.id, draftTitle.trim() || message.trim() || 'Untitled draft', on.map((i) => i.path));
      toast('Draft saved on the server. Only you can see it.');
      setDraftOpen(false);
      setDraftTitle('');
      bump('drafts');
    } catch (e) {
      if (e instanceof ApiError && e.status === 404)
        fail(new ApiError(404, 'old_app', 'This version of the desktop app cannot save drafts yet. Update it, or save a checkpoint for now.'));
      else fail(e, 'save a draft');
    } finally {
      setBusy(null);
    }
  }

  async function release(path: string) {
    try {
      await api.release(project.id, ws!.branch_id, [path]);
      toast(`Released ${baseName(path)}.`);
      dismissNudge(path);
      bump('claims');
    } catch (e) {
      fail(e, `release ${baseName(path)}`);
    }
  }

  const behind = Math.max(status?.behind || 0, 0);
  const myClaimSince = (p: string) => claims.find((c) => c.path === p && c.user.id === me?.id)?.created_at;

  return (
    <aside className="work" aria-label="Your work">
      <h2>Your work</h2>
      <p>
        {!status
          ? 'Reading your folder'
          : !items.length
            ? 'Nothing to publish. Edits and new files in your folder show up here by themselves.'
            : on.length
              ? `${on.length} of ${plural(items.length, 'file')} going into change ${nextNumber}${newCount ? `, ${newCount} of them new` : ''}. ${size(bytes)} to send.`
              : 'Nothing ticked. Tick the files you want to publish.'}
      </p>
      {wsBranch && wsBranch.id !== branch.id && (
        <div className="notice acc">Your folder is on <b>{wsBranch.name}</b>. Publishing goes there. Switch branch in the desktop app.</div>
      )}
      {behind > 0 && (
        <div className="hid">
          <span>{plural(behind, 'newer file')} on the server</span>
          <button onClick={getLatest} disabled={busy === 'latest'}>{busy === 'latest' ? 'Getting latest' : 'Get latest'}</button>
        </div>
      )}
      {!status && <Loading what="Reading your folder" />}
      {items.length > 0 && (
        <ul>
          {items.map((f) => (
            <li
              key={f.path}
              className={left.has(f.path) ? 'off' : ''}
              tabIndex={0}
              title="Right-click for checkpoint options"
              onContextMenu={(e) => {
                if (f.kind === 'deleted') return;
                e.preventDefault();
                openSkip(f.path);
              }}
            >
              <input type="checkbox" aria-label={`Include ${baseName(f.path)}`} checked={!left.has(f.path)} onChange={() => toggle(f.path)} />
              <Thumb seed={f.path} size={44} />
              <div>
                <strong className="el" title={f.path}>{baseName(f.path)}</strong>
                <small>
                  {f.kind === 'new' ? <b>New</b> : f.kind === 'deleted' ? <b className="del">Deleted</b> : 'Edited'}
                  {f.claimed ? ', claimed' : ''}, {size(f.size)}
                </small>
              </div>
            </li>
          ))}
        </ul>
      )}
      {claimedOnly.length > 0 && (
        <div className="hid">
          <span>{plural(claimedOnly.length, 'file')} claimed but not edited</span>
        </div>
      )}
      {status && status.ignored > 0 && (
        <div className="hid">
          <span>{plural(status.ignored, 'file')} ignored by the ignore list</span>
          <span className="fine">See them in the desktop app</span>
        </div>
      )}
      {status?.checkpoint_notice && <div className="notice hold">{status.checkpoint_notice}</div>}
      <label className="sr" htmlFor="work-msg">Say what you changed</label>
      <textarea id="work-msg" className="textarea note" placeholder="Say what you changed" value={message} onChange={(e) => setMessage(e.target.value)} />
      <div className="hid">
        <span>
          {lastNamed ? `Last checkpoint ${since(lastNamed.created_at)}: ${lastNamed.note}` : lastAny ? `Last checkpoint ${since(lastAny.created_at)}, automatic` : 'No checkpoints yet'}
        </span>
        <button onClick={() => setCpOpen(true)} disabled={!items.length}>Save checkpoint</button>
      </div>
      <div className="row">
        <button className="btn" onClick={() => setDraftOpen(true)} disabled={!on.length || !!busy}>Save draft</button>
        <button className="btn pri grow" onClick={publish} disabled={!on.length || !!busy}>
          {busy === 'publish' ? 'Publishing' : on.length ? `Publish ${plural(on.length, 'file')}` : 'Publish'}
        </button>
      </div>
      {nudges.map((n) => (
        <div key={n.path} className="nudge">
          <b>{firstName(n.by)} is waiting for {baseName(n.path)}.</b>
          {myClaimSince(n.path) && <> You have held it since {clock(myClaimSince(n.path)!)}.</>}
          <div className="row">
            <button className="btn sm" onClick={() => release(n.path)}>Release it</button>
            <button className="btn sm" onClick={() => dismissNudge(n.path)}>Keep it for now</button>
          </div>
        </div>
      ))}
      {cpOpen && (
        <Dialog title="Save a checkpoint" onClose={() => setCpOpen(false)}>
          <p>
            A private save point of your unpublished work, kept {status?.checkpoint_storage === 'server' ? 'on the server, where only you can see it' : 'on this computer'}.
            You can rewind to it later from the desktop app. Render output and caches are left out.
            {status?.auto_checkpoints ? ' This project also takes checkpoints automatically while you work.' : ''}
          </p>
          <label className="field">
            <span>Note</span>
            <input className="input" placeholder="before retopo" value={cpNote} onChange={(e) => setCpNote(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && checkpoint()} />
            <small>Named checkpoints are kept until you publish or discard the work.</small>
          </label>
          <div className="actions">
            <button className="btn" onClick={() => setCpOpen(false)}>Cancel</button>
            <button className="btn pri" onClick={checkpoint} disabled={busy === 'checkpoint'}>Save checkpoint</button>
          </div>
        </Dialog>
      )}
      {skipFor && (
        <Dialog title="Leave out of checkpoints" onClose={() => setSkipFor(null)}>
          <p>
            Checkpoints copy your unpublished files so you can go back. Files that can be made again, like renders and caches, are better left out.
            Choose what to leave out, starting from <b>{baseName(skipFor)}</b>. Publishing these files is not affected.
          </p>
          {!skipChoices && <Loading what="Reading patterns" />}
          {skipChoices?.map((c) => (
            <div key={c.pattern} className="hid">
              <span>
                {c.label}
                <span className="fine"> {c.pattern}, {plural(c.matches, 'file')}, {size(c.bytes)}</span>
              </span>
              <button onClick={() => skip(c.pattern)} disabled={busy === 'skip'}>Leave out</button>
            </div>
          ))}
          <label className="check-label">
            <input type="checkbox" checked={skipPersonal} onChange={(e) => setSkipPersonal(e.target.checked)} />
            Only on this computer, not for the whole project
          </label>
          <div className="actions">
            <button className="btn" onClick={() => setSkipFor(null)}>Cancel</button>
          </div>
        </Dialog>
      )}
      {draftOpen && (
        <Dialog title="Save as a draft" onClose={() => setDraftOpen(false)}>
          <p>Sends the {plural(on.length, 'ticked file')} to the server without publishing. Only you can see a draft, and it is also a backup of your work.</p>
          <label className="field">
            <span>Draft name</span>
            <input className="input" placeholder={message || 'Villager rig, half done'} value={draftTitle} onChange={(e) => setDraftTitle(e.target.value)} />
          </label>
          <div className="actions">
            <button className="btn" onClick={() => setDraftOpen(false)}>Cancel</button>
            <button className="btn pri" onClick={saveDraft} disabled={busy === 'draft'}>{busy === 'draft' ? 'Saving draft' : 'Save draft'}</button>
          </div>
        </Dialog>
      )}
    </aside>
  );
}
