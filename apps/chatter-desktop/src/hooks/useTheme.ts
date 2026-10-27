import { useCallback, useEffect, useMemo, useState } from "react";

import { usePreference } from "./usePreference";
import { themePreference, type ThemePreference } from "../runtime/capabilities/preferences";
export type { ThemePreference } from "../runtime/capabilities/preferences";
export type ResolvedTheme = "light" | "dark";

function getSystemTheme(): ResolvedTheme {
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function resolveTheme(preference: ThemePreference): ResolvedTheme {
  return preference === "system" ? getSystemTheme() : preference;
}

function applyTheme(preference: ThemePreference): void {
  const el = document.documentElement;
  if (preference === "system") {
    el.removeAttribute("data-theme");
  } else {
    el.setAttribute("data-theme", preference);
  }
}

export function useTheme() {
  const [preference, setPreference] = usePreference(themePreference);
  const theme = preference.value;
  const [resolved, setResolved] = useState<ResolvedTheme>(() =>
    resolveTheme(theme),
  );

  const setTheme = useCallback((preference: ThemePreference) => {
    setPreference(preference);
    applyTheme(preference);
    setResolved(resolveTheme(preference));
  }, [setPreference]);

  // Apply on mount
  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  // Track system preference changes
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      if (theme === "system") {
        setResolved(getSystemTheme());
      }
    };
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  return useMemo(
    () => ({ theme, setTheme, resolved, preference }),
    [theme, setTheme, resolved, preference],
  );
}
