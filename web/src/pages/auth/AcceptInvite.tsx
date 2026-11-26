import { useState, type FormEvent } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { api, explain } from '../../api/client';
import { useSession } from '../../lib/session';
import { useAsync } from '../../lib/useAsync';
import { ROLE_LABEL, until } from '../../lib/format';
import { ErrorNote, Loading } from '../../components/ui';
import { AuthBrand } from './SignIn';

export default function AcceptInvite() {
  const { token = '' } = useParams();
  const { me, signIn } = useSession();
  const navigate = useNavigate();
  const info = useAsync(() => api.invite(token), [token]);
  const [name, setName] = useState('');
  const [password, setPassword] = useState('');
  const [again, setAgain] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  async function accept(e?: FormEvent) {
    e?.preventDefault();
    setError('');
    if (!me) {
      if (!name.trim()) return setError('Enter the name people will see next to your work.');
      if (password.length < 8) return setError('Choose a password with at least 8 characters.');
      if (password !== again) return setError('The two passwords are not the same. Type them again.');
    }
    setBusy(true);
    try {
      const r = await api.acceptInvite(token, me ? {} : { name: name.trim(), password });
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
            <h2>Join {info.data.org.name}</h2>
            <p>
              You are invited as <b>{ROLE_LABEL[info.data.role]}</b>
              {info.data.email ? <> for <b>{info.data.email}</b></> : null}. {until(info.data.expires_at)}.
            </p>
            {me ? (
              <>
                <div className="notice">You are signed in as <b>{me.name}</b> ({me.email}). Accepting adds {info.data.org.name} to this account.</div>
                {error && <div className="notice bad" role="alert">{error}</div>}
                <button className="btn pri" type="submit" disabled={busy}>Join {info.data.org.name}</button>
              </>
            ) : (
              <>
                <label className="field">
                  <span>Your name</span>
                  <input className="input" value={name} onChange={(e) => setName(e.target.value)} autoComplete="name" autoFocus />
                  <small>Shown next to everything you publish and claim.</small>
                </label>
                <label className="field">
                  <span>Password</span>
                  <input className="input" type="password" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} />
                  <small>At least 8 characters.</small>
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
