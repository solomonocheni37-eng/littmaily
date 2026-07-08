/**
 * Renders upcoming calendar events grouped by relative time buckets (Today, Tomorrow, etc.).
 * Uses a deterministic hash of the event summary to assign stable avatar colors,
 * preventing UI flicker when the component re-renders.
 */
import { createMemo, Show, For, createResource } from "solid-js";
import { CalendarApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import { Calendar as CalIcon, Loader2, MapPin, FileText } from "lucide-solid";
import { parseICal } from "../utils/icalParser";
import { format, isToday, isTomorrow, isPast, isThisWeek } from "date-fns";

const CalendarView = () => {
  const { state } = useAppContext();

  const [events] = createResource(
    () => state.selectedAccountId,
    async (accId) => (accId ? await CalendarApi.getEvents(accId) : [])
  );

  const parsedEvents = createMemo(() => {
    const raw = events() || [];
    return raw
      .map((e) => ({ raw: e, data: parseICal(e.ical_data) }))
      .filter((e) => e.data.dtstart !== null)
      .sort((a, b) => a.data.dtstart!.getTime() - b.data.dtstart!.getTime());
  });

  const groupedEvents = createMemo(() => {
    const groups: Record<
      string,
      {
        title: string;
        events: { raw: any; data: ReturnType<typeof parseICal> }[];
      }
    > = {
      past: { title: "Past Events", events: [] },
      today: { title: "Today", events: [] },
      tomorrow: { title: "Tomorrow", events: [] },
      thisWeek: { title: "This Week", events: [] },
      later: { title: "Later", events: [] },
    };

    for (const evt of parsedEvents()) {
      const d = evt.data.dtstart!;
      if (isToday(d)) groups.today.events.push(evt);
      else if (isPast(d)) groups.past.events.push(evt);
      else if (isTomorrow(d)) groups.tomorrow.events.push(evt);
      else if (isThisWeek(d)) groups.thisWeek.events.push(evt);
      else groups.later.events.push(evt);
    }
    return groups;
  });

  const formatTime = (d: Date | null, isAllDay: boolean) => {
    if (!d) return "";
    if (isAllDay) return "All Day";
    return format(d, "h:mm a");
  };

  const getEventColor = (summary: string) => {
    const colors = [
      "bg-indigo-500",
      "bg-rose-500",
      "bg-emerald-500",
      "bg-amber-500",
      "bg-sky-500",
      "bg-fuchsia-500",
    ];
    let hash = 0;
    for (let i = 0; i < summary.length; i++)
      hash = summary.charCodeAt(i) + ((hash << 5) - hash);
    return colors[Math.abs(hash) % colors.length];
  };

  return (
    <div class="h-full flex flex-col bg-surface-50 dark:bg-surface-950 text-surface-900 dark:text-surface-50 overflow-hidden">
      <div class="p-6 border-b border-surface-200 dark:border-surface-800 bg-white dark:bg-surface-900 flex-shrink-0">
        <h1 class="text-2xl font-bold flex items-center gap-3">
          <CalIcon class="text-brand-500" /> Calendar
        </h1>
        <p class="text-sm text-surface-500 mt-1">
          Your upcoming events and schedule.
        </p>
      </div>
      <div class="flex-1 overflow-y-auto p-6">
        <Show
          when={!events.loading}
          fallback={
            <div class="flex items-center justify-center h-full text-surface-500">
              <Loader2 class="animate-spin mr-2" /> Syncing calendar...
            </div>
          }
        >
          <Show
            when={parsedEvents().length > 0}
            fallback={
              <div class="flex flex-col items-center justify-center h-full text-surface-400">
                <CalIcon size={48} class="mb-4 opacity-50" />
                <h3 class="text-lg font-medium">No events found</h3>
                <p class="text-sm">
                  Your calendar events will appear here once synced.
                </p>
              </div>
            }
          >
            <div class="max-w-3xl mx-auto space-y-8">
              <For each={["today", "tomorrow", "thisWeek", "later", "past"]}>
                {(key) => {
                  const group = groupedEvents()[key];
                  return (
                    <Show when={group.events.length > 0}>
                      <div>
                        <h2
                          class={`text-sm font-bold uppercase tracking-wider mb-4 px-2 ${
                            key === "past"
                              ? "text-surface-400"
                              : "text-brand-500"
                          }`}
                        >
                          {group.title}
                        </h2>
                        <div class="space-y-3">
                          <For each={group.events}>
                            {(evt) => (
                              <div
                                class={`bg-white dark:bg-surface-900 rounded-xl border border-surface-200 dark:border-surface-800 shadow-sm overflow-hidden flex transition-all hover:shadow-md ${
                                  key === "past" ? "opacity-60" : ""
                                }`}
                              >
                                <div
                                  class={`w-1.5 flex-shrink-0 ${getEventColor(
                                    evt.data.summary
                                  )}`}
                                ></div>
                                <div class="flex-1 p-4 flex flex-col sm:flex-row sm:items-center gap-4">
                                  <div class="sm:w-32 flex-shrink-0">
                                    <div class="text-sm font-semibold text-surface-900 dark:text-surface-50">
                                      {formatTime(
                                        evt.data.dtstart,
                                        evt.data.isAllDay
                                      )}
                                    </div>
                                    <Show
                                      when={
                                        !evt.data.isAllDay && evt.data.dtend
                                      }
                                    >
                                      <div class="text-xs text-surface-500">
                                        {format(evt.data.dtend!, "h:mm a")}
                                      </div>
                                    </Show>
                                    <Show when={evt.data.isAllDay}>
                                      <div class="text-xs text-surface-500">
                                        All Day
                                      </div>
                                    </Show>
                                    <div class="text-xs text-surface-400 mt-1 sm:hidden">
                                      {format(evt.data.dtstart!, "MMM d, yyyy")}
                                    </div>
                                  </div>
                                  <div class="flex-1 min-w-0">
                                    <h3 class="text-base font-semibold truncate mb-1">
                                      {evt.data.summary}
                                    </h3>
                                    <Show when={evt.data.location}>
                                      <div class="flex items-center gap-1.5 text-xs text-surface-500 mb-1 truncate">
                                        <MapPin
                                          size={12}
                                          class="flex-shrink-0"
                                        />{" "}
                                        {evt.data.location}
                                      </div>
                                    </Show>
                                    <Show when={evt.data.description}>
                                      <div class="flex items-start gap-1.5 text-xs text-surface-400 line-clamp-2">
                                        <FileText
                                          size={12}
                                          class="flex-shrink-0 mt-0.5"
                                        />{" "}
                                        {evt.data.description}
                                      </div>
                                    </Show>
                                  </div>
                                  <div class="hidden sm:block text-xs text-surface-400 flex-shrink-0">
                                    {format(evt.data.dtstart!, "MMM d, yyyy")}
                                  </div>
                                </div>
                              </div>
                            )}
                          </For>
                        </div>
                      </div>
                    </Show>
                  );
                }}
              </For>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
};

export default CalendarView;
