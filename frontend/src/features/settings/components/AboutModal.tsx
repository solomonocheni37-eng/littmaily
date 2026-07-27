import { createSignal, onMount, Show, For } from "solid-js";
import { X, FileText, Shield, ScrollText, Loader2 } from "lucide-solid";

type Tab = "changelog" | "privacy" | "licenses";

interface AboutModalProps {
  show: boolean;
  onClose: () => void;
}

export default function AboutModal(props: AboutModalProps) {
  const [activeTab, setActiveTab] = createSignal<Tab>("changelog");
  const [content, setContent] = createSignal<string>("");
  const [loading, setLoading] = createSignal(false);

  const loadContent = async (tab: Tab) => {
    setLoading(true);
    setActiveTab(tab);
    try {
      const file = tab === "changelog" ? "/CHANGELOG.md" : tab === "privacy" ? "/PRIVACY.md" : "/licenses.txt";
      const res = await fetch(file);
      if (res.ok) {
        setContent(await res.text());
      } else {
        setContent(`Failed to load ${file}.\n\nPlease ensure the file exists in the public directory and you have generated the licenses file.`);
      }
    } catch (e) {
      setContent("Error loading document.");
    } finally {
      setLoading(false);
    }
  };

  onMount(() => loadContent("changelog"));

  const tabs = [
    { id: "changelog" as Tab, label: "Changelog", icon: ScrollText },
    { id: "privacy" as Tab, label: "Privacy Policy", icon: Shield },
    { id: "licenses" as Tab, label: "Open Source", icon: FileText },
  ];

  return (
    <Show when={props.show}>
      <div
        class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-[60] p-4"
        onClick={props.onClose}
      >
        <div
          class="bg-white dark:bg-surface-900 rounded-2xl shadow-2xl w-full max-w-3xl h-[80vh] flex flex-col border border-surface-200 dark:border-surface-800"
          onClick={(e) => e.stopPropagation()}
        >
          <div class="flex justify-between items-center p-4 border-b border-surface-200 dark:border-surface-800">
            <h2 class="text-xl font-bold text-surface-900 dark:text-surface-50">About Littmaily</h2>
            <button
              onClick={props.onClose}
              class="p-1.5 rounded-lg hover:bg-surface-100 dark:hover:bg-surface-800 text-surface-500 hover:text-surface-900 dark:hover:text-white transition-colors"
            >
              <X size={20} />
            </button>
          </div>

          <div class="flex border-b border-surface-200 dark:border-surface-800 px-4 gap-4">
            <For each={tabs}>
              {(tab) => (
                <button
                  onClick={() => loadContent(tab.id)}
                  class={`flex items-center gap-2 py-3 text-sm font-medium border-b-2 transition-colors ${
                    activeTab() === tab.id
                      ? "border-brand-500 text-brand-600 dark:text-brand-400"
                      : "border-transparent text-surface-500 hover:text-surface-800 dark:hover:text-surface-200"
                  }`}
                >
                  <tab.icon size={16} />
                  {tab.label}
                </button>
              )}
            </For>
          </div>

          <div class="flex-1 overflow-y-auto p-6 bg-surface-50 dark:bg-surface-950">
            <Show when={!loading()} fallback={<div class="flex justify-center p-8"><Loader2 class="animate-spin text-brand-500" size={24} /></div>}>
              <pre class="whitespace-pre-wrap font-sans text-sm text-surface-800 dark:text-surface-200 leading-relaxed">
                {content()}
              </pre>
            </Show>
          </div>
        </div>
      </div>
    </Show>
  );
}
