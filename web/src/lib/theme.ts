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

export type ThemeContext = { light?: string | null; dark?: string | null } | null;

type Engine = {
  apply(t: Theme): void;
  saved(): string;
  save(id: string): void;
  chosen(ctx?: ThemeContext): string;
  applyChosen(ctx?: ThemeContext): Theme | undefined;
  mode(): 'light' | 'dark';
  AUTO: string;
  DEFAULTS: { light: string; dark: string };
  list?: Theme[];
};
const engine = (globalThis as unknown as { MPTheme: Engine }).MPTheme;

export const THEMES = themesJson as unknown as Theme[];
engine.list = THEMES;

/** "auto" means: follow the organization or project, else the light or dark default. */
export const AUTO = engine.AUTO;
export const DEFAULT_THEMES = engine.DEFAULTS;
export const themeById = (id?: string | null) => THEMES.find((t) => t.id === id);

/** The person's own pick in this browser, or AUTO. */
export function currentThemeId(): string {
  return engine.saved();
}

/** The theme showing now. */
export function shownThemeId(): string {
  return engine.chosen();
}

/** Remember the person's pick (a theme id, or AUTO) and show the result. */
export function applyTheme(id: string) {
  engine.save(id);
  return engine.applyChosen();
}

// The organization's look applies everywhere; a project's on top while it is open.
let base: ThemeContext = null;
let project: ThemeContext = null;
const merged = (): ThemeContext => ({ light: project?.light || base?.light || null, dark: project?.dark || base?.dark || null });

export function setOrgTheme(ctx: ThemeContext) {
  base = ctx;
  engine.applyChosen(merged());
}

export function setProjectTheme(ctx: ThemeContext) {
  project = ctx;
  engine.applyChosen(merged());
}

/** Apply what should show; called before the first render. */
export function applySavedTheme() {
  engine.applyChosen(null);
}
