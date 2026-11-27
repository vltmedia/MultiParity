import { WEB, webSignIn } from './lib.mjs';
import { rowButton, scopeCinder, webApi } from './web.mjs';

const tidy = async (r, org) => {
  for (const p of (await webApi(r, 'GET', `/admin/orgs/${org}/policies`)).filter((p) => p.name === 'Audio team')) await webApi(r, 'DELETE', `/admin/policies/${p.id}`);
};

export default async function scene(r) {
  await webSignIn(r);
  const org = await scopeCinder(r);
  await tidy(r, org);
  await r.open(`${WEB}/admin/policies`, 2200);
  await r.park(900, 420, 10);
  await r.record();

  await r.beat('hello');
  await r.callout('spot', { css: 'ul.policies > li', index: 0 }, { dur: 2.8, pad: 4 });
  await r.hover({ css: 'ul.policies h3', index: 0 }, { ms: 1000 });

  await r.beat('start');
  await r.callout('tag', { css: 'ul.policies > li .rules', index: 0 }, { label: 'Everyone, to start', dur: 2.6 });
  await r.hover({ css: 'ul.policies > li .rules li', index: 0 }, { after: 600 });
  await r.at(0.62);
  await r.click({ text: 'New policy', sel: 'button' }, { after: 900 });
  await r.type('Audio team', { cps: 12 });

  await r.beat('rules');
  await r.click({ text: 'Add rule', sel: 'button' }, { after: 500 });
  await r.choose({ css: '.rule-edit li select[aria-label=Covers]', index: 0 }, 'Northwind Games › Emberfall');
  await r.choose({ css: '.rule-edit li select.lvl', index: 0 }, 'Can look');
  await r.at(0.45);
  await r.click({ text: 'Add rule', sel: 'button' }, { after: 500 });
  await r.choose({ css: '.rule-edit li select[aria-label=Covers]', index: 1 }, 'Northwind Games › Emberfall › Audio');
  await r.callout('sweep', { css: '.rule-edit' }, { dur: 2.6, pad: 4 });
  await r.click({ text: 'Make policy', sel: 'button' }, { after: 1400 });

  await r.beat('people');
  const pp = await r.eval(`[...document.querySelectorAll('ul.policies > li')].findIndex((li) => li.textContent.includes('Audio team'))`);
  await r.click({ text: 'People…', sel: 'button', index: pp - 1 }, { after: 900 });
  await r.click({ text: 'Mara', sel: '.people-pick label span' }, { after: 500 });
  await r.click({ text: 'Save', sel: 'button' }, { after: 1500 });

  await r.beat('who');
  await r.open(`${WEB}/admin/projects`, 2000);
  const w = await rowButton(r, 'Audio', 'Who can see');
  await r.callout('ring', { text: 'Who can see…', sel: 'button', index: w }, { dur: 1.6 });
  await r.click({ text: 'Who can see…', sel: 'button', index: w }, { after: 1600 });
  await r.callout('tag', { text: 'Audio team policy', sel: 'td' }, { label: 'Why', dur: 2.6 });
  await r.hover({ text: 'Audio team policy', sel: 'td' }, { after: 900 });
  await r.end(1.8);
  await tidy(r, org);
}
scene.options = { view: 1440 };
