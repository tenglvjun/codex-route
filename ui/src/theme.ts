import type { ThemePreference } from "./api";

export type ResolvedTheme = "light" | "dark";

export function resolveTheme(preference: ThemePreference, systemDark: boolean): ResolvedTheme {
  return preference === "system" ? (systemDark ? "dark" : "light") : preference;
}

export function applyTheme(preference: ThemePreference): () => void {
  const root = document.documentElement;
  const media = typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : { matches: false, addEventListener: undefined, removeEventListener: undefined };
  const apply = () => {
    root.dataset.theme = preference;
    root.style.colorScheme = resolveTheme(preference, media.matches);
  };
  apply();
  if (preference !== "system") return () => undefined;
  const listener = () => apply();
  media.addEventListener?.("change", listener);
  return () => media.removeEventListener?.("change", listener);
}
