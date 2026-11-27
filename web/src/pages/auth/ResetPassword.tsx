import { useState, type FormEvent } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { api, explain } from '../../api/client';
import { AuthBrand } from './SignIn';

/** "Forgot password?": ask for a link by email. */
export function ForgotPassword() {
  const [email, setEmail] = useState('');
  const [sent, setSent] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!email.trim()) return setError('Enter the email you sign in with.');
    setBusy(true);
    setError('');
    try {
      await api.forgotPassword(email.trim());
      setSent(true);
    } catch (err) {
      setError(explain(err, 'send the email'));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="auth">
      <AuthBrand />
      <div className="form">
        <form className="panel" onSubmit={submit} noValidate>
          <h2>Set a new password</h2>
          {sent ? (
            <>
              <div className="notice good" role="status">If {email.trim()} has an account here, an email with a link is on its way. The link works for an hour.</div>
              <p className="fine">Nothing arrived? Check spam, or ask an admin to set a new password for you.</p>
            </>
          ) : (
            <>
              <p>We will email you a link to choose a new password.</p>
              <label className="field">
                <span>Email</span>
                <input className="input" type="email" autoComplete="username" value={email} onChange={(e) => setEmail(e.target.value)} autoFocus />
              </label>
              {error && <div className="notice bad" role="alert">{error}</div>}
              <button className="btn pri" type="submit" disabled={busy}>{busy ? 'Sending' : 'Email me a link'}</button>
            </>
          )}
          <p className="fine"><Link to="/sign-in">Back to sign in</Link></p>
        </form>
      </div>
    </div>
  );
}

/** The page the emailed link opens. */
export function ResetPassword() {
  const [params] = useSearchParams();
  const token = params.get('token') || '';
  const [password, setPassword] = useState('');
  const [again, setAgain] = useState('');
  const [done, setDone] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (password.length < 10) return setError('Use a password of at least 10 characters.');
    if (password !== again) return setError('The two passwords are different.');
    setBusy(true);
    setError('');
    try {
      const r = await api.resetPassword(token, password);
      setDone(r.email);
    } catch (err) {
      setError(explain(err, 'set the new password'));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="auth">
      <AuthBrand />
      <div className="form">
        <form className="panel" onSubmit={submit} noValidate>
          <h2>Choose a new password</h2>
          {done ? (
            <>
              <div className="notice good" role="status">Your password is changed. You were signed out everywhere else.</div>
              <Link className="btn pri" to={`/sign-in`}>Sign in as {done}</Link>
            </>
          ) : !token ? (
            <div className="notice bad">This page needs the link from the email. <Link to="/forgot-password">Ask for a new one.</Link></div>
          ) : (
            <>
              <label className="field">
                <span>New password</span>
                <input className="input" type="password" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} autoFocus />
                <small>At least 10 characters.</small>
              </label>
              <label className="field">
                <span>The same again</span>
                <input className="input" type="password" autoComplete="new-password" value={again} onChange={(e) => setAgain(e.target.value)} />
              </label>
              {error && <div className="notice bad" role="alert">{error} {/expired|used/.test(error) && <Link to="/forgot-password">Ask for a new link.</Link>}</div>}
              <button className="btn pri" type="submit" disabled={busy}>{busy ? 'Saving' : 'Set password'}</button>
            </>
          )}
        </form>
      </div>
    </div>
  );
}
