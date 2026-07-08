import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import path from "path";

export default defineConfig({
  plugins: [solid()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],

  // Pre-bundles these dependencies to prevent Vite from re-optimizing and
  // triggering full page reloads during HMR when they are dynamically imported
  // or cause circular dependency warnings in dev mode.
  optimizeDeps: {
    include: [
      "solid-js",
      "solid-js/store",
      "solid-js/web",
      "@tauri-apps/api",
      "@tauri-apps/api/core",
      "@tauri-apps/api/event",
      "@tauri-apps/api/window",
      "@tanstack/solid-virtual",
      "date-fns",
      "lucide-solid",
      "@kobalte/core",
    ],
  },
});
