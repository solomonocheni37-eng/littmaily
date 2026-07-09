import {
  createSignal,
  onMount,
  onCleanup,
  Show,
  createEffect,
  ErrorBoundary,
} from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { AppProvider } from "./core/store/AppStore";
import SyncListener from "./features/sync/SyncListener";
import AddAccountModal from "./features/accounts/components/AddAccountModal";
import ComposeModal from "./features/compose/components/ComposeModal";
import CommandPalette from "./features/search/components/CommandPalette";
import ContextMenu from "./features/email/components/ContextMenu";
import SettingsModal from "./features/settings/components/SettingsModal";
import { AppShell } from "./features/layout/components/AppShell";
import { useAppUpdater } from "./core/hooks/useAppUpdater";

function App() {
  const [isReady, setIsReady] = createSignal(false);

  // Initialize the auto-updater check on mount
  useAppUpdater();

  let active = true;
  let interval: ReturnType<typeof setInterval> | undefined;

  onMount(() => {
    console.log("[FRONTEND] App mounted. Starting DB readiness check...");
    const checkReady = async () => {
      try {
        const ready = await invoke<boolean>("check_db_ready");
        console.log("[FRONTEND] check_db_ready returned:", ready);
        if (ready && active) {
          setIsReady(true);
          return true;
        }
      } catch (e) {
        console.error("[FRONTEND] check_db_ready threw error:", e);
      }
      return false;
    };

    checkReady().then((ready) => {
      if (ready && interval) clearInterval(interval);
    });

    interval = setInterval(async () => {
      const ready = await checkReady();
      if (ready) clearInterval(interval);
    }, 500);
  });

  onCleanup(() => {
    active = false;
    if (interval) clearInterval(interval);
  });

  createEffect(() => {
    if (isReady()) {
      const splash = document.getElementById("splash");
      if (splash) {
        splash.style.opacity = "0";
        splash.style.transform = "scale(1.05)";
        setTimeout(() => splash.remove(), 600);
      }
      const root = document.getElementById("root");
      if (root) root.style.display = "block";
    }
  });

  return (
    <Show when={isReady()}>
      <ErrorBoundary
        fallback={(err) => (
          <div
            style={{
              position: "fixed",
              top: "100px",
              left: "10px",
              right: "10px",
              background: "red",
              color: "white",
              padding: "20px",
              "z-index": 999998,
              "font-family": "monospace",
              "white-space": "pre-wrap",
            }}
          >
            RENDER CRASH: {err?.stack || err?.toString()}
          </div>
        )}
      >
        <AppProvider>
          <SyncListener />
          <AddAccountModal />
          <ComposeModal />
          <CommandPalette />
          <ContextMenu />
          <SettingsModal />
          <AppShell />
        </AppProvider>
      </ErrorBoundary>
    </Show>
  );
}

export default App;
