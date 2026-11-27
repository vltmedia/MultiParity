import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useId,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { explain } from '../api/client';
import { hue, initial, personColors } from '../lib/format';
import { DRAWABLE, useThumb } from '../lib/thumbs';

// ---------- small pieces ----------

/** Placeholder thumbnail coloured from a seed, until previews exist. Size comes from CSS unless given. */
export function Thumb({ seed, size, className = '' }: { seed: string; size?: number; className?: string }) {
  return (
    <span
      className={`thumb ${className}`}
      style={{ ['--h' as string]: hue(seed), ...(size ? { width: size, height: size } : {}) }}
      aria-hidden="true"
    />
  );
}

/** A published file's real thumbnail when the server can draw it, the coloured placeholder otherwise. */
export function FileThumb({ projectId, hash, path, size }: { projectId: string; hash?: string | null; path: string; size?: number }) {
  const url = useThumb(projectId, DRAWABLE.test(path) ? hash : null);
  if (!url) return <Thumb seed={path} size={size} />;
  return (
    <span className="thumb img" style={size ? { width: size, height: size } : undefined} aria-hidden="true">
      <img src={url} alt="" />
    </span>
  );
}

export function Who({ id, name, size }: { id: string; name: string; size?: number }) {
  const c = personColors(id);
  return (
    <span className="who" style={{ background: c.bg, color: c.fg, width: size, height: size }} title={name} aria-hidden="true">
      {initial(name)}
    </span>
  );
}

export function Loading({ what = 'Loading' }: { what?: string }) {
  return <p className="loading" role="status">{what}</p>;
}

export function ErrorNote({ error, doing, retry }: { error: unknown; doing?: string; retry?: () => void }) {
  return (
    <div className="notice bad" role="alert">
      <p>{explain(error, doing)}</p>
      {retry && (
        <div className="row">
          <button className="btn sm" onClick={retry}>Try again</button>
        </div>
      )}
    </div>
  );
}

export function Empty({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="empty">
      <b>{title}</b>
      {children}
    </div>
  );
}

export function Toggle({ on, onChange, label, disabled }: { on: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button type="button" role="switch" className="toggle" aria-checked={on} aria-label={label} disabled={disabled} onClick={() => onChange(!on)} />
  );
}

export function TriCheck({ state, onChange, label }: { state: 'all' | 'some' | 'none'; onChange: (next: boolean) => void; label: string }) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = state === 'some';
  }, [state]);
  return <input ref={ref} type="checkbox" aria-label={label} checked={state === 'all'} onChange={() => onChange(state !== 'all')} />;
}

export function Parity({ total, done, now = 0 }: { total: number; done: number; now?: number }) {
  return (
    <div className="parity" aria-hidden="true">
      {Array.from({ length: total }, (_, i) => (
        <s key={i} className={i < done ? 'd' : i < done + now ? 'n' : ''} />
      ))}
    </div>
  );
}

// ---------- menus ----------

export function Menu({
  label,
  button,
  children,
  align = 'left',
  className = 'btn sm',
}: {
  label: string;
  button: ReactNode;
  children: (close: () => void) => ReactNode;
  align?: 'left' | 'right';
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  const id = useId();
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (wrap.current && !wrap.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    const first = wrap.current?.querySelector<HTMLElement>('.menu button, .menu a, .menu input');
    first?.focus();
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);
  return (
    <div className="menu-wrap" ref={wrap}>
      <button type="button" className={className} aria-haspopup="menu" aria-expanded={open} aria-controls={id} aria-label={label} onClick={() => setOpen((o) => !o)}>
        {button}
      </button>
      {open && (
        <div className="menu" role="menu" id={id} style={align === 'right' ? { right: 0, top: 'calc(100% + 4px)' } : { left: 0, top: 'calc(100% + 4px)' }}>
          {children(() => setOpen(false))}
        </div>
      )}
    </div>
  );
}

// ---------- dialogs ----------

export function Dialog({ title, children, onClose, wide }: { title: string; children: ReactNode; onClose: () => void; wide?: boolean }) {
  const box = useRef<HTMLDivElement>(null);
  const id = useId();
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    const el = box.current?.querySelector<HTMLElement>('[data-autofocus], input, textarea, select, button.pri, button');
    el?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
      if (e.key === 'Tab' && box.current) {
        const f = [...box.current.querySelectorAll<HTMLElement>('button, input, select, textarea, a[href]')].filter((x) => !x.hasAttribute('disabled'));
        if (!f.length) return;
        if (e.shiftKey && document.activeElement === f[0]) {
          e.preventDefault();
          f[f.length - 1].focus();
        } else if (!e.shiftKey && document.activeElement === f[f.length - 1]) {
          e.preventDefault();
          f[0].focus();
        }
      }
    };
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
      prev?.focus?.();
    };
  }, [onClose]);
  return (
    <div className="veil" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`dialog ${wide ? 'wide' : ''}`} role="dialog" aria-modal="true" aria-labelledby={id} ref={box}>
        <h2 id={id}>{title}</h2>
        {children}
      </div>
    </div>
  );
}

