import { THEMES } from '../lib/theme';

const GROUPS = [...new Set(THEMES.map((t) => t.grp))];

/** Two selects, a theme for light mode and one for dark mode; "" follows `inherit`. */
export default function ThemePicker({ light, dark, inherit, onChange, disabled }: {
  light: string;
  dark: string;
  /** What "" means, e.g. "Same as the organization". */
  inherit: { light: string; dark: string };
  onChange: (which: 'light' | 'dark', id: string) => void;
  disabled?: boolean;
}) {
  const select = (which: 'light' | 'dark', value: string) => (
    <label className="field">
      <span>{which === 'light' ? 'In light mode' : 'In dark mode'}</span>
      <select className="select" value={value} disabled={disabled} onChange={(e) => onChange(which, e.target.value)}>
        <option value="">{inherit[which]}</option>
        {GROUPS.map((g) => (
          <optgroup key={g} label={g}>
            {THEMES.filter((t) => t.grp === g).map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </optgroup>
        ))}
      </select>
    </label>
  );
  return (
    <div className="themepick">
      {select('light', light)}
      {select('dark', dark)}
    </div>
  );
}
