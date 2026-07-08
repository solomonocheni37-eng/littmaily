import {
  For,
  Show,
  createEffect,
  createSignal,
  onMount,
  onCleanup,
} from "solid-js";
import { AccountsApi, EmailApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import ThemeToggle from "./ThemeToggle";
import type { Mailbox } from "@/core/types/generated";
import {
  Inbox,
  Send,
  FileText,
  Trash2,
  Plus,
  Search,
  Pencil,
  Mail,
  Settings,
  Calendar,
  Users,
  WifiOff,
  FolderPlus,
  Archive,
  ShieldAlert,
  Star,
} from "lucide-solid";
import { toast } from "@/core/ui/toast";
import { appEvents } from "@/core/events/eventBus";

const Sidebar = () => {
  const {
    state,
    selectAccount,
    selectMailbox,
    setMailboxes,
    setAccounts,
    setShowAddAccount,
    setShowCompose,
    setShowSearch,
    setShowSettings,
    setCurrentView,
  } = useAppContext();

  const [isOnline, setIsOnline] = createSignal(navigator.onLine);

  const handleOnline = () => setIsOnline(true);
  const handleOffline = () => setIsOnline(false);

  const refreshMailboxes = async () => {
    if (state.selectedAccountId) {
      try {
        const mbs = await EmailApi.getMailboxes(state.selectedAccountId);
        setMailboxes(mbs);
      } catch (e) {
        if (import.meta.env.DEV) console.error(e);
      }
    }
  };

  let cleanupMailboxes: (() => void) | undefined;

  onMount(async () => {
    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);
    cleanupMailboxes = appEvents.on("mailboxes:refresh", refreshMailboxes);

    try {
      const accs = await AccountsApi.list();
      setAccounts(accs);
      if (accs.length > 0 && !state.selectedAccountId)
        selectAccount(accs[0].id);
    } catch (e) {
      if (import.meta.env.DEV) console.error("Failed to fetch accounts:", e);
    }
  });

  onCleanup(() => {
    window.removeEventListener("online", handleOnline);
    window.removeEventListener("offline", handleOffline);
    cleanupMailboxes?.();
  });

  createEffect(() => {
    if (state.selectedAccountId)
      EmailApi.getMailboxes(state.selectedAccountId)
        .then((mbs) => {
          setMailboxes(mbs);
          if (
            mbs.length > 0 &&
            !mbs.find((m) => m.name === state.selectedMailboxName) &&
            !state.selectedMailboxName?.startsWith("__")
          ) {
            const inbox =
              mbs.find(
                (m) =>
                  m.name.toLowerCase() === "inbox" ||
                  m.attributes.includes("\\Inbox")
              ) || mbs[0];
            selectMailbox(inbox.name);
          }
        })
        .catch((e) => {
          if (import.meta.env.DEV) console.error(e);
        });
  });

  const handleCreateFolder = async () => {
    const name = prompt("Enter new folder name:");
    if (name && state.selectedAccountId) {
      try {
        await EmailApi.createFolder(state.selectedAccountId, name);
        toast("Folder created");
        EmailApi.getMailboxes(state.selectedAccountId).then(setMailboxes);
      } catch (e) {
        toast("Failed to create folder");
      }
    }
  };

  const getIcon = (name: string) => {
    const n = name.toLowerCase();
    if (n.includes("inbox")) return <Inbox size={16} />;
    if (n.includes("sent")) return <Send size={16} />;
    if (n.includes("draft")) return <FileText size={16} />;
    if (n.includes("trash") || n.includes("deleted"))
      return <Trash2 size={16} />;
    if (n.includes("spam") || n.includes("junk"))
      return <ShieldAlert size={16} />;
    if (n.includes("archive") || n.includes("all mail"))
      return <Archive size={16} />;
    if (n.includes("starred") || n.includes("favorites"))
      return <Star size={16} />;
    return <Mail size={16} />;
  };

  return (
    <div class="h-full flex flex-col p-4 gap-6 overflow-y-auto glass-panel">
      <Show when={!isOnline()}>
        <div class="bg-amber-500 text-white text-xs text-center py-1.5 px-4 font-medium rounded-md flex items-center justify-center gap-2 shadow-sm">
          <WifiOff size={12} /> Offline Mode
        </div>
      </Show>

      <div class="flex gap-2 mb-2">
        <button
          onClick={() => setCurrentView("mail")}
          class={`flex-1 py-2 rounded-lg flex items-center justify-center gap-1 text-xs font-medium transition-colors ${
            state.currentView === "mail"
              ? "bg-brand-500 text-white shadow-elevated"
              : "bg-surface-100 dark:bg-surface-800 text-surface-600 dark:text-surface-400"
          }`}
        >
          <Mail size={14} />
        </button>
        <button
          onClick={() => setCurrentView("calendar")}
          class={`flex-1 py-2 rounded-lg flex items-center justify-center gap-1 text-xs font-medium transition-colors ${
            state.currentView === "calendar"
              ? "bg-brand-500 text-white shadow-elevated"
              : "bg-surface-100 dark:bg-surface-800 text-surface-600 dark:text-surface-400"
          }`}
        >
          <Calendar size={14} />
        </button>
        <button
          onClick={() => setCurrentView("contacts")}
          class={`flex-1 py-2 rounded-lg flex items-center justify-center gap-1 text-xs font-medium transition-colors ${
            state.currentView === "contacts"
              ? "bg-brand-500 text-white shadow-elevated"
              : "bg-surface-100 dark:bg-surface-800 text-surface-600 dark:text-surface-400"
          }`}
        >
          <Users size={14} />
        </button>
      </div>

      <div class="space-y-2">
        <button
          onClick={() => setShowCompose(true)}
          class="w-full py-2.5 bg-surface-900 dark:bg-surface-50 hover:bg-surface-800 dark:hover:bg-surface-200 text-white dark:text-surface-900 rounded-lg font-medium flex items-center justify-center gap-2 shadow-elevated transition-all active:scale-95"
        >
          <Pencil size={15} /> Compose
        </button>
        <button
          onClick={() => setShowSearch(true)}
          class="w-full py-2 bg-surface-100 dark:bg-surface-800/50 hover:bg-surface-200 dark:hover:bg-surface-800 rounded-lg text-sm flex items-center justify-center gap-2 text-surface-600 dark:text-surface-300 transition-colors border border-surface-200 dark:border-surface-700/50"
        >
          <Search size={14} /> Search
          <span class="ml-auto text-[10px] font-semibold opacity-60 bg-surface-200 dark:bg-surface-700 px-1.5 py-0.5 rounded">
            ⌘ K
          </span>
        </button>
      </div>

      <div>
        <div class="flex justify-between items-center mb-3 px-1">
          <h3 class="text-[11px] font-bold text-surface-400 dark:text-surface-500 uppercase tracking-widest">
            Accounts
          </h3>
          <button
            onClick={() => setShowAddAccount(true)}
            class="text-surface-400 hover:text-brand-500 transition-colors p-1 rounded hover:bg-surface-100 dark:hover:bg-surface-800"
          >
            <Plus size={14} />
          </button>
        </div>
        <Show
          when={state.accounts.length > 0}
          fallback={
            <div class="text-xs text-surface-400 px-2">No accounts</div>
          }
        >
          <div class="space-y-1">
            <For each={state.accounts}>
              {(acc) => (
                <button
                  onClick={() => selectAccount(acc.id)}
                  class={`w-full text-left px-3 py-2 rounded-lg text-sm transition-all flex items-center gap-2 ${
                    state.selectedAccountId === acc.id
                      ? "bg-surface-200/70 dark:bg-surface-800 text-surface-900 dark:text-surface-50 font-medium shadow-soft"
                      : "hover:bg-surface-100 dark:hover:bg-surface-800/50 text-surface-600 dark:text-surface-400"
                  }`}
                >
                  <div class="w-2 h-2 rounded-full bg-emerald-500 flex-shrink-0" />
                  <span class="truncate">{acc.email}</span>
                </button>
              )}
            </For>
          </div>
        </Show>
      </div>

      {/* Grouping Logic */}
      {(() => {
        const inbox = state.mailboxes.find(
          (m: Mailbox) =>
            m.name.toLowerCase() === "inbox" ||
            m.attributes.toLowerCase().includes("\\inbox")
        );
        const sent = state.mailboxes.find(
          (m: Mailbox) =>
            m.name.toLowerCase().includes("sent") ||
            m.attributes.toLowerCase().includes("\\sent")
        );
        const drafts = state.mailboxes.find(
          (m: Mailbox) =>
            m.name.toLowerCase().includes("draft") ||
            m.attributes.toLowerCase().includes("\\drafts")
        );
        const trash = state.mailboxes.find(
          (m: Mailbox) =>
            m.name.toLowerCase().includes("trash") ||
            m.name.toLowerCase().includes("deleted") ||
            m.attributes.toLowerCase().includes("\\trash")
        );

        // Virtual folders that don't exist on the IMAP server but are handled via specific SQL queries in the backend.
        const smartFolders = [
          { id: "__STARRED__", name: "Starred", icon: <Star size={16} /> },
          { id: "__ARCHIVE__", name: "Archive", icon: <Archive size={16} /> },
          { id: "__SPAM__", name: "Spam", icon: <ShieldAlert size={16} /> },
        ];

        const customFolders = state.mailboxes.filter((m: Mailbox) => {
          const n = m.name.toLowerCase();
          const a = m.attributes.toLowerCase();
          if (n === "inbox" || a.includes("\\inbox")) return false;
          if (n.includes("sent") || a.includes("\\sent")) return false;
          if (n.includes("draft") || a.includes("\\drafts")) return false;
          if (
            n.includes("trash") ||
            n.includes("deleted") ||
            a.includes("\\trash")
          )
            return false;
          if (
            n.includes("archive") ||
            n.includes("all mail") ||
            a.includes("\\archive")
          )
            return false;
          if (n.includes("spam") || n.includes("junk") || a.includes("\\junk"))
            return false;
          return true;
        });

        const renderFolder = (mb: any) => (
          <button
            onClick={() => selectMailbox(mb.name || mb.id)}
            class={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-all ${
              state.selectedMailboxName === (mb.name || mb.id)
                ? "bg-surface-200/70 dark:bg-surface-800 font-medium text-surface-900 dark:text-surface-50 shadow-soft"
                : "hover:bg-surface-100 dark:hover:bg-surface-800/50 text-surface-600 dark:text-surface-400"
            }`}
          >
            <span class="text-surface-400 dark:text-surface-500">
              {mb.icon || getIcon(mb.name)}
            </span>
            <span class="truncate flex-1 text-left">
              {mb.display_name || mb.name}
            </span>
            {mb.unread_count > 0 && (
              <span class="ml-auto text-[10px] font-bold bg-brand-500 text-white px-1.5 py-0.5 rounded-full min-w-[18px] text-center shadow-sm">
                {mb.unread_count}
              </span>
            )}
          </button>
        );

        return (
          <div class="flex-1 overflow-y-auto space-y-4">
            {/* 1. Inbox */}
            {inbox && (
              <div class="space-y-0.5">
                {renderFolder({
                  ...inbox,
                  display_name: "Inbox",
                  icon: <Inbox size={16} />,
                })}
              </div>
            )}

            {/* 2. Smart Folders */}
            <div class="space-y-0.5">
              <h3 class="text-[11px] font-bold text-surface-400 dark:text-surface-500 uppercase tracking-widest px-3 mb-1">
                Smart Folders
              </h3>
              <For each={smartFolders}>{renderFolder}</For>
            </div>

            {/* 3. Standard Folders */}
            <div class="space-y-0.5">
              <h3 class="text-[11px] font-bold text-surface-400 dark:text-surface-500 uppercase tracking-widest px-3 mb-1">
                Folders
              </h3>
              {sent &&
                renderFolder({
                  ...sent,
                  display_name: "Sent",
                  icon: <Send size={16} />,
                })}
              {drafts &&
                renderFolder({
                  ...drafts,
                  display_name: "Drafts",
                  icon: <FileText size={16} />,
                })}
              {trash &&
                renderFolder({
                  ...trash,
                  display_name: "Trash",
                  icon: <Trash2 size={16} />,
                })}
            </div>

            {/* 4. Custom Folders */}
            {customFolders.length > 0 && (
              <div class="space-y-0.5">
                <div class="flex justify-between items-center px-3 mb-1">
                  <h3 class="text-[11px] font-bold text-surface-400 dark:text-surface-500 uppercase tracking-widest">
                    Labels
                  </h3>
                  <button
                    onClick={handleCreateFolder}
                    class="text-surface-400 hover:text-brand-500 transition-colors p-1 rounded hover:bg-surface-100 dark:hover:bg-surface-800"
                    title="Create Folder"
                  >
                    <FolderPlus size={14} />
                  </button>
                </div>
                <For each={customFolders}>{renderFolder}</For>
              </div>
            )}
          </div>
        );
      })()}

      <div class="mt-auto pt-4 border-t border-surface-200 dark:border-surface-800 space-y-1">
        <ThemeToggle />
        <button
          onClick={() => setShowSettings(true)}
          class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm text-surface-600 dark:text-surface-400 hover:bg-surface-100 dark:hover:bg-surface-800/50 transition-all"
        >
          <Settings size={16} />
          <span>Settings</span>
        </button>
      </div>
    </div>
  );
};

export default Sidebar;
