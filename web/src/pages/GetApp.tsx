import TopBar from '../components/TopBar';
import { api } from '../api/client';
import { useAsync } from '../lib/useAsync';
import { ErrorNote, Loading } from '../components/ui';
import { size } from '../lib/format';

const PLATFORM: Record<string, string> = {
  'windows-x86_64': 'Windows',
  'windows-aarch64': 'Windows on Arm',
  'darwin-aarch64': 'Mac with Apple silicon',
  'darwin-x86_64': 'Mac with Intel',
  'linux-x86_64': 'Linux (AppImage)',
};

function guess(): string {
  const ua = navigator.userAgent;
  if (/Windows/.test(ua)) return 'windows-x86_64';
  if (/Mac/.test(ua)) return 'darwin-aarch64';
  return 'linux-x86_64';
}

/** Downloads of the desktop app this server hands out. */
export default function GetApp() {
  const info = useAsync(() => api.desktop(), []);
  const mine = guess();
  const list = [...(info.data?.downloads || [])].sort((a, b) => (a.platform === mine ? -1 : b.platform === mine ? 1 : a.platform.localeCompare(b.platform)));
  return (
    <>
      <TopBar />
      <div className="page">
        <header className="head">
          <div>
            <h1>The desktop app</h1>
            <p>Keeps your project folders in sync, claims files as you start editing them, and tells you when others publish. It updates itself from this server.</p>
          </div>
        </header>
        <div className="body">
          {info.loading && !info.data && <Loading what="Looking for downloads" />}
          {info.error !== undefined && <ErrorNote error={info.error} doing="look for downloads" retry={info.reload} />}
          {info.data && !list.length && (
            <div className="notice">This server has no desktop app to hand out yet. An admin adds it by copying a build to the server (docs/DESKTOP.md in the MultiParity repository).</div>
          )}
          {list.length > 0 && (
            <>
              <p className="fine" style={{ marginBottom: 12 }}>Version {info.data!.version}{info.data!.notes ? `: ${info.data!.notes}` : ''}</p>
              <ul className="getapp">
                {list.map((d) => (
                  <li key={d.platform}>
                    <div>
                      <b>{PLATFORM[d.platform] || d.platform}</b>
                      <small>{d.name}{d.size ? `, ${size(d.size)}` : ''}</small>
                    </div>
                    <a className={`btn${d.platform === mine ? ' pri' : ''}`} href={d.url}>Download</a>
                  </li>
                ))}
              </ul>
              <p className="fine" style={{ marginTop: 14 }}>After installing, sign in with the same email and password you use here, then choose which projects to keep on your computer.</p>
            </>
          )}
        </div>
      </div>
    </>
  );
}
