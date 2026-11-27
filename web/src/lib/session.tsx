import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';
import { api, ApiError, getToken, onUnauthorized, setToken } from '../api/client';
import type { Me } from '../api/types';
import { setOrgTheme } from './theme';

type SessionValue = {
  me: Me | null;
  loading: boolean;
  error: unknown;
  signIn: (token: string, me: Me) => void;
  signOut: () => Promise<void>;
  refresh: () => Promise<void>;
  /** Server admin, or owner/admin of at least one organization. */
  canAdmin: boolean;
};

const Ctx = createContext<SessionValue | null>(null);

export function SessionProvider({ children }: { children: ReactNode }) {
  const [me, setMe] = useState<Me | null>(null);
  useEffect(() => {
    const o = me?.orgs.map((x) => x.org).find((x) => x.theme_light || x.theme_dark);
    setOrgTheme(o ? { light: o.theme_light, dark: o.theme_dark } : null);
  }, [me]);
  const [loading, setLoading] = useState<boolean>(!!getToken());
  const [error, setError] = useState<unknown>(undefined);
  const navigate = useNavigate();

  useEffect(() => {
    onUnauthorized(() => {
      setMe(null);
      const here = window.location.pathname + window.location.search;
      if (!here.startsWith('/sign-in') && !here.startsWith('/invite/')) {
        navigate(`/sign-in?next=${encodeURIComponent(here)}`, { replace: true });
      }
    });
  }, [navigate]);

  const refresh = useCallback(async () => {
    if (!getToken()) {
      setMe(null);
      setLoading(false);
      return;
    }
    try {
      setMe(await api.me());
      setError(undefined);
    } catch (e) {
      if (!(e instanceof ApiError && e.status === 401)) setError(e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const signIn = useCallback((token: string, user: Me) => {
    setToken(token);
    setMe(user);
    setError(undefined);
  }, []);

  const signOut = useCallback(async () => {
    try {
      await api.logout();
    } catch {
      /* signing out locally is enough */
    }
    setToken(null);
    setMe(null);
    navigate('/sign-in', { replace: true });
  }, [navigate]);

  const canAdmin = !!me && (me.is_server_admin || me.orgs.some((o) => o.role === 'owner' || o.role === 'admin'));
  const value = useMemo(
    () => ({ me, loading, error, signIn, signOut, refresh, canAdmin }),
    [me, loading, error, signIn, signOut, refresh, canAdmin],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useSession() {
  const v = useContext(Ctx);
  if (!v) throw new Error('useSession outside SessionProvider');
  return v;
}

/** For pages that only render when signed in. */
export function useMe(): Me {
  const { me } = useSession();
  if (!me) throw new Error('useMe without a session');
  return me;
}
