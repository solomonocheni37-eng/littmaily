// FILE: ./frontend/src/features/search/components/CommandPalette.tsx
import { createSignal, Show, For, onMount, onCleanup } from "solid-js";
import { Portal } from "solid-js/web";
import { SearchApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import { Search, Mail, Calendar, User, X } from "lucide-solid";
import type { UnifiedSearchItem } from "@/core/types/generated";
import { appEvents } from "@/core/events/eventBus";

export default function CommandPalette() {
  const { state, setShowSearch, selectEmail } = useAppContext();
  const [query, setQuery] = createSignal("");
  const [results, setResults] = createSignal<UnifiedSearchItem[]>([]);
  const [, setLoading] = createSignal(false);
  let inputRef: HTMLInputElement | undefined;

  const handleSearch = async (q: string) => {
    if (!q.trim()) {
      setResults([]);
      return;
    }
    setLoading(true);
    try {
      const res = await SearchApi.unified(q, 20);
      setResults(res);
    } catch (e) {
      if (import.meta.env.DEV) console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const handler = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "k") {
      e.preventDefault();
      setShowSearch(!state.showSearch);
    }
    if (e.key === "Escape" && state.showSearch) {
      setShowSearch(false);
    }
  };

  onMount(() => {
    window.addEventListener("keydown", handler);
  });

  onCleanup(() => {
    window.removeEventListener("keydown", handler);
  });

  const getIcon = (type: string) => {
    if (type === "email") return <Mail size={16} class="text-blue-500" />;
    if (type === "event") return <Calendar size={16} class="text-green-500" />;
    if (type === "contact") return <User size={16} class="text-purple-500" />;
    return <Search size={16} />;
  };

  const handleSelect = (item: UnifiedSearchItem) => {
    if (item.item_type === "email" && item.data.type === "Email") {
      const record = item.data.record;

      // If the selected search result is already open, trigger a reopen
      if (
        state.selectedEmail &&
        state.selectedEmail.uid === record.uid &&
        state.selectedEmail.account_id === record.account_id &&
        state.selectedEmail.mailbox_name === record.mailbox_name
      ) {
        appEvents.emit("email:reopen", { uid: record.uid });
      } else {
        selectEmail(record);
      }
      setShowSearch(false);
    }
  };

  return (
    <Show when={state.showSearch}>
      <Portal>
        <div
          class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-start justify-center pt-[15vh] z-50 p-4"
          onClick={() => setShowSearch(false)}
        >
          <div
            class="bg-white dark:bg-surface-900 rounded-xl shadow-2xl w-full max-w-xl border border-surface-200 dark:border-surface-800 overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            <div class="flex items-center gap-3 p-4 border-b border-surface-200 dark:border-surface-800">
              <Search
                size={20}
                class="text-surface-400 dark:text-surface-500"
              />
              <input
                ref={inputRef}
                type="text"
                placeholder="Search emails, contacts, events..."
                value={query()}
                onInput={(e) => {
                  setQuery(e.currentTarget.value);
                  handleSearch(e.currentTarget.value);
                }}
                autofocus
                class="flex-1 bg-transparent outline-none text-lg text-surface-900 dark:text-surface-50 placeholder:text-surface-400 dark:placeholder:text-surface-500"
              />
              <button
                onClick={() => setShowSearch(false)}
                class="text-surface-400 hover:text-surface-900 dark:hover:text-white"
              >
                <X size={20} />
              </button>
            </div>
            <div class="max-h-[400px] overflow-y-auto p-2">
              <Show
                when={results().length > 0}
                fallback={
                  <div class="p-8 text-center text-surface-500 dark:text-surface-400">
                    Type to search your local database...
                  </div>
                }
              >
                <For each={results()}>
                  {(item) => (
                    <button
                      onClick={() => handleSelect(item)}
                      class="w-full flex items-center gap-3 p-3 rounded-lg hover:bg-surface-100 dark:hover:bg-surface-800 text-left transition-colors"
                    >
                      {getIcon(item.item_type)}
                      <div class="flex-1 min-w-0">
                        <div class="font-medium truncate text-surface-900 dark:text-surface-50">
                          {item.title}
                        </div>
                        <div class="text-xs text-surface-500 dark:text-surface-400 truncate">
                          {item.subtitle}
                        </div>
                      </div>
                      <span class="text-xs uppercase tracking-wider text-surface-500 dark:text-surface-400 bg-surface-100 dark:bg-surface-800 px-2 py-0.5 rounded">
                        {item.item_type}
                      </span>
                    </button>
                  )}
                </For>
              </Show>
            </div>
          </div>
        </div>
      </Portal>
    </Show>
  );
}
