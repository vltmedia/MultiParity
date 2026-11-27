import { api } from '../../api/client';
import { useAsync } from '../../lib/useAsync';
import { ErrorNote, Loading, useAction, useUi } from '../../components/ui';

/** Admin > Fixing: whether the server sends email, and a test. Server admins only. */
export default function EmailSection() {
  const e = useAsync(() => api.admin.email(), []);
  const { run, busy } = useAction();
  const { toast } = useUi();
  const d = e.data;

  async function test() {
    const r = await run('test', 'send a test email', () => api.admin.emailTest());
    if (r) toast(`Sent a test email to ${r.sent_to}. Check that it arrived (and not in spam).`);
  }

  return (
    <section className="section">
      <h2>Email</h2>
      <p>The server emails invites, "forgot password" links, requests to release a file, and failed backups to server admins.</p>
      {e.loading && !d && <Loading what="Looking at email" />}
      {e.error !== undefined && <ErrorNote error={e.error} doing="load the email settings" retry={e.reload} />}
      {d && !d.configured && (
        <div className="notice">
          <b>Email is off.</b> Invite links are copied by hand and admins set new passwords. To turn it on, set an SMTP server or a SendGrid key in the server's settings (docs/EMAIL.md) and restart it.
        </div>
      )}
      {d && d.configured && (
        <div className="toolbar" style={{ alignItems: 'center' }}>
          <span style={{ flex: 1 }}>
            Sending through <b>{d.via}</b> as {d.from}.
          </span>
          <button className="btn sm" onClick={test} disabled={busy === 'test'}>{busy === 'test' ? 'Sending' : 'Send me a test email'}</button>
        </div>
      )}
    </section>
  );
}
