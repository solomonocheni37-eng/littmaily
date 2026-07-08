import { createSignal, createEffect, onMount } from "solid-js";

export type Theme = "light" | "dark" | "system";

const THEME_KEY = "littmaily-theme";

/**
 * Manages application theme state, persisting the user's choice to localStorage.
 * Falls back to the OS-level `prefers-color-scheme` media query when set to "system".
 *
 * Applies `light` or `dark` classes directly to the document root to integrate with
 * Tailwind's `darkMode: "class"` configuration.
 */
export function createThemeManager() {
  const [theme, setTheme] = createSignal<Theme>("system");

  onMount(() => {
    const saved = localStorage.getItem(THEME_KEY) as Theme;
    if (saved) setTheme(saved);
  });

  createEffect(() => {
    const currentTheme = theme();
    localStorage.setItem(THEME_KEY, currentTheme);

    const root = window.document.documentElement;
    // Explicitly remove both classes first to prevent state leakage if the system preference changes.
    root.classList.remove("light", "dark");

    if (currentTheme === "system") {
      const systemTheme = window.matchMedia("(prefers-color-scheme: dark)")
        .matches
        ? "dark"
        : "light";
      root.classList.add(systemTheme);
    } else {
      root.classList.add(currentTheme);
    }
  });

  return { theme, setTheme };
}
