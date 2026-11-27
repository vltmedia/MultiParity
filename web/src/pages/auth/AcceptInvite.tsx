import { useState, type FormEvent } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { api, explain } from '../../api/client';
import type { InviteInfo } from '../../api/types';
import { useSession } from '../../lib/session';
import { useAsync } from '../../lib/useAsync';
import { ROLE_LABEL, until } from '../../lib/format';
import { ErrorNote, Loading } from '../../components/ui';
import { AuthBrand } from './SignIn';

/** "Kitlig - Tests", "Kitlig - Tests and Cinder Works", "A, B and C". */
function joinNames(i: InviteInfo) {
  const names = [i.org.name, ...(i.also || []).map((a) => a.org.name)];
  return names.length === 1 ? names[0] : `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`;
}

export default function AcceptInvite() {
  const { token = '' } = useParams();
  const { me, signIn } = useSession();
  const navigate = useNavigate();
  const info = useAsync(() => api.invite(token), [token]);
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [again, setAgain] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  async function accept(e?: FormEvent) {
    e?.preventDefault();
    setError('');
    const needEmail = !me && !info.data?.email;
    if (!me) {
      if (!name.trim()) return setError('Enter the name people will see next to your work.');
      if (needEmail && !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email.trim())) return setError('Enter the email address you will sign in with.');
      if (password.length < 10) return setError('Choose a password with at least 10 characters.');
      if (password !== again) return setError('The two passwords are not the same. Type them again.');
    }
    setBusy(true);
    try {
      const r = await api.acceptInvite(token, me ? {} : { name: name.trim(), password, ...(needEmail ? { email: email.trim() } : {}) });
      signIn(r.token, r.user);
      navigate('/', { replace: true });
    } catch (err) {
      setError(explain(err, 'accept the invite'));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="auth">
      <AuthBrand />
      <div className="form">
        {info.loading && <Loading what="Checking the invite" />}
        {info.error !== undefined && !info.loading && (
          <div className="panel">
            <h2>This invite does not work</h2>
            <ErrorNote error={info.error} />
            <p className="fine">Invite links can be used once and may expire. Ask whoever sent it for a new one.</p>
            <Link className="btn" to="/sign-in">Go to sign in</Link>
          </div>
        )}
        {info.data && (
          <form className="panel" onSubmit={accept} noValidate>
            <h2>Join {joinNames(info.data)}</h2>
            {info.data.also && info.data.also.length > 0 ? (
              <>
                <p>
                  You are invited{info.data.email ? <> as <b>{info.data.email}</b></> : null} to {info.data.also.length + 1} organizations. {until(info.data.expires_at)}.
                </p>
                <ul className="mini">
                  {[{ org: info.data.org, role: info.data.role }, ...info.data.also].map((a) => (
                    <li key={a.org.id}><b>{a.org.name}</b> as {ROLE_LABEL[a.role]}</li>
                  ))}
                </ul>
              </>
            ) : (
              <p>
                You are invited as <b>{ROLE_LABEL[info.data.role]}</b>
                {info.data.email ? <> for <b>{info.data.email}</b></> : null}. {until(info.data.expires_at)}.
              </p>
            )}
            {me ? (
              <>
                <div className="notice">You are signed in as <b>{me.name}</b> ({me.email}). Accepting adds {joinNames(info.data)} to this account.</div>
                {error && <div className="notice bad" role="alert">{error}</div>}
                <button className="btn pri" type="submit" disabled={busy}>Join {joinNames(info.data)}</button>
              </>
            ) : (
              <>
                <label className="field">
                  <span>Your name</span>
                  <input className="input" value={name} onChange={(e) => setName(e.target.value)} autoComplete="name" autoFocus />
                  <small>Shown next to everything you publish and claim.</small>
                </label>
                {!info.data.email && (
                  <label className="field">
                    <span>Email</span>
                    <input className="input" type="email" autoComplete="email" value={email} onChange={(e) => setEmail(e.target.value)} />
                    <small>You sign in with this. It is not shared outside {info.data.org.name}.</small>
                  </label>
                )}
                <label className="field">
                  <span>Password</span>
                  <input className="input" type="password" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} />
                  <small>At least 10 characters.</small>
                </label>
                <label className="field">
                  <span>Password again</span>
                  <input className="input" type="password" autoComplete="new-password" value={again} onChange={(e) => setAgain(e.target.value)} />
                </label>
                {error && <div className="notice bad" role="alert">{error}</div>}
                <button className="btn pri" type="submit" disabled={busy}>{busy ? 'Setting up your account' : 'Create account and join'}</button>
                <p className="fine">Already have an account on this server? <Link className="link" to={`/sign-in?next=${encodeURIComponent(`/invite/${token}`)}`}>Sign in first</Link>, then come back to this link.</p>
              </>
            )}
          </form>
        )}
      </div>
    </div>
  );
}
