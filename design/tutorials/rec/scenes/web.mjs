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

/** Call the server's API as the signed-in person (for setting a scene up, off camera). */
export async function webApi(r, method, path, body) {
  return r.eval(`fetch('/api${path}', { method: ${JSON.stringify(method)}, headers: { 'content-type': 'application/json', authorization: 'Bearer ' + localStorage.getItem('mp.token') }${body ? `, body: ${JSON.stringify(JSON.stringify(body))}` : ''} }).then(async (x) => { const t = await x.text(); try { return JSON.parse(t); } catch { return t; } })`);
}

/** The index, among all buttons whose text starts with `label`, of the one on the table row
 *  that mentions `rowText` (for r.click({ text: label, sel: 'button', index })). */
export async function rowButton(r, rowText, label) {
  return r.eval(`(() => {
    const all = [...document.querySelectorAll('button')].filter((b) => b.textContent.trim().startsWith(${JSON.stringify(label)}) && b.getBoundingClientRect().width > 2);
    const i = all.findIndex((b) => { const tr = b.closest('tr'); return tr && tr.textContent.includes(${JSON.stringify(rowText)}); });
    if (i < 0) throw new Error('No ' + ${JSON.stringify(label)} + ' on the ' + ${JSON.stringify(rowText)} + ' row');
    return i;
  })()`);
}

/** Admin pages scoped to Cinder Works (the demo admin looks after two organizations). */
export async function scopeCinder(r) {
  const orgs = await webApi(r, 'GET', '/orgs');
  const cinder = orgs.find((o) => o.name === 'Cinder Works');
  await r.eval(`sessionStorage.setItem('mp.admin.scope', ${JSON.stringify(cinder.id)})`);
  return cinder.id;
}
