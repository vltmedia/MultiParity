// Access tokens: sign scripts and AI assistants in as you, without your password. A token has
// exactly your access and lasts until revoked. The assistant setup uses the server's /api/mcp.
import { useState } from 'react';
import TopBar from '../components/TopBar';
import { api } from '../api/client';
import type { Session } from '../api/types';
import { useAsync } from '../lib/useAsync';
import { since } from '../lib/format';
import { CopyButton, Empty, ErrorNote, Loading, useAction, useUi } from '../components/ui';

export default function Tokens() {
  const list = useAsync(() => api.myTokens(), []);
  const { run, busy } = useAction();
  const { confirm, toast } = useUi();
  const [label, setLabel] = useState('');
  const [made, setMade] = useState<{ token: string; label: string } | null>(null);
  const [err, setErr] = useState('');
  const tokens = (list.data || []).filter((s) => s.kind === 'token');
  const mcpUrl = `${location.origin}/api/mcp`;

  async function make() {
    setErr('');
    if (!label.trim()) return setErr('Name it after where it will be used, like "Claude on my laptop".');
    const r = await run('make', 'make the token', () => api.createToken(label.trim()));
    if (!r) return;
    setMade(r);
    setLabel('');
    list.reload();
  }

  async function revoke(s: Session) {
    const ok = await confirm({
      title: `Revoke ${s.label || 'this token'}?`,
      what: <>Anything using it is signed out straight away and cannot sign in with it again. Your files, claims and work are not touched.</>,
      confirm: 'Revoke token',
      danger: true,
    });
    if (!ok) return;
    if (await run('rv:' + s.id, 'revoke the token', () => api.revokeToken(s.id))) {
      toast(`Revoked ${s.label || 'the token'}.`);
      if (made && s.label === made.label) setMade(null);
      list.reload();
    }
  }

  const claude = made ? `claude mcp add --transport http multiparity ${mcpUrl} --header "Authorization: Bearer ${made.token}"` : '';
  const config = made ? JSON.stringify({ mcpServers: { multiparity: { type: 'http', url: mcpUrl, headers: { Authorization: `Bearer ${made.token}` } } } }, null, 2) : '';

  return (
    <>
      <TopBar />
      <div className="page tokens-page">
        <header className="head">
          <div>
            <h1>Access tokens</h1>
            <p>Sign scripts and AI assistants in as you, without your password. A token can see and change exactly what you can, until you revoke it.</p>
          </div>
        </header>
        <div className="body">
          <div className="panel">
            <h2>New token</h2>
            <div className="row" style={{ gap: 8, flexWrap: 'nowrap' }}>
              <input className="input" value={label} onChange={(e) => setLabel(e.target.value)} placeholder="Claude on my laptop" aria-label="Token name" onKeyDown={(e) => e.key === 'Enter' && make()} />
              <button className="btn pri" onClick={make} disabled={busy === 'make'}>Make token</button>
            </div>
            {err && <div className="notice bad">{err}</div>}
            {made && (
              <div className="result good token-made">
                <b>Copy {made.label} now: it is not shown again.</b>
                <div className="tok-row"><code>{made.token}</code><CopyButton text={made.token} label="Copy token" /></div>
                <h3>Use it in an AI assistant</h3>
                <p className="fine">Claude Code, in a terminal:</p>
                <div className="tok-row"><code>{claude}</code><CopyButton text={claude} label="Copy" /></div>
                <p className="fine">Other assistants that take an MCP server address (Claude Desktop, Cursor, VS Code…):</p>
                <div className="tok-row"><pre>{config}</pre><CopyButton text={config} label="Copy" /></div>
                <p className="fine">For scripts: send <code>Authorization: Bearer &lt;token&gt;</code> with any call to <code>{location.origin}/api</code>.</p>
              </div>
            )}
          </div>
          <div className="panel">
            <h2>Your tokens</h2>
            {list.loading && !list.data && <Loading what="Loading tokens" />}
            {list.error !== undefined && <ErrorNote error={list.error} doing="load your tokens" retry={list.reload} />}
            {list.data && !tokens.length && <Empty title="No tokens">Make one above when a script or assistant needs to sign in as you.</Empty>}
            {tokens.length > 0 && (
              <table className="t">
                <thead><tr><th>Name</th><th>Made</th><th>Last used</th><th /></tr></thead>
                <tbody>
                  {tokens.map((s) => (
                    <tr key={s.id}>
                      <td>{s.label || 'Unnamed'}</td>
                      <td>{since(s.created_at)}</td>
                      <td>{s.last_used_at ? since(s.last_used_at) : <span className="mute">Never</span>}</td>
                      <td className="r"><button className="btn sm danger" disabled={busy === 'rv:' + s.id} onClick={() => revoke(s)}>Revoke</button></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      </div>
    </>
  );
}
