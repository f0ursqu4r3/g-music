export const themes = ["midnight", "plum", "ember"] as const;

export type ThemeName = (typeof themes)[number];

export function readTheme(value: string | null): ThemeName {
  return themes.includes(value as ThemeName)
    ? (value as ThemeName)
    : "midnight";
}
