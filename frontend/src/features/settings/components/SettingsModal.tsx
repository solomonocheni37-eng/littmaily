// FILE: ./frontend/src/features/settings/components/SettingsModal.tsx
import { Show, For, createSignal } from "solid-js";
import { Portal } from "solid-js/web";
import { invoke } from "@tauri-apps/api/core";
import { AccountsApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import { X, Trash2, Loader2, FolderOpen, Database, Info } from "lucide-solid";
import { toast } from "@/core/ui/toast";
import { confirm } from "@tauri-apps/plugin-dialog";
import AboutModal from "./AboutModal";

export default function SettingsModal() {
  const { state, setShowSettings, setAccounts, selectAccount } =
    useAppContext();
  const [loadingId, setLoadingId] = createSignal<string | null>(null);
  const [showAbout, setShowAbout] = createSignal(false);

  const handleDelete = async (id: string, email: string) => {
    const isConfirmed = await confirm(
      `Are you sure you want to remove ${email}? This will delete all local cached data.`,
      { title: "Confirm Deletion", okLabel: "Delete", cancelLabel: "Cancel" }
    );
    if (!isConfirmed) return;
    setLoadingId(id);
    try {
      await AccountsApi.delete(id);
      const updatedAccs = await AccountsApi.list();
      setAccounts(updatedAccs);
      if (state.selectedAccountId === id) {
        if (updatedAccs.length > 0) {
          selectAccount(updatedAccs[0].id);
        } else {
          selectAccount(null);
        }
      }
      toast(`Account ${email} removed.`);
    } catch (e) {
      if (import.meta.env.DEV) console.error(e);
      toast("Failed to delete account");
    } finally {
      setLoadingId(null);
    }
  };

  const handleSyncWindowChange = async (id: string, value: string) => {
    try {
      await AccountsApi.updateSyncWindow(id, value);
      toast("Sync preference updated. Resyncing...");
      const updatedAccs = await AccountsApi.list();
      setAccounts(updatedAccs);
    } catch (e) {
      toast("Failed to update sync window");
    }
  };

  return (
    <Show when={state.showSettings}>
      <Portal>
        <div
          class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 p-4"
          onClick={() => setShowSettings(false)}
        >
          <div
            class="bg-white dark:bg-surface-900 rounded-2xl shadow-2xl w-full max-w-md p-6 border border-surface-200 dark:border-surface-800 max-h-[80vh] overflow-y-auto"
            onClick={(e) => e.stopPropagation()}
          >
            <div class="flex justify-between items-center mb-6">
              <h2 class="text-xl font-bold text-surface-900 dark:text-surface-50">
                Settings
              </h2>
              <button
                onClick={() => setShowSettings(false)}
                class="text-surface-500 hover:text-surface-900 dark:hover:text-white"
              >
                <X size={20} />
              </button>
            </div>

            <div class="space-y-6">
              <div>
                <h3 class="text-sm font-semibold text-surface-700 dark:text-surface-300 uppercase tracking-wider mb-3">
                  Accounts & Storage
                </h3>
                <div class="space-y-3">
                  <For each={state.accounts}>
                    {(acc) => (
                      <div class="p-4 bg-surface-50 dark:bg-surface-800/50 rounded-lg border border-surface-200 dark:border-surface-700/50 space-y-3">
                        <div class="flex items-center justify-between">
                          <div class="flex items-center gap-3 min-w-0">
                            <div class="w-8 h-8 rounded-full bg-brand-500/20 text-brand-600 dark:text-brand-400 flex items-center justify-center text-sm font-bold">
                              {acc.email.charAt(0).toUpperCase()}
                            </div>
                            <div class="min-w-0">
                              <div class="text-sm font-medium truncate text-surface-900 dark:text-surface-50">
                                {acc.email}
                              </div>
                              <div class="text-xs text-surface-500 dark:text-surface-400 truncate capitalize">
                                {acc.provider}
                              </div>
                            </div>
                          </div>
                          <button
                            onClick={() => handleDelete(acc.id, acc.email)}
                            disabled={loadingId() === acc.id}
                            class="p-2 text-surface-400 hover:text-red-500 hover:bg-red-500/10 rounded-md transition-colors disabled:opacity-50"
                          >
                            <Show
                              when={loadingId() === acc.id}
                              fallback={<Trash2 size={16} />}
                            >
                              <Loader2 size={16} class="animate-spin" />
                            </Show>
                          </button>
                        </div>

                        <div class="flex items-center gap-2 pt-2 border-t border-surface-200 dark:border-surface-700/50">
                          <Database
                            size={14}
                            class="text-surface-500 dark:text-surface-400"
                          />
                          <select
                            value={acc.sync_window}
                            onChange={(e) =>
                              handleSyncWindowChange(
                                acc.id,
                                e.currentTarget.value
                              )
                            }
                            class="flex-1 text-xs bg-surface-100 dark:bg-surface-700 text-surface-900 dark:text-surface-200 border border-surface-200 dark:border-surface-600 rounded px-2 py-1 outline-none focus:ring-1 focus:ring-brand-500"
                          >
                            <option value="LAST_30_DAYS">
                              Keep Last 30 Days (Recommended)
                            </option>
                            <option value="LAST_6_MONTHS">
                              Keep Last 6 Months
                            </option>
                            <option value="LAST_100_MESSAGES">
                              Keep Last 100 Messages
                            </option>
                            <option value="EVERYTHING">
                              Keep Everything (High Disk Usage)
                            </option>
                          </select>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </div>

              <div class="pt-6 border-t border-surface-200 dark:border-surface-800 space-y-3">
                <h3 class="text-sm font-semibold text-surface-700 dark:text-surface-300 uppercase tracking-wider mb-3">
                  Support & Debugging
                </h3>

                <button
                  onClick={() => setShowAbout(true)}
                  class="w-full flex items-center justify-between p-3 bg-surface-50 dark:bg-surface-800/50 rounded-lg border border-surface-200 dark:border-surface-700/50 hover:bg-surface-100 dark:hover:bg-surface-800 transition-colors"
                >
                  <div class="flex items-center gap-3">
                    <Info size={16} class="text-brand-500" />
                    <div class="text-left">
                      <div class="text-sm font-medium text-surface-900 dark:text-surface-200">
                        About & Legal
                      </div>
                      <div class="text-xs text-surface-500 dark:text-surface-400">
                        Changelog, Privacy, and Licenses
                      </div>
                    </div>
                  </div>
                </button>

                <div class="p-3 bg-surface-50 dark:bg-surface-800/50 rounded-lg border border-surface-200 dark:border-surface-700/50 flex items-center justify-between">
                  <div class="min-w-0 mr-4">
                    <div class="text-sm font-medium text-surface-900 dark:text-surface-200">
                      Application Logs
                    </div>
                    <div class="text-xs text-surface-500 dark:text-surface-400 mt-1">
                      Open the folder containing local debug logs.
                    </div>
                  </div>
                  <button
                    onClick={() => invoke("open_logs_folder")}
                    class="p-2.5 bg-brand-500/10 hover:bg-brand-500/20 text-brand-600 dark:text-brand-400 rounded-lg transition-colors flex-shrink-0"
                  >
                    <FolderOpen size={18} />
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Render the About Modal */}
        <AboutModal show={showAbout()} onClose={() => setShowAbout(false)} />
      </Portal>
    </Show>
  );
}
