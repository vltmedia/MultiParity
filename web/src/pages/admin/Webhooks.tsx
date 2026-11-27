import { useState } from 'react';
import { api } from '../../api/client';
import { useAsync } from '../../lib/useAsync';
import { since } from '../../lib/format';
import { CopyButton, ErrorNote, Loading, useAction, useUi } from '../../components/ui';
import { useAdmin } from './AdminLayout';

const PRESETS: Array<[string, string]> = [
  ['', 'Everything'],
  ['change.*', 'Publishes and merges'],
  ['claim.*', 'Claims'],
  ['change.*,claim.*,draft.saved', 'Work (changes, claims, drafts)'],
];

/** Admin > Fixing: webhooks of the chosen organization. */
export default function WebhooksSection() {
  const { scope, scopeName } = useAdmin();
  const hooks = useAsync(() => (scope ? api.admin.webhooks(scope) : Promise.resolve([])), [scope]);
  const { run, busy } = useAction();
  const { toast, confirm } = useUi();
  const [url, setUrl] = useState('');
  const [events, setEvents] = useState('change.*');
  const [secret, setSecret] = useState<string | null>(null);

  async function add() {
    const r = await run('add', 'add the webhook', () => api.admin.addWebhook(scope, { url: url.trim(), events: events ? events.split(',') : [] }));
    if (r) {
      setSecret(r.secret || null);
      setUrl('');
      hooks.reload();
    }
  }
  async function test(id: string) {
    const r = await run('test' + id, 'send a test event', () => api.admin.testWebhook(id));
    if (r) toast(r.ok ? `Delivered (answered ${r.status}).` : `Not delivered: ${r.error}`, r.ok ? undefined : 'bad');
    hooks.reload();
  }
  async function toggle(id: string, active: boolean) {
    if (await run('toggle' + id, active ? 'resume the webhook' : 'pause the webhook', () => api.admin.updateWebhook(id, { active }))) hooks.reload();
  }
  async function remove(id: string, u: string) {
    if (!(await confirm({ title: 'Delete this webhook?', what: `Events stop going to ${u}.`, confirm: 'Delete', danger: true }))) return;
    if (await run('del' + id, 'delete the webhook', () => api.admin.deleteWebhook(id))) hooks.reload();
  }

  return (
    <section className="section">
      <h2>Webhooks</h2>
      <p>Send events to another service as they happen: a chat channel, a build server, your own script. Each is posted as JSON and signed with the webhook's secret (docs/AUTOMATION.md).</p>
      {!scope && <div className="notice">Choose an organization at the top to set up its webhooks.</div>}
      {scope && (
        <>
          {hooks.loading && !hooks.data && <Loading what="Loading webhooks" />}
          {hooks.error !== undefined && <ErrorNote error={hooks.error} doing="load webhooks" retry={hooks.reload} />}
          {hooks.data && hooks.data.length > 0 && (
            <table className="t">
              <thead>
                <tr><th>Address</th><th>Events</th><th>Last delivery</th><th /></tr>
              </thead>
              <tbody>
                {hooks.data.map((h) => (
                  <tr key={h.id}>
                    <td style={{ wordBreak: 'break-all' }}>{h.url}{!h.active && <span className="st theirs"> Paused</span>}</td>
                    <td>{h.events.length ? h.events.join(', ') : 'Everything'}</td>
                    <td className="fine">
                      {h.last_at ? `${since(h.last_at)}: ${h.last_error ? h.last_error : `answered ${h.last_status}`}` : 'None yet'}
                      {h.failures > 0 && ` (${h.failures} failed in a row)`}
                    </td>
                    <td className="r nowrap">
                      <button className="btn sm" disabled={!!busy} onClick={() => test(h.id)}>Send a test</button>{' '}
                      <button className="btn sm" disabled={!!busy} onClick={() => toggle(h.id, !h.active)}>{h.active ? 'Pause' : 'Resume'}</button>{' '}
                      <button className="btn sm" disabled={!!busy} onClick={() => remove(h.id, h.url)}>Delete</button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {secret && (
            <div className="result good">
              <b>Webhook added. Copy its secret now: it is not shown again.</b>
              <ul className="links"><li><code>{secret}</code><CopyButton text={secret} /></li></ul>
              <p className="fine">The receiving service checks <code>X-MultiParity-Signature</code> (HMAC-SHA256 of the body) with it.</p>
            </div>
          )}
          <div className="toolbar" style={{ alignItems: 'flex-end', flexWrap: 'wrap', gap: 10, marginTop: 12 }}>
            <label className="field" style={{ flex: '1 1 280px' }}>
              <span>Send {scopeName}'s events to</span>
              <input className="input" placeholder="https://hooks.example.com/multiparity" value={url} onChange={(e) => setUrl(e.target.value)} />
            </label>
            <label className="field">
              <span>Which events</span>
              <select className="select" value={events} onChange={(e) => setEvents(e.target.value)}>
                {PRESETS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
              </select>
            </label>
            <button className="btn pri" disabled={!url.trim() || busy === 'add'} onClick={add}>Add webhook</button>
          </div>
        </>
      )}
    </section>
  );
}
