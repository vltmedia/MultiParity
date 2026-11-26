import { Component, type ReactNode } from 'react';

type Props = { children: ReactNode; resetKey?: string };
type State = { error: Error | null };

/**
 * Keeps one broken view from blanking the whole app. Shows what happened and a way back;
 * resets when the route changes (`resetKey`).
 */
export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error) {
    console.error('MultiParity view error', error);
  }

  componentDidUpdate(prev: Props) {
    if (prev.resetKey !== this.props.resetKey && this.state.error) this.setState({ error: null });
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div style={{ padding: '48px 36px', maxWidth: 640 }}>
        <h1 style={{ fontSize: 32, fontWeight: 700, letterSpacing: '-0.03em', marginBottom: 10 }}>This page hit a problem</h1>
        <p style={{ color: 'var(--mute, #6d6378)', marginBottom: 18 }}>
          Nothing was lost: your files and the server are unaffected. Reload to try again; if it keeps happening, the
          details below help whoever looks after this server.
        </p>
        <pre style={{ background: '#fff', border: '1px solid #ddd7e4', padding: 12, whiteSpace: 'pre-wrap', fontSize: 13, marginBottom: 18 }}>
          {this.state.error.message}
        </pre>
        <button
          type="button"
          onClick={() => window.location.reload()}
          style={{ padding: '10px 16px', fontWeight: 600, border: '1.5px solid #ff6a13', background: '#ff6a13', color: '#1b0b00', cursor: 'pointer' }}
        >
          Reload
        </button>
      </div>
    );
  }
}
