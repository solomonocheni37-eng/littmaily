import type { Component } from "solid-js";
import { Sun, Moon, Monitor } from "lucide-solid";
import { createThemeManager, type Theme } from "@/core/store/ThemeStore";

const ThemeToggle: Component = () => {
  const { theme, setTheme } = createThemeManager();

  const nextTheme = () => {
    const current = theme();
    if (current === "system") setTheme("light");
    else if (current === "light") setTheme("dark");
    else setTheme("system");
  };

  const getIcon = (t: Theme) => {
    if (t === "light") return <Sun size={16} />;
    if (t === "dark") return <Moon size={16} />;
    return <Monitor size={16} />;
  };

  return (
    <button
      onClick={nextTheme}
      class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm text-surface-600 dark:text-surface-400 hover:bg-surface-100 dark:hover:bg-surface-800/50 transition-all"
      title={`Current: ${theme()}`}
    >
      {getIcon(theme())}
      <span class="capitalize">{theme()}</span>
    </button>
  );
};

export default ThemeToggle;
