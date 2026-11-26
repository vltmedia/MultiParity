// Themes for the web UI. The list and the engine are shared with the desktop app
// (desktop/ui/themes.json and theme-engine.js), so both apps look the same.
import '../../../desktop/ui/theme-engine.js';
import themesJson from '../../../desktop/ui/themes.json';

export type Theme = {
  id: string;
  name: string;
  grp: string;
  desc: string;
  font: string;
  display: string;
  c: string[];
  r: number;
  br?: number;
  bw: string;
  pw?: string;
  panelLine?: string;
  sh?: string;
  bgimg?: string;
  bgsize?: string;
};

type Engine = { apply(t: Theme): void; saved(): string; save(id: string): void };
const engine = (globalThis as unknown as { MPTheme: Engine }).MPTheme;

export const THEMES = themesJson as unknown as Theme[];

export function currentThemeId(): string {
  return engine.saved();
}

export function applyTheme(id: string, remember = true) {
  const t = THEMES.find((x) => x.id === id) || THEMES[0];
  engine.apply(t);
  if (remember) engine.save(t.id);
  return t;
}

/** Apply the remembered theme; called before the first render. */
export function applySavedTheme() {
  applyTheme(engine.saved(), false);
}
