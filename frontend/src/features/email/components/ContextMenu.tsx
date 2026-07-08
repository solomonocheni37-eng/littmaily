import {
  Show,
  onMount,
  onCleanup,
  For,
  createSignal,
  createMemo,
} from "solid-js";
import { Portal } from "solid-js/web";
import { useAppContext } from "@/core/store/AppStore";
import { EmailApi } from "@/core/ipc";
import {
  Trash2,
  MailOpen,
  MailMinus,
  Folder,
  ChevronRight,
  Star,
} from "lucide-solid";
import { appEvents } from "@/core/events/eventBus";
import { hasFlag } from "../utils/flagUtils";

export default function ContextMenu() {
  const { state, setContextMenu, selectEmail } = useAppContext();
  const [showMoveMenu, setShowMoveMenu] = createSignal(false);

  const isStarred = createMemo(() => {
    if (!state.contextMenu) return false;
    return hasFlag(state.contextMenu.email.flags, "Flagged");
  });

  const close = () => {
    setContextMenu(null);
    setShowMoveMenu(false);
  };

  onMount(() => {
    window.addEventListener("click", close);
    // Use capture phase to close the menu if the user scrolls the underlying page.
    window.addEventListener("scroll", close, true);
  });

  onCleanup(() => {
    window.removeEventListener("click", close);
    window.removeEventListener("scroll", close, true);
  });

  const handleAction = async (
    action: "read" | "unread" | "delete" | "star" | "unstar"
  ) => {
    const ctx = state.contextMenu;
    if (!ctx) return;

    appEvents.emit("email:action", { uid: ctx.email.uid, action });
    try {
      await EmailApi.updateState(
        ctx.email.account_id,
        ctx.email.mailbox_name,
        ctx.email.uid,
        action
      );
      appEvents.emit("mailboxes:refresh");
      if (action === "delete" && state.selectedEmail?.uid === ctx.email.uid) {
        selectEmail(null);
      }
    } catch (e) {
      if (import.meta.env.DEV) console.error(e);
    } finally {
      setContextMenu(null);
    }
  };

  const handleMove = async (dest: string) => {
    const ctx = state.contextMenu;
    if (!ctx) return;

    try {
      appEvents.emit("email:action", {
        uid: ctx.email.uid,
        action: "move",
        destMailbox: dest,
      });
      await EmailApi.updateState(
        ctx.email.account_id,
        ctx.email.mailbox_name,
        ctx.email.uid,
        "move",
        dest
      );
      appEvents.emit("mailboxes:refresh");
      if (state.selectedEmail?.uid === ctx.email.uid) selectEmail(null);
    } catch (e) {
      if (import.meta.env.DEV) console.error(e);
    } finally {
      setContextMenu(null);
    }
  };

  return (
    <Show when={state.contextMenu}>
      <Portal>
        <div
          class="fixed z-[100] bg-white dark:bg-surface-800 rounded-lg shadow-xl border border-surface-200 dark:border-surface-700 py-1 min-w-[160px]"
          style={{
            top: `${state.contextMenu!.y}px`,
            left: `${state.contextMenu!.x}px`,
          }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            onClick={() => handleAction("read")}
            class="w-full px-4 py-2 text-sm text-left text-surface-700 dark:text-surface-200 hover:bg-surface-100 dark:hover:bg-surface-700 flex items-center gap-2"
          >
            <MailOpen size={14} /> Mark as Read
          </button>
          <button
            onClick={() => handleAction("unread")}
            class="w-full px-4 py-2 text-sm text-left text-surface-700 dark:text-surface-200 hover:bg-surface-100 dark:hover:bg-surface-700 flex items-center gap-2"
          >
            <MailMinus size={14} /> Mark as Unread
          </button>
          <button
            onClick={() => handleAction(isStarred() ? "unstar" : "star")}
            class="w-full px-4 py-2 text-sm text-left text-surface-700 dark:text-surface-200 hover:bg-surface-100 dark:hover:bg-surface-700 flex items-center gap-2"
          >
            <Star
              size={14}
              class={isStarred() ? "fill-amber-400 text-amber-400" : ""}
            />
            {isStarred() ? "Unstar" : "Star"}
          </button>

          <div class="relative">
            <button
              onClick={() => setShowMoveMenu(!showMoveMenu())}
              class="w-full px-4 py-2 text-sm text-left text-surface-700 dark:text-surface-200 hover:bg-surface-100 dark:hover:bg-surface-700 flex items-center justify-between"
            >
              <span class="flex items-center gap-2">
                <Folder size={14} /> Move to...
              </span>
              <ChevronRight size={14} />
            </button>
            <Show when={showMoveMenu()}>
              <div class="absolute left-full top-0 ml-1 bg-white dark:bg-surface-800 rounded-lg shadow-xl border border-surface-200 dark:border-surface-700 py-1 min-w-[160px] max-h-60 overflow-y-auto">
                <For
                  each={state.mailboxes.filter(
                    (m: any) => !m.name.startsWith("__")
                  )}
                >
                  {(mb) => (
                    <button
                      onClick={() => handleMove(mb.name)}
                      class="w-full px-4 py-2 text-sm text-left text-surface-600 dark:text-surface-300 hover:bg-surface-100 dark:hover:bg-surface-700 truncate"
                    >
                      {mb.name}
                    </button>
                  )}
                </For>
              </div>
            </Show>
          </div>

          <div class="my-1 border-t border-surface-200 dark:border-surface-700"></div>
          <button
            onClick={() => handleAction("delete")}
            class="w-full px-4 py-2 text-sm text-left text-red-500 hover:bg-red-500/10 flex items-center gap-2"
          >
            <Trash2 size={14} /> Delete
          </button>
        </div>
      </Portal>
    </Show>
  );
}
