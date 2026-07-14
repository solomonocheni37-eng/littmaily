// ./frontend/src/features/sync/SyncListener.tsx
import { onMount, onCleanup, For, Show } from "solid-js";
import { listen } from "@tauri-apps/api/event";
import { commands } from "@/core/types/generated";
import { toasts, setToasts } from "@/core/ui/toast";
import { useAppContext } from "@/core/store/AppStore";
import { appEvents } from "@/core/events/eventBus";

type SyncPayload = { account_id: string; mailbox: string; new_uids: number[] };
type StatePayload = { account_id: string; mailbox: string };

const SyncListener = () => {
  const { state } = useAppContext();
  let unlistenFn: (() => void) | undefined;
  let unlistenErrorFn: (() => void) | undefined;
  let unlistenStateFn: (() => void) | undefined;

  onMount(async () => {
    // ==========================================
    // GLOBAL AUTO-SYNC FOR BADGES & SIDEBAR
    // ==========================================
    // Catches all user interactions (Swipe, Hotkeys, Context Menu, Reading Pane)
    // and automatically updates the OS Badge and Sidebar counts after the DB updates.
    const cleanupAction = appEvents.on("email:action", () => {
      setTimeout(async () => {
        appEvents.emit("mailboxes:refresh");
        try {
          await commands.updateBadgeCount();
        } catch (e) {
          if (import.meta.env.DEV) console.error("Badge update failed:", e);
        }
      }, 300); // 300ms debounce allows the Rust optimistic DB update to commit
    });

    // Listens to Tauri backend events for background syncs
    unlistenFn = await listen<SyncPayload>("sync:new-email", async (event) => {
      const mb = event.payload.mailbox;
      if (
        event.payload.account_id === state.selectedAccountId &&
        mb === state.selectedMailboxName
      ) {
        appEvents.emit("inbox:refresh");
      }
      appEvents.emit("mailboxes:refresh");
      try { await commands.updateBadgeCount(); } catch (e) {}
    });

    unlistenStateFn = await listen<StatePayload>(
      "sync:state-updated",
      async (event) => {
        if (
          event.payload.account_id === state.selectedAccountId &&
          event.payload.mailbox === state.selectedMailboxName
        ) {
          appEvents.emit("inbox:refresh");
        }
        appEvents.emit("mailboxes:refresh");
        try { await commands.updateBadgeCount(); } catch (e) {}
      }
    );

    unlistenErrorFn = await listen<string>("sync:error", (event) => {
      console.error("Sync error:", event.payload);
    });

    onCleanup(() => cleanupAction());
  });

  onCleanup(() => {
    if (unlistenFn) unlistenFn();
    if (unlistenErrorFn) unlistenErrorFn();
    if (unlistenStateFn) unlistenStateFn();
  });

  return (
    <div class="fixed bottom-4 right-4 z-50 flex flex-col gap-2 pointer-events-none">
      <For each={toasts()}>
        {(t) => (
          <div class="bg-surface-800 text-white px-4 py-3 rounded-lg shadow-lg animate-slide-up pointer-events-auto max-w-xs flex items-center justify-between gap-4">
            <span>{t.msg}</span>
            <Show when={t.actionLabel && t.onAction}>
              <button
                class="text-brand-400 font-semibold hover:underline whitespace-nowrap"
                onClick={() => {
                  t.onAction!();
                  setToasts((prev) => prev.filter((x) => x.id !== t.id));
                }}
              >
                {t.actionLabel}
              </button>
            </Show>
          </div>
        )}
      </For>
    </div>
  );
};

export default SyncListener;
