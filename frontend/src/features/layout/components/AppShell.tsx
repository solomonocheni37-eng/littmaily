import { createSignal, Show } from "solid-js";
import Sidebar from "./Sidebar";
import EmailListPane from "@/features/email/views/EmailListPane";
import ReadingPane from "@/features/email/views/ReadingPane";
import CalendarView from "@/features/calendar/views/CalendarView";
import ContactsView from "@/features/contacts/views/ContactsView";
import { ResizeHandle } from "./ResizeHandle";
import { OnboardingHero } from "./OnboardingHero";
import { useAppContext } from "@/core/store/AppStore";
import { useGlobalHotkeys } from "../hooks/useGlobalHotkeys";
import { PanelLeftOpen } from "lucide-solid";

export function AppShell() {
  const { state, toggleListPane } = useAppContext();
  const [sidebarWidth, setSidebarWidth] = createSignal(256);
  const [listWidth, setListWidth] = createSignal(384);

  // Register global hotkeys cleanly
  useGlobalHotkeys();

  const handleSidebarResize = (deltaX: number) =>
    setSidebarWidth((prev) => Math.max(200, Math.min(450, prev + deltaX)));
  const handleListResize = (deltaX: number) =>
    setListWidth((prev) => Math.max(280, Math.min(700, prev + deltaX)));

  return (
    <div class="flex h-screen w-screen overflow-hidden bg-surface-50 dark:bg-surface-950 text-surface-900 dark:text-surface-50 font-sans">
      <div
        style={{ width: `${sidebarWidth()}px` }}
        class="flex-shrink-0 border-r border-surface-200 dark:border-surface-800 overflow-hidden"
      >
        <Sidebar />
      </div>
      <ResizeHandle onResize={handleSidebarResize} />

      <Show when={state.accounts.length > 0} fallback={<OnboardingHero />}>
        <Show
          when={state.currentView === "mail"}
          fallback={
            <div class="flex-1 min-w-0 overflow-hidden">
              {state.currentView === "calendar" && <CalendarView />}
              {state.currentView === "contacts" && <ContactsView />}
            </div>
          }
        >
          <div
            style={{
              width: state.isListPaneCollapsed ? "0px" : `${listWidth()}px`,
            }}
            class={`flex-shrink-0 overflow-hidden transition-[width] duration-200 ease-in-out ${
              !state.isListPaneCollapsed
                ? "border-r border-surface-200 dark:border-surface-800"
                : ""
            }`}
          >
            <div style={{ width: `${listWidth()}px` }} class="h-full">
              <EmailListPane />
            </div>
          </div>
          <Show when={!state.isListPaneCollapsed}>
            <ResizeHandle onResize={handleListResize} />
          </Show>
          <div class="flex-1 min-w-[400px] overflow-hidden relative">
            <Show when={state.isListPaneCollapsed}>
              <button
                onClick={toggleListPane}
                class="absolute top-4 left-4 z-20 p-2 bg-surface-100 dark:bg-surface-800 hover:bg-surface-200 dark:hover:bg-surface-700 rounded-lg shadow-elevated text-surface-600 dark:text-surface-300 transition-colors"
                title="Expand List"
              >
                <PanelLeftOpen size={20} />
              </button>
            </Show>
            <ReadingPane />
          </div>
        </Show>
      </Show>
    </div>
  );
}
