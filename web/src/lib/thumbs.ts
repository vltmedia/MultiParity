import { useEffect, useState } from 'react';
import { fetchAuthed } from '../api/client';

// Thumbnails are addressed by content hash, so one fetch per hash and size is enough for the
// whole session. Each resolves to an object URL, or null when there is no picture.
const cache = new Map<string, Promise<string | null>>();
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function load(projectId: string, hash: string, size: 'small' | 'large'): Promise<string | null> {
  // 202 while the server draws it: ask again for up to a minute.
  for (let i = 0; i < 20; i++) {
    let res: Response;
    try {
      res = await fetchAuthed(`/api/projects/${encodeURIComponent(projectId)}/thumbs/${hash}${size === 'large' ? '?size=large' : ''}`);
    } catch {
      return null;
    }
    if (res.status === 200) return URL.createObjectURL(await res.blob());
    if (res.status !== 202) return null;
    await wait(i < 5 ? 1500 : 4000);
  }
  return null;
}

export function thumbUrl(projectId: string, hash: string, size: 'small' | 'large' = 'small'): Promise<string | null> {
  const key = `${projectId}/${hash}/${size}`;
  let p = cache.get(key);
  if (!p) {
    p = load(projectId, hash, size);
    cache.set(key, p);
    // A miss is not remembered forever: the file may be drawn later.
    p.then((u) => { if (!u) setTimeout(() => cache.delete(key), 60_000); });
  }
  return p;
}

/** The thumbnail's object URL once it is known; null while loading or when there is none. */
export function useThumb(projectId: string | undefined, hash: string | undefined | null, size: 'small' | 'large' = 'small'): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    setUrl(null);
    if (!projectId || !hash) return;
    let live = true;
    thumbUrl(projectId, hash, size).then((u) => live && setUrl(u));
    return () => { live = false; };
  }, [projectId, hash, size]);
  return url;
}

/** Extensions the server can draw. */
export const DRAWABLE = /\.(png|jpe?g|gif|webp|bmp|tga|tiff?|exr|hdr|blend)$/i;
