import { useCallback, useEffect, useRef, useState } from 'react';

export type Async<T> = {
  data: T | undefined;
  error: unknown;
  loading: boolean;
  reload: () => void;
  setData: (fn: (d: T | undefined) => T | undefined) => void;
};

/**
 * Run an async loader whenever deps change. Keeps the previous data while reloading so views do
 * not flash empty on live updates.
 */
export function useAsync<T>(fn: () => Promise<T>, deps: unknown[], enabled = true): Async<T> {
  const [data, setDataState] = useState<T | undefined>(undefined);
  const [error, setError] = useState<unknown>(undefined);
  const [loading, setLoading] = useState<boolean>(enabled);
  const [tick, setTick] = useState(0);
  const fnRef = useRef(fn);
  fnRef.current = fn;

  useEffect(() => {
    if (!enabled) {
      setLoading(false);
      return;
    }
    let live = true;
    setLoading(true);
    fnRef.current().then(
      (d) => {
        if (!live) return;
        setDataState(d);
        setError(undefined);
        setLoading(false);
      },
      (e) => {
        if (!live) return;
        setError(e);
        setLoading(false);
      },
    );
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, tick, enabled]);

  const reload = useCallback(() => setTick((t) => t + 1), []);
  const setData = useCallback((f: (d: T | undefined) => T | undefined) => setDataState((d) => f(d)), []);
  return { data, error, loading, reload, setData };
}
