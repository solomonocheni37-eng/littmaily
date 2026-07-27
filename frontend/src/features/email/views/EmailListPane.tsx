// FILE: ./frontend/src/features/email/views/EmailListPane.tsx
import {
For,
createEffect,
Show,
onMount,
onCleanup,
untrack,
createSignal,
} from "solid-js";
import { createVirtualizer } from "@tanstack/solid-virtual";
import type { Message } from "@/core/types/generated";
import { commands } from "@/core/types/generated";
import { EmailApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import EmailListItem from "@/features/email/components/EmailListItem";
import {
Inbox,
RefreshCw,
Loader2,
PanelLeftClose,
CheckSquare,
Trash2,
Archive,
MailOpen,
MailMinus,
} from "lucide-solid";
import { toast } from "@/core/ui/toast";
import { useEmailPagination } from "../hooks/useEmailPagination";
import { useViewportSnippets } from "../hooks/useViewportSnippets";
import { useListKeyboardNav } from "../hooks/useListKeyboardNav";
import { appEvents } from "@/core/events/eventBus";
import { parseFlags, serializeFlags } from "../utils/flagUtils";

const EmailListPane = () => {
let scrollElementRef!: HTMLDivElement;
const { state, selectEmail, toggleListPane, setFocusedUid } = useAppContext();
const { emails, setEmails, hasMore, isLoading, isSwitching, fetchPage } =
useEmailPagination();
const [isSelectionMode, setIsSelectionMode] = createSignal(false);
const [selectedIds, setSelectedIds] = createSignal<Set<string>>(
new Set<string>()
);

const virtualizer = createVirtualizer({
get count() {
return emails().length + (hasMore() || isLoading() ? 1 : 0);
},
getScrollElement: () => scrollElementRef,
estimateSize: (index) => (index === emails().length ? 60 : 84),
overscan: 50,
});

useViewportSnippets(emails, setEmails, virtualizer);
useListKeyboardNav(emails, hasMore, isLoading, fetchPage);

createEffect(() => {
const currentEmails = emails();
if (currentEmails.length > 0 && state.focusedUid === null) {
setFocusedUid(currentEmails[0].uid);
}
});

createEffect(() => {
const items = virtualizer.getVirtualItems();
if (items.length > 0 && hasMore() && !isLoading()) {
const lastItem = items[items.length - 1];
if (lastItem.index >= emails().length - 40) {
untrack(() => fetchPage());
}
}
});

const toggleSelectAll = () => {
const allIds = new Set<string>(
emails()
.map((e) => e.id?.toString())
.filter((id): id is string => Boolean(id))
);
if (selectedIds().size === allIds.size) {
setSelectedIds(new Set<string>());
} else {
setSelectedIds(allIds);
}
};

const toggleSelect = (id: string) => {
const newSet = new Set(selectedIds());
if (newSet.has(id)) {
newSet.delete(id);
} else {
newSet.add(id);
}
setSelectedIds(newSet);
if (newSet.size === 0) {
setIsSelectionMode(false);
}
};

const clearSelection = () => {
setSelectedIds(new Set<string>());
setIsSelectionMode(false);
};

const getSelectedEmails = () => {
return emails().filter((e) => e.id && selectedIds().has(e.id.toString()));
};

const handleBulkAction = async (action: string, destMailbox?: string) => {
const selected = getSelectedEmails();
if (selected.length === 0) return;

const promises = selected.map((email) =>
EmailApi.updateState(
email.account_id,
email.mailbox_name,
email.uid,
action as any,
destMailbox
).catch((e) => {
if (import.meta.env.DEV) console.error(e);
})
);

await Promise.all(promises);

setEmails((prev) => {
if (action === "delete" || action === "move" || action === "archive") {
return prev.filter((e) => !selectedIds().has(e.id?.toString() || ""));
}
return prev.map((e) => {
if (selectedIds().has(e.id?.toString() || "")) {
const flags = parseFlags(e.flags);
if (action === "read" && !flags.includes("Seen")) flags.push("Seen");
if (action === "unread") {
const idx = flags.indexOf("Seen");
if (idx > -1) flags.splice(idx, 1);
}
if (action === "star" && !flags.includes("Flagged"))
flags.push("Flagged");
if (action === "unstar") {
const idx = flags.indexOf("Flagged");
if (idx > -1) flags.splice(idx, 1);
}
return { ...e, flags: serializeFlags(flags) };
}
return e;
});
});

toast(
`${action.charAt(0).toUpperCase() + action.slice(1)} ${
selected.length
} email${selected.length > 1 ? "s" : ""}`
);
clearSelection();
appEvents.emit("mailboxes:refresh");
try {
await commands.updateBadgeCount();
} catch (e) {
if (import.meta.env.DEV) console.error(e);
}
};

const handleEmailAction = (payload: {
uid: number;
action: string;
destMailbox?: string;
}) => {
const { uid, action } = payload;
if (action === "delete" || action === "move" || action === "archive") {
setEmails((prev) => prev.filter((em) => em.uid !== uid));
if (state.selectedEmail?.uid === uid) selectEmail(null);
if (state.focusedUid === uid) {
const remaining = emails();
if (remaining.length > 0) setFocusedUid(remaining[0].uid);
else setFocusedUid(null);
}
} else if (["read", "unread", "star", "unstar"].includes(action)) {
setEmails((prev) =>
prev.map((em) => {
if (em.uid === uid) {
const flags = parseFlags(em.flags);
if (action === "read" && !flags.includes("Seen"))
flags.push("Seen");
if (action === "unread") {
const idx = flags.indexOf("Seen");
if (idx > -1) flags.splice(idx, 1);
}
if (action === "star" && !flags.includes("Flagged"))
flags.push("Flagged");
if (action === "unstar") {
const idx = flags.indexOf("Flagged");
if (idx > -1) flags.splice(idx, 1);
}
return { ...em, flags: serializeFlags(flags) };
}
return em;
})
);
}
};

const handleManualRefresh = async () => {
if (!state.selectedAccountId || !state.selectedMailboxName) return;
if (state.selectedMailboxName.startsWith("__")) {
untrack(() => fetchPage(true));
toast("Refreshed");
return;
}
try {
// Only hit the network when the user manually requests a sync
const count = await EmailApi.checkForNew(
state.selectedAccountId,
state.selectedMailboxName
);
if (count > 0) {
toast(`Synced ${count} new email${count > 1 ? "s" : ""}`);
untrack(() => fetchPage(true));
} else {
toast("Inbox is up to date");
}
} catch (e) {
if (import.meta.env.DEV) console.error(e);
}
};

const handleBackgroundRefresh = () => {
// The Rust SyncWorker already fetched and inserted the new emails.
// Instantly reload from the local DB without hitting the network.
untrack(() => fetchPage(true));
};

let cleanupAction: (() => void) | undefined;
let cleanupRefresh: (() => void) | undefined;

onMount(() => {
cleanupAction = appEvents.on("email:action", handleEmailAction);
// Bind to the fast local-only refresh
cleanupRefresh = appEvents.on("inbox:refresh", handleBackgroundRefresh);
});

onCleanup(() => {
cleanupAction?.();
cleanupRefresh?.();
});

const handleEmailClick = async (email: Message) => {
if (isSelectionMode()) {
if (email.id) toggleSelect(email.id.toString());
return;
}

// CRITICAL FIX: If the user clicks an already open email, force a refetch/reopen
if (
state.selectedEmail &&
state.selectedEmail.uid === email.uid &&
state.selectedEmail.account_id === email.account_id &&
state.selectedEmail.mailbox_name === email.mailbox_name
) {
appEvents.emit("email:reopen", { uid: email.uid });
return;
}

setFocusedUid(email.uid);
selectEmail(email);

let isRead = false;
try {
const flags = JSON.parse(email.flags || "[]");
isRead = flags.includes("Seen");
} catch {
isRead = email.flags.includes("Seen");
}

if (!isRead) {
try {
appEvents.emit("email:action", { uid: email.uid, action: "read" });
await EmailApi.updateState(
email.account_id,
email.mailbox_name,
email.uid,
"read"
);
appEvents.emit("mailboxes:refresh");
} catch (e) {
if (import.meta.env.DEV) console.error(e);
}
}
};

return (
<div class="h-full flex flex-col bg-surface-0 dark:bg-surface-950">
<div class="px-4 py-3 border-b border-surface-200 dark:border-surface-800 flex items-center justify-between bg-surface-50 dark:bg-surface-900 flex-shrink-0 gap-3">
<h2
class="text-lg font-semibold text-surface-900 dark:text-surface-50 truncate min-w-0"
title={state.selectedMailboxName || "Inbox"}
>
{state.selectedMailboxName || "Inbox"}
</h2>
<div class="flex items-center gap-2 flex-shrink-0">
<button
onClick={() => {
if (isSelectionMode()) {
clearSelection();
} else {
setIsSelectionMode(true);
}
}}
class={`p-1.5 rounded-md transition-colors ${
isSelectionMode()
? "bg-brand-500 text-white"
: "hover:bg-surface-200 dark:hover:bg-surface-800 text-surface-500"
}`}
title={isSelectionMode() ? "Cancel Selection" : "Select Emails"}
>
<CheckSquare size={16} />
</button>
<button
onClick={handleManualRefresh}
class="p-1.5 hover:bg-surface-200 dark:hover:bg-surface-800 rounded-md text-surface-500 transition-colors"
title="Sync Now"
>
<RefreshCw size={16} class={isLoading() ? "animate-spin" : ""} />
</button>
<button
onClick={toggleListPane}
class="p-1.5 hover:bg-surface-200 dark:hover:bg-surface-800 rounded-md text-surface-500 transition-colors"
title="Collapse List"
>
<PanelLeftClose size={16} />
</button>
</div>
</div>

<Show when={isSelectionMode() && selectedIds().size > 0}>
<div class="px-4 py-2 bg-brand-50 dark:bg-brand-900/20 border-b border-brand-200 dark:border-brand-800 flex items-center justify-between gap-3 flex-shrink-0">
<div class="flex items-center gap-2">
<button
onClick={toggleSelectAll}
class="px-2 py-1 text-xs font-medium text-brand-700 dark:text-brand-300 hover:bg-brand-100 dark:hover:bg-brand-800 rounded transition-colors"
>
{selectedIds().size === emails().length
? "Deselect All"
: "Select All"}
</button>
<span class="text-sm font-medium text-brand-800 dark:text-brand-200">
{selectedIds().size} selected
</span>
</div>
<div class="flex items-center gap-1">
<button
onClick={() => handleBulkAction("read")}
class="p-1.5 hover:bg-brand-100 dark:hover:bg-brand-800 rounded text-brand-600 dark:text-brand-400 transition-colors"
title="Mark as Read"
>
<MailOpen size={16} />
</button>
<button
onClick={() => handleBulkAction("unread")}
class="p-1.5 hover:bg-brand-100 dark:hover:bg-brand-800 rounded text-brand-600 dark:text-brand-400 transition-colors"
title="Mark as Unread"
>
<MailMinus size={16} />
</button>
<button
onClick={() => handleBulkAction("archive")}
class="p-1.5 hover:bg-brand-100 dark:hover:bg-brand-800 rounded text-brand-600 dark:text-brand-400 transition-colors"
title="Archive"
>
<Archive size={16} />
</button>
<button
onClick={() => handleBulkAction("delete")}
class="p-1.5 hover:bg-red-100 dark:hover:bg-red-900/30 rounded text-red-600 dark:text-red-400 transition-colors"
title="Delete"
>
<Trash2 size={16} />
</button>
</div>
</div>
</Show>

<div
ref={scrollElementRef}
class="flex-1 overflow-auto relative"
style={{ "overflow-anchor": "none" }}
>
<Show
when={!isLoading() && !isSwitching() && emails().length === 0}
fallback={
<div
style={{
height: `${virtualizer.getTotalSize()}px`,
width: "100%",
position: "relative",
}}
>
<For each={virtualizer.getVirtualItems()}>
{(virtualRow) => {
const isSentinel = virtualRow.index === emails().length;
if (isSentinel) {
return (
<div
class="virtual-row"
style={{
position: "absolute",
top: "0",
left: "0",
width: "100%",
height: "60px",
transform: `translateY(${virtualRow.start}px)`,
}}
classList={{
"flex items-center justify-center text-surface-500 dark:text-surface-400 text-sm": true,
}}
>
{isLoading() ? (
<div class="flex items-center gap-2">
<Loader2 size={16} class="animate-spin" />
<span>
{emails().length > 0
? "Loading older emails..."
: "Syncing inbox..."}
</span>
</div>
) : hasMore() ? (
<span class="opacity-0">Sentinel</span>
) : null}
</div>
);
}

return (
<div
class="virtual-row"
onClick={() => {
const em = emails()[virtualRow.index];
if (em) handleEmailClick(em);
}}
style={{
position: "absolute",
top: "0",
left: "0",
width: "100%",
transform: `translateY(${virtualRow.start}px)`,
}}
>
<Show when={emails()[virtualRow.index]}>
<EmailListItem
email={emails()[virtualRow.index]!}
isSelected={
state.selectedEmail?.uid ===
emails()[virtualRow.index]!.uid
}
isSelectionMode={isSelectionMode()}
isReading={
state.selectedEmail?.uid ===
emails()[virtualRow.index]!.uid
}
onToggleSelect={(id) => toggleSelect(id)}
checked={
emails()[virtualRow.index]!.id
? selectedIds().has(
emails()[virtualRow.index]!.id!.toString()
)
: false
}
/>
</Show>
</div>
);
}}
</For>
</div>
}
>
<div class="h-full flex flex-col items-center justify-center text-surface-400 dark:text-surface-600 p-8">
<div class="w-20 h-20 rounded-full bg-surface-100 dark:bg-surface-800 flex items-center justify-center mb-4 shadow-inner">
<Inbox size={40} class="opacity-50" />
</div>
<h3 class="text-lg font-medium text-surface-700 dark:text-surface-300 mb-1">
Inbox Zero
</h3>
<p class="text-sm text-center">
You're all caught up! No emails to display.
</p>
</div>
</Show>
</div>
</div>
);
};

export default EmailListPane;