import { useEffect } from 'react';
import { api } from '../../api/client';
import { useAsync } from '../../lib/useAsync';
import { plural, since, size } from '../../lib/format';
import { ErrorNote, Loading, useAction, useUi } from '../../components/ui';

/** Admin > Fixing: nightly backups, their status and "Back up now". Server admins only. */
export default function BackupsSection() {
  const b = useAsync(() => api.admin.backups(), []);
  const { run, busy } = useAction();
  const { toast } = useUi();
  const d = b.data;
  const running = !!d && 'running' in d && d.running;
  // While a backup runs, look again every few seconds.
  useEffect(() => {
    if (!running) return;
    const t = setTimeout(b.reload, 3000);
    return () => clearTimeout(t);
  }, [running, b.data, b.reload]);

  async function backUpNow() {
    const r = await run('backup', 'start a backup', () => api.admin.backupNow());
    if (r) {
      toast('Backup started. It runs in the background.');
      setTimeout(b.reload, 800);
    }
  }

  const last = d && d.configured ? d.runs[0] : undefined;
  return (
    <section className="section">
      <h2>Backups</h2>
      <p>Every night the server copies its database and everyone's file data to a backup place. Only new file data is copied each time.</p>
      {b.loading && !d && <Loading what="Looking at backups" />}
      {b.error !== undefined && <ErrorNote error={b.error} doing="load backups" retry={b.reload} />}
      {d && !d.configured && (
        <div className="notice bad">
          <b>Backups are off.</b> Set <code>MP_BACKUP_TARGET</code> in the server's settings to a folder, a second drive or an S3 bucket, then restart it. docs/BACKUP.md explains how.
        </div>
      )}
      {d && d.configured && (
        <>
          {d.target_error && (
            <div className="notice bad" role="alert"><b>The backup place cannot be reached.</b> {d.target_error}</div>
          )}
          {last && last.status === 'failed' && (
            <div className="notice bad" role="alert"><b>The last backup failed</b> {since(last.started_at)}: {last.error}</div>
          )}
          <div className="toolbar" style={{ alignItems: 'center' }}>
            <span style={{ flex: 1 }}>
              {running ? (
                <b>Backing up now…</b>
              ) : last ? (
                <>Last backup <b>{since(last.finished_at || last.started_at)}</b>{last.status === 'done' ? `, ${size(last.new_chunk_bytes || 0)} of new file data` : ''}. </>
              ) : (
                <>No backups yet. </>
              )}
              <span className="fine">Next at {d.schedule}, to {d.target}. Keeps {d.keep_daily} daily and {d.keep_weekly} weekly.</span>
            </span>
            <button className="btn sm" onClick={backUpNow} disabled={running || busy === 'backup'}>{running ? 'Backing up' : 'Back up now'}</button>
          </div>
          {d.snapshots.length > 0 && (
            <table className="t">
              <thead>
                <tr>
                  <th>Backup</th>
                  <th className="r">Database</th>
                  <th className="r">File data</th>
                  <th className="r">New that night</th>
                </tr>
              </thead>
              <tbody>
                {d.snapshots.map((s) => (
                  <tr key={s.id}>
                    <td>{since(s.created_at)} <span className="fine">{s.id}</span></td>
                    <td className="r">{size(s.db_bytes)}</td>
                    <td className="r">{plural(s.chunks, 'part')}</td>
                    <td className="r">{size(s.new_chunk_bytes)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <p className="fine">To restore, see docs/BACKUP.md: it takes one command on the server, into an empty database.</p>
        </>
      )}
    </section>
  );
}
