// Mock mode: opened with ?mock=1 (remembered for the tab in sessionStorage) or built with VITE_MOCK=1.
// The mock lives in src/api/mock/ and is only ever loaded with a dynamic import, so it stays out of
// the main bundle. Set VITE_NO_MOCK=1 at build time to drop the ?mock=1 switch entirely.

const KEY = 'mp.mock';

function readFlag(): boolean {
  if (import.meta.env.VITE_NO_MOCK === '1') return false;
  if (import.meta.env.VITE_MOCK === '1') return true;
  try {
    const q = new URLSearchParams(window.location.search).get('mock');
    if (q === '1') sessionStorage.setItem(KEY, '1');
    if (q === '0') sessionStorage.removeItem(KEY);
    return sessionStorage.getItem(KEY) === '1';
  } catch {
    return false;
  }
}

export const MOCK = readFlag();

type MockModule = typeof import('./mock');
let loading: Promise<MockModule> | null = null;

export function loadMock(): Promise<MockModule> {
  if (!loading) loading = import('./mock');
  return loading;
}
