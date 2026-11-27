import { useEffect, useState, type FormEvent } from 'react';
import { Link, Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import { api, ApiError, explain } from '../../api/client';
import { MOCK } from '../../api/mode';
import { useSession } from '../../lib/session';

export function AuthBrand() {
  return (
    <div className="brand">
      <div className="logo">MultiParity</div>
      <div>
        <h1>Share big files without stepping on each other.</h1>
        <p>Claim what you are working on, publish when it is ready, and get the latest from everyone else.</p>
        <div className="thumbs" aria-hidden="true">
          {[18, 32, 265, 330].map((h) => (
            <span key={h} className="thumb" style={{ ['--h' as string]: h }} />
          ))}
        </div>
      </div>
      <p className="fine" style={{ color: 'var(--sidemute)' }}>Server: {location.host}</p>
    </div>
  );
}

export default function SignIn() {
  const { me, signIn } = useSession();
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const [email, setEmail] = useState(MOCK ? 'justin@cinderworks.example' : '');
  const [password, setPassword] = useState(MOCK ? 'anything' : '');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const next = params.get('next') || '/';
  const [canEmail, setCanEmail] = useState(false);
  useEffect(() => { api.serverInfo().then((i) => setCanEmail(i.email)).catch(() => {}); }, []);

  if (me) return <Navigate to={next} replace />;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!email.trim() || !password) {
      setError('Enter your email and password.');
      return;
    }
    setBusy(true);
    setError('');
    try {
      const r = await api.login(email.trim(), password);
      signIn(r.token, r.user);
      navigate(next.startsWith('/') ? next : '/', { replace: true });
    } catch (err) {
      if (err instanceof ApiError && err.code === 'unauthorized')
        setError('That email and password do not match an account on this server. Check them, or ask an admin to reset your password.');
      else if (err instanceof ApiError && err.code === 'suspended')
        setError('This account is suspended, so it cannot sign in. Ask an organization admin to reactivate it.');
      else setError(explain(err, 'sign you in'));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="auth">
      <AuthBrand />
      <div className="form">
        <form className="panel" onSubmit={submit} noValidate>
          <h2>Sign in</h2>
          <p>Use the account your studio set up for you on {location.host}.</p>
          <label className="field">
            <span>Email</span>
            <input className="input" type="email" autoComplete="username" value={email} onChange={(e) => setEmail(e.target.value)} autoFocus />
          </label>
          <label className="field">
            <span>Password</span>
            <input className="input" type="password" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} />
          </label>
          {error && <div className="notice bad" role="alert">{error}</div>}
          <button className="btn pri" type="submit" disabled={busy}>{busy ? 'Signing in' : 'Sign in'}</button>
          <p className="fine">
            No account yet? Ask an admin for an invite link.{' '}
            {canEmail ? <Link to="/forgot-password">Forgot your password?</Link> : 'Forgot your password? An admin can set a new one for you.'}
          </p>
          {MOCK && <p className="fine">Mock mode: any password works. Try pell@cinderworks.example to see a suspended account.</p>}
        </form>
      </div>
    </div>
  );
}
