import { useState } from 'react';
import TopBar from '../components/TopBar';
import { useUi } from '../components/ui';
import { THEMES, applyTheme, currentThemeId, type Theme } from '../lib/theme';

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
  const list = THEMES.filter((t) => group === 'All' || t.grp === group);

  function choose(t: Theme) {
    applyTheme(t.id);
    setCurrent(t.id);
    toast(`Now using ${t.name}.`);
  }

  return (
    <>
      <TopBar />
      <div className="page">
        <header className="head">
          <div>
            <h1>Look</h1>
            <p>Choose how MultiParity looks in this browser. It changes right away and only for you.</p>
          </div>
        </header>
        <div className="body">
          <div className="seg sm" role="toolbar" aria-label="Filter themes" style={{ marginBottom: 16 }}>
            {GROUPS.map((g) => (
              <button key={g} type="button" aria-pressed={g === group} onClick={() => setGroup(g)}>{g}</button>
            ))}
          </div>
          <div className="looks">
            {list.map((t) => (
              <button key={t.id} type="button" className="look" aria-pressed={t.id === current} title={t.desc} onClick={() => choose(t)}>
                <Swatch t={t} />
                <span className="look-name">
                  <strong>{t.name}</strong>
                  <small>{t.id === current ? 'In use' : t.grp}</small>
                </span>
              </button>
            ))}
          </div>
        </div>
      </div>
    </>
  );
}