export type ConfirmOptions = {
  title: string;
  /** Exactly what will happen. */
  what: ReactNode;
  detail?: ReactNode;
  confirm: string;
  cancel?: string;
  danger?: boolean;
  /** Ask the person to type this before the button works. */
  typeToConfirm?: string;
};

type UiValue = {
  toast: (text: string, tone?: 'ok' | 'bad' | 'hold') => void;
  fail: (err: unknown, doing?: string) => void;
  confirm: (o: ConfirmOptions) => Promise<boolean>;
};

const UiCtx = createContext<UiValue | null>(null);

type Toast = { id: number; text: string; tone: 'ok' | 'bad' | 'hold' };

export function UiProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [ask, setAsk] = useState<(ConfirmOptions & { resolve: (v: boolean) => void }) | null>(null);
  const [typed, setTyped] = useState('');
  const n = useRef(0);

  const toast = useCallback((text: string, tone: Toast['tone'] = 'ok') => {
    n.current += 1;
    const id = n.current;
    setToasts((t) => [...t.slice(-3), { id, text, tone }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), tone === 'bad' ? 9000 : 5000);
  }, []);
  const fail = useCallback((err: unknown, doing?: string) => toast(explain(err, doing), 'bad'), [toast]);
  const confirm = useCallback(
    (o: ConfirmOptions) =>
      new Promise<boolean>((resolve) => {
        setTyped('');
        setAsk({ ...o, resolve });
      }),
    [],
  );
  const close = useCallback(
    (v: boolean) => {
      ask?.resolve(v);
      setAsk(null);
    },
    [ask],
  );
  const cancel = useCallback(() => close(false), [close]);

  return (
    <UiCtx.Provider value={{ toast, fail, confirm }}>
      {children}
      {ask && (
        <Dialog title={ask.title} onClose={cancel}>
          <div className={`what ${ask.danger ? '' : 'calm'}`}>{ask.what}</div>
          {ask.detail && <div className="fine">{ask.detail}</div>}
          {ask.typeToConfirm && (
            <label className="field">
              <span>Type {ask.typeToConfirm} to confirm</span>
              <input className="input" value={typed} onChange={(e) => setTyped(e.target.value)} autoComplete="off" spellCheck={false} data-autofocus />
            </label>
          )}
          <div className="actions">
            <button className="btn" onClick={cancel}>{ask.cancel || 'Cancel'}</button>
            <button
              className={ask.danger ? 'btn danger solid' : 'btn pri'}
              disabled={!!ask.typeToConfirm && typed.trim() !== ask.typeToConfirm}
              onClick={() => close(true)}
            >
              {ask.confirm}
            </button>
          </div>
        </Dialog>
      )}
      <div className="toasts" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast ${t.tone}`}>
            <p>{t.text}</p>
            <button onClick={() => setToasts((x) => x.filter((y) => y.id !== t.id))} aria-label="Dismiss">Close</button>
          </div>
        ))}
      </div>
    </UiCtx.Provider>
  );
}

export function useUi() {
  const v = useContext(UiCtx);
  if (!v) throw new Error('useUi outside UiProvider');
  return v;
}

/** Run an action with busy state and voiced errors. */
export function useAction() {
  const { fail } = useUi();
  const [busy, setBusy] = useState<string | null>(null);
  const run = useCallback(
    async <T,>(key: string, doing: string, fn: () => Promise<T>): Promise<T | undefined> => {
      setBusy(key);
      try {
        return await fn();
      } catch (e) {
        fail(e, doing);
        return undefined;
      } finally {
        setBusy(null);
      }
    },
    [fail],
  );
  return { busy, run };
}

export function CopyButton({ text, label = 'Copy link' }: { text: string; label?: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      className="btn sm"
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(text);
        } catch {
          const ta = document.createElement('textarea');
          ta.value = text;
          document.body.appendChild(ta);
          ta.select();
          document.execCommand('copy');
          ta.remove();
        }
        setDone(true);
        setTimeout(() => setDone(false), 1800);
      }}
    >
      {done ? 'Copied' : label}
    </button>
  );
}
