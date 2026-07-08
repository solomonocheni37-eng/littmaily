// ./frontend/src/features/email/components/EmailListItem.tsx
import type { Component } from "solid-js";
import { createMemo, Show } from "solid-js";
import type { Message } from "@/core/types/generated";
import { formatDistanceToNow } from "date-fns";
import { Paperclip, CheckSquare, Square, Star, Trash2 } from "lucide-solid";
import { useAppContext } from "@/core/store/AppStore";
import { EmailApi } from "@/core/ipc";
import { useSwipeGesture } from "../hooks/useSwipeGesture";
import { hasFlag } from "../utils/flagUtils";
import { appEvents } from "@/core/events/eventBus";

interface Props {
  email: Message;
  isSelectionMode?: boolean;
  isSelected?: boolean;
  isReading?: boolean;
  checked?: boolean;
  onToggleSelect?: (id: string) => void;
  style?: Record<string, string>;
}

const getGradient = (name: string) => {
  const gradients = [
    "from-rose-400 to-orange-300",
    "from-indigo-400 to-purple-300",
    "from-emerald-400 to-teal-300",
    "from-sky-400 to-blue-300",
    "from-amber-400 to-yellow-300",
    "from-fuchsia-400 to-pink-300",
  ];
  let hash = 0;
  for (let i = 0; i < name.length; i++)
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  return gradients[Math.abs(hash) % gradients.length];
};

const EmailListItem: Component<Props> = (props) => {
  const { state, setContextMenu } = useAppContext();
  const {
    offset,
    isDragging,
    isSwiping,
    handlePointerDown,
    handlePointerMove,
    handlePointerUp,
  } = useSwipeGesture(
    () => props.email.account_id,
    () => props.email.mailbox_name,
    () => props.email.uid
  );

  const isFocused = createMemo(() => state.focusedUid === props.email.uid);
  const isRead = createMemo(() => hasFlag(props.email.flags, "Seen"));
  const isStarred = createMemo(() => hasFlag(props.email.flags, "Flagged"));

  const formattedDate = createMemo(() => {
    try {
      return formatDistanceToNow(new Date(props.email.date || Date.now()), {
        addSuffix: true,
      });
    } catch {
      return "";
    }
  });

  const senderName = createMemo(
    () => props.email.sender?.split("<")[0].trim() || "Unknown"
  );
  const avatarGradient = createMemo(() => getGradient(senderName()));

  const handleClick = (e: MouseEvent) => {
    if (isSwiping()) {
      e.stopPropagation();
      e.preventDefault();
    }
  };

  const handleContextMenu = (e: MouseEvent) => {
    e.preventDefault();
    if (!props.isSelectionMode) {
      setContextMenu({ x: e.clientX, y: e.clientY, email: props.email });
    }
  };

  const handleCheckboxClick = (e: MouseEvent) => {
    e.stopPropagation();
    if (props.onToggleSelect && props.email.id)
      props.onToggleSelect(props.email.id.toString());
  };

  const handleStarClick = async (e: MouseEvent) => {
    e.stopPropagation();
    const action = isStarred() ? "unstar" : "star";
    appEvents.emit("email:action", { uid: props.email.uid, action });
    try {
      await EmailApi.updateState(
        props.email.account_id,
        props.email.mailbox_name,
        props.email.uid,
        action
      );
    } catch (err) {
      if (import.meta.env.DEV) console.error(err);
    }
  };

  return (
    <div class="relative overflow-hidden" style={props.style}>
      <div class="absolute inset-0 bg-red-500 flex items-center justify-end px-6 text-white font-medium">
        <Trash2 size={20} class="mr-2" /> Delete
      </div>
      <div
        class={`group flex items-center px-5 py-3.5 border-b border-surface-100 dark:border-surface-800/50 cursor-pointer transition-all duration-200 h-[84px] overflow-hidden border-l-2 ${
          isFocused()
            ? "bg-surface-100 dark:bg-surface-800 ring-2 ring-inset ring-brand-500/80 z-10 border-l-brand-600"
            : props.isSelected || props.isReading
            ? "bg-surface-50 dark:bg-surface-900 border-l-brand-500"
            : "bg-white dark:bg-surface-950 hover:bg-surface-50 dark:hover:bg-surface-900 border-l-transparent"
        }`}
        style={{
          transform: `translateX(${offset()}px)`,
          // Allows vertical scrolling on touch devices while intercepting horizontal swipes for the delete gesture.
          "touch-action": "pan-y",
          transition: isDragging() ? "none" : "transform 0.2s ease-out",
        }}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerCancel={handlePointerUp}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
      >
        <Show when={props.isSelectionMode}>
          <div
            class="w-6 flex-shrink-0 mr-2 flex items-center justify-center"
            onClick={handleCheckboxClick}
          >
            <Show
              when={props.checked}
              fallback={<Square size={18} class="text-surface-400" />}
            >
              <CheckSquare size={18} class="text-brand-500" />
            </Show>
          </div>
        </Show>

        <div class="w-2 flex-shrink-0 mr-2">
          <Show when={!isRead() && !props.isSelectionMode}>
            <div class="w-1.5 h-1.5 rounded-full bg-brand-500 shadow-glow" />
          </Show>
        </div>

        <div
          class={`w-9 h-9 rounded-full bg-gradient-to-br ${avatarGradient()} flex items-center justify-center text-white text-sm font-medium shadow-soft flex-shrink-0 mr-4`}
        >
          {senderName().charAt(0).toUpperCase()}
        </div>

        <div class="flex-1 flex flex-col min-w-0 mr-4">
          <div class="flex items-center justify-between mb-0.5">
            <span
              class={`truncate text-sm ${
                !isRead()
                  ? "font-semibold text-surface-900 dark:text-surface-50"
                  : "font-medium text-surface-600 dark:text-surface-300"
              }`}
            >
              {senderName()}
            </span>
            <div class="flex items-center gap-2 flex-shrink-0 ml-2">
              <button
                onClick={handleStarClick}
                class={`p-1 rounded hover:bg-surface-200 dark:hover:bg-surface-700 transition-colors ${
                  isStarred()
                    ? "text-amber-400"
                    : "text-surface-300 dark:text-surface-600 opacity-0 group-hover:opacity-100"
                }`}
              >
                <Star size={14} class={isStarred() ? "fill-amber-400" : ""} />
              </button>
              <span class="text-xs text-surface-400 dark:text-surface-500 tabular-nums whitespace-nowrap">
                {formattedDate()}
              </span>
            </div>
          </div>
          <div class="flex items-center gap-2 mb-1">
            <span
              class={`truncate text-sm ${
                !isRead()
                  ? "font-medium text-surface-800 dark:text-surface-100"
                  : "text-surface-600 dark:text-surface-400"
              }`}
            >
              {props.email.subject || "(No Subject)"}
            </span>
            <Show when={(props.email.thread_count || 0) > 1}>
              <span class="px-1.5 py-0.5 text-[10px] font-bold bg-brand-500/10 text-brand-600 dark:text-brand-400 rounded-full flex-shrink-0">
                {props.email.thread_count}
              </span>
            </Show>
            <Show when={props.email.has_attachments}>
              <Paperclip size={12} class="text-surface-400 flex-shrink-0" />
            </Show>
          </div>
          <div class="snippet-fade text-xs text-surface-500 dark:text-surface-500 leading-relaxed line-clamp-1">
            {props.email.snippet && props.email.snippet.trim() !== ""
              ? props.email.snippet
              : "\u00A0"}
          </div>
        </div>
      </div>
    </div>
  );
};

export default EmailListItem;
