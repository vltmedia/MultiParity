import { useEffect, useReducer, useState } from 'react';
import TopBar from '../components/TopBar';
import { useUi } from '../components/ui';
import { AUTO, DEFAULT_THEMES, THEMES, applyTheme, currentThemeId, shownThemeId, themeById, type Theme } from '../lib/theme';

const GROUPS = ['All', ...new Set(THEMES.map((t) => t.grp))];
const solid = (c: string) => (c.includes('gradient') ? '#ffffff' : c);

/** A small picture of the theme in its own colours, corners and type. */
function Swatch({ t }: { t: Theme }) {
  const [side, sideInk, , paper, card, ink, , line, acc, accInk, hold, nw] = t.c;
  return (
    <span className="look-art" style={{ background: paper, backgroundImage: t.bgimg || 'none', backgroundSize: t.bgsize || 'auto' }}>
      <span className="look-side" style={{ background: side }}>
        <i style={{ background: solid(sideInk) }} />
        <i style={{ background: solid(sideInk), opacity: 0.5 }} />
        <i style={{ background: solid(sideInk), opacity: 0.5 }} />
      </span>
      <span className="look-panel" style={{ background: card, border: `${t.pw || t.bw} solid ${t.panelLine || ink}`, borderRadius: t.r, boxShadow: t.sh || 'none' }}>
        <span className="look-title" style={{ color: ink, fontFamily: t.display }}>Aa</span>
        <span className="look-row"><i style={{ background: nw }} /><b style={{ background: line }} /></span>
        <span className="look-row"><i style={{ background: hold }} /><b style={{ background: line }} /></span>
        <span className="look-btn" style={{ background: acc, color: accInk, borderRadius: t.br ?? t.r }}>Publish</span>
      </span>
    </span>
  );
}

export default function Look() {
  const { toast } = useUi();
  const [current, setCurrent] = useState(currentThemeId());
  const [group, setGroup] = useState('All');
  // Redraw when the computer switches between light and dark, so "Showing now" stays right.
  const [, redraw] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    const mq = matchMedia('(prefers-color-scheme: dark)');
    mq.addEventListener('change', redraw);
    return () => mq.removeEventListener('change', redraw);
  }, []);
  const list = THEMES.filter((t) => group === 'All' || t.grp === group);

  function choose(t: Theme) {
    applyTheme(t.id);
    setCurrent(t.id);
    toast(`Now using ${t.name}.`);
  }
  function chooseAuto() {
    applyTheme(AUTO);
    setCurrent(AUTO);
    toast(`Now automatic: ${themeById(shownThemeId())?.name ?? 'the default'} for now.`);
  }
  const light = themeById(DEFAULT_THEMES.light)!;
  const dark = themeById(DEFAULT_THEMES.dark)!;

  return (
    <>
      <TopBar />
      <div className="page">
        <header className="head">
          <div>
            <h1>Look</h1>
            <p>Choose how MultiParity looks in this browser. It changes right away and only for you. Automatic uses your organization's or project's look, or Ice Lab and Orbital HUD by light and dark mode.</p>
          </div>
        </header>
        <div className="body">
          <div className="seg sm" role="toolbar" aria-label="Filter themes" style={{ marginBottom: 16 }}>
            {GROUPS.map((g) => (
              <button key={g} type="button" aria-pressed={g === group} onClick={() => setGroup(g)}>{g}</button>
            ))}
          </div>
          <div className="looks">
            {group === 'All' && (
              <button type="button" className="look auto" aria-pressed={current === AUTO} onClick={chooseAuto}
                title="Your organization's or project's look when it has one; otherwise Ice Lab in light mode and Orbital HUD in dark mode, following this computer.">
                <span className="look-split">
                  <Swatch t={light} />
                  <Swatch t={dark} />
                </span>
                <span className="look-name">
                  <strong>Automatic</strong>
                  <small>{current === AUTO ? `In use: ${themeById(shownThemeId())?.name ?? ''}` : 'Follows light and dark mode'}</small>
                </span>
              </button>
            )}
            {list.map((t) => (
              <button key={t.id} type="button" className="look" aria-pressed={t.id === current} title={t.desc} onClick={() => choose(t)}>
                <Swatch t={t} />
                <span className="look-name">
                  <strong>{t.name}</strong>
                  <small>{t.id === current ? 'In use' : current === AUTO && t.id === shownThemeId() ? 'Showing now (automatic)' : t.grp}</small>
                </span>
              </button>
            ))}
          </div>
        </div>
      </div>
    </>
  );
}
