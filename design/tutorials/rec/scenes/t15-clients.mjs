import { WEB, webSignIn } from './lib.mjs';
import { rowButton, scopeCinder, webApi } from './web.mjs';

/** As before the take: no Saltmarsh client, Frostline back in Northwind Games > Emberfall. */
async function tidy(r, org) {
  const st = await webApi(r, 'GET', `/admin/orgs/${org}/structure`);
  const ember = st.productions.find((p) => p.name === 'Emberfall' && p.client_id);
  const projects = await webApi(r, 'GET', `/admin/projects?org=${org}`);
  const frost = projects.find((p) => p.name === 'Frostline');
  if (ember && frost) await webApi(r, 'PATCH', `/admin/projects/${frost.id}`, { production: ember.id });
  for (const p of st.productions.filter((p) => p.name === 'Tidewrack')) await webApi(r, 'DELETE', `/admin/productions/${p.id}`);
  for (const c of st.clients.filter((c) => c.name === 'Saltmarsh Studios')) await webApi(r, 'DELETE', `/admin/clients/${c.id}`);
}

export default async function scene(r) {
  await webSignIn(r);
  const org = await scopeCinder(r);
  await tidy(r, org);
  await r.open(`${WEB}/admin/projects`, 2200);
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.callout('sweep', { css: 'tbody' }, { dur: 2.6, pad: 2 });
  await r.hover({ css: 'tr.grp.client .grp-name' }, { ms: 1000 });

  await r.beat('client');
  await r.callout('ring', { text: 'New client', sel: 'button' }, { dur: 1.6 });
  await r.click({ text: 'New client', sel: 'button' }, { after: 900 });
  await r.type('Saltmarsh Studios', { cps: 14 });
  await r.click({ text: 'Make client', sel: 'button' }, { after: 1400 });

  await r.beat('production');
  const np = await rowButton(r, 'Saltmarsh Studios', 'New production');
  await r.click({ text: 'New production', sel: 'button', index: np }, { after: 900 });
  await r.type('Tidewrack', { cps: 12 });
  await r.callout('tag', { css: '[role=dialog] select, dialog select' }, { label: 'Under Saltmarsh Studios', dur: 2.4 });
  await r.click({ text: 'Make production', sel: 'button' }, { after: 1400 });

  await r.beat('file');
  const fu = await rowButton(r, 'Frostline', 'File under');
  await r.callout('ring', { text: 'File under…', sel: 'button', index: fu }, { dur: 1.6 });
  await r.click({ text: 'File under…', sel: 'button', index: fu }, { after: 900 });
  await r.choose({ css: '[role=dialog] select, dialog select' }, 'Saltmarsh Studios › Tidewrack');
  await r.at(0.6);
  await r.click({ text: 'File it here', sel: 'button' }, { after: 1500 });

  await r.beat('main');
  const mm = await rowButton(r, 'Frostline', 'Make main');
  await r.callout('ring', { text: 'Make main', sel: 'button', index: mm }, { dur: 1.6 });
  await r.click({ text: 'Make main', sel: 'button', index: mm }, { after: 1400 });
  await r.callout('tag', { text: 'Main', sel: '.badge' }, { label: 'Listed first', dur: 2.4 });

  await r.beat('see');
  await r.open(`${WEB}/`, 2000);
  await r.callout('sweep', { css: '.psub', index: 0 }, { dur: 2.4, pad: 4 });
  await r.hover({ css: '.psub', index: 1 }, { after: 600 });
  await r.end(1.8);
  await tidy(r, org);
}
scene.options = { view: 1440 };
