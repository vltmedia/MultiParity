// Helpers for web-app scenes: signed in as Alice, on a project's page.
import { WEB, webSignIn } from './lib.mjs';

export async function projectId(r, name = 'Emberfall') {
  return r.eval(`fetch('/api/projects', { headers: { authorization: 'Bearer ' + localStorage.getItem('mp.token') } }).then((x) => x.json())
    .then((d) => (Array.isArray(d) ? d : d.projects).find((p) => p.name === ${JSON.stringify(name)}).id)`);
}

/** Signed in, on /p/<project>/<view> (or any path starting with /). */
export async function webReady(r, view = 'today', project = 'Emberfall') {
  await webSignIn(r);
  const where = view.startsWith('/') ? view : `/p/${await projectId(r, project)}/${view}`;
  await r.open(WEB + where, 2200);
  return where;
}
export { WEB };
