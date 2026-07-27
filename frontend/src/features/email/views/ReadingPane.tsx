import { Show, For, createEffect, createSignal } from "solid-js";
import { useAppContext } from "@/core/store/AppStore";
import { confirm } from "@tauri-apps/plugin-dialog";
import {
  Paperclip,
  Download,
  Loader2,
  MailQuestion,
  Reply,
  Forward,
  Trash2,
  ShieldAlert,
  MailMinus,
  Archive,
  ZoomIn,
  ZoomOut,
  Maximize2,
  Hand,
  MousePointer2,
} from "lucide-solid";
import { toast } from "@/core/ui/toast";
import { EmailApi } from "@/core/ipc";
import { useEmailViewer } from "../hooks/useEmailViewer";
import { isExecutable, getSafeFilename } from "../utils/attachmentSecurity";
import { appEvents } from "@/core/events/eventBus";

const ReadingPane = () => {
  const { openCompose, selectEmail } = useAppContext();
  const {
    state,
    emailBody,
    refetch,
    imagesLoaded,
    renderedHtml,
    iframeRef,
    srcdoc,
    triggerLoadRemoteImages,
    zoom,
    setZoom,
    panMode,
    setPanMode,
  } = useEmailViewer();

  let scrollContainerRef: HTMLDivElement | undefined;

  createEffect(() => {
    state.selectedEmail?.uid;
    if (scrollContainerRef) {
      scrollContainerRef.scrollTop = 0;
    }
  });

  const downloadAttachment = async (
    blobHash: string,
    rawFilename: string,
    mimeType: string
  ) => {
    const filename = getSafeFilename(rawFilename, mimeType, blobHash);
    const displayName =
      filename.length > 40
        ? `${filename.substring(0, 20)}...${filename.substring(
            filename.length - 15
          )}`
        : filename;

    if (isExecutable(filename)) {
      const isConfirmed = await confirm(
        `The file "${displayName}" is an executable or script.\nRunning untrusted files from emails is dangerous.\nAre you absolutely sure?`,
        {
          title: "Security Warning",
          okLabel: "Download Anyway",
          cancelLabel: "Cancel",
        }
      );
      if (!isConfirmed) return;
    }

    try {
      const saved = await EmailApi.saveAttachmentDialog(blobHash, filename);
      if (saved) toast("Attachment saved successfully!");
    } catch (e) {
      if (import.meta.env.DEV) console.error(e);
      toast("Failed to save attachment");
    }
  };

  const handleDelete = async () => {
    if (!state.selectedEmail) return;
    const uid = state.selectedEmail.uid;
    appEvents.emit("email:action", { uid, action: "delete" });
    try {
      await EmailApi.updateState(
        state.selectedEmail.account_id,
        state.selectedEmail.mailbox_name,
        uid,
        "delete"
      );
      appEvents.emit("mailboxes:refresh");
      selectEmail(null);
      toast("Message deleted");
    } catch (e) {
      if (import.meta.env.DEV) console.error(e);
    }
  };

  const handleArchive = async () => {
    if (!state.selectedEmail) return;
    const uid = state.selectedEmail.uid;
    appEvents.emit("email:action", { uid, action: "archive" });
    try {
      await EmailApi.updateState(
        state.selectedEmail.account_id,
        state.selectedEmail.mailbox_name,
        uid,
        "archive"
      );
      appEvents.emit("mailboxes:refresh");
      selectEmail(null);
      toast("Message archived");
    } catch (e) {
      if (import.meta.env.DEV) console.error(e);
    }
  };

  return (
    <div class="h-full flex flex-col bg-surface-50 dark:bg-surface-950 overflow-hidden">
      <Show
        when={state.selectedEmail}
        fallback={
          <div class="h-full flex flex-col items-center justify-center text-surface-400 p-8">
            <MailQuestion size={48} class="opacity-50 mb-4" />
            <h3 class="text-xl font-semibold">No Message Selected</h3>
          </div>
        }
      >
        <div class="p-6 border-b border-surface-200 dark:border-surface-800 flex-shrink-0 bg-white dark:bg-surface-900">
          <div class="flex justify-between items-start mb-4">
            <h1 class="text-2xl font-bold text-surface-900 dark:text-surface-50">
              {state.selectedEmail!.subject || "(No Subject)"}
            </h1>
            <div class="flex gap-2 items-center">
              <button
                onClick={() =>
                  openCompose({ type: "reply", email: state.selectedEmail! })
                }
                class="p-2 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg text-surface-600 dark:text-surface-300"
                title="Reply"
              >
                <Reply size={18} />
              </button>
              <button
                onClick={() =>
                  openCompose({ type: "forward", email: state.selectedEmail! })
                }
                class="p-2 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg text-surface-600 dark:text-surface-300"
                title="Forward"
              >
                <Forward size={18} />
              </button>
              <button
                onClick={handleArchive}
                class="p-2 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg text-surface-600 dark:text-surface-300"
                title="Archive"
              >
                <Archive size={18} />
              </button>
              <button
                onClick={() => {
                  const uid = state.selectedEmail!.uid;
                  appEvents.emit("email:action", { uid, action: "unread" });
                  EmailApi.updateState(
                    state.selectedEmail!.account_id,
                    state.selectedEmail!.mailbox_name,
                    uid,
                    "unread"
                  )
                    .then(() => {
                      appEvents.emit("mailboxes:refresh");
                    })
                    .catch((e) => {
                      if (import.meta.env.DEV) console.error(e);
                    });
                }}
                class="p-2 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg text-surface-600 dark:text-surface-300"
                title="Mark as Unread"
              >
                <MailMinus size={18} />
              </button>
              <button
                onClick={handleDelete}
                class="p-2 hover:bg-red-500/10 text-red-500 rounded-lg"
                title="Delete"
              >
                <Trash2 size={18} />
              </button>
              <div class="flex items-center gap-1 border-l border-surface-200 dark:border-surface-800 pl-2 ml-2">
                <button
                  onClick={() => setZoom((z) => Math.max(50, z - 10))}
                  class="p-1.5 hover:bg-surface-200 dark:hover:bg-surface-800 rounded text-surface-600 dark:text-surface-300 transition-colors"
                  title="Zoom Out"
                >
                  <ZoomOut size={16} />
                </button>
                <span class="text-xs w-10 text-center text-surface-500 dark:text-surface-400 font-medium tabular-nums">
                  {zoom()}%
                </span>
                <button
                  onClick={() => setZoom((z) => Math.min(200, z + 10))}
                  class="p-1.5 hover:bg-surface-200 dark:hover:bg-surface-800 rounded text-surface-600 dark:text-surface-300 transition-colors"
                  title="Zoom In"
                >
                  <ZoomIn size={16} />
                </button>
                <button
                  onClick={() => setZoom(100)}
                  class="p-1.5 hover:bg-surface-200 dark:hover:bg-surface-800 rounded text-surface-600 dark:text-surface-300 transition-colors ml-1"
                  title="Reset Zoom"
                >
                  <Maximize2 size={14} />
                </button>
              </div>
              <div class="flex items-center border-l border-surface-200 dark:border-surface-800 pl-2 ml-2">
                <button
                  onClick={() => setPanMode((p) => !p)}
                  class={`p-1.5 rounded transition-colors ${
                    panMode()
                      ? "bg-brand-500 text-white shadow-sm"
                      : "hover:bg-surface-200 dark:hover:bg-surface-800 text-surface-600 dark:text-surface-300"
                  }`}
                  title={
                    panMode()
                      ? "Interaction Mode (Select Text)"
                      : "Hand Tool (Drag to Pan)"
                  }
                >
                  {panMode() ? <Hand size={16} /> : <MousePointer2 size={16} />}
                </button>
              </div>
            </div>
          </div>
          <div class="text-sm text-surface-600 dark:text-surface-400">
            From:{" "}
            <span class="font-semibold">{state.selectedEmail!.sender}</span>
          </div>
          <div class="text-xs text-surface-500 mt-1">
            {state.selectedEmail!.date}
          </div>
        </div>

        <div ref={scrollContainerRef} class="flex-1 overflow-y-auto overflow-x-hidden overscroll-contain">
          <Show
            when={emailBody.state === "ready"}
            fallback={
              <Show
                when={emailBody.state === "errored"}
                fallback={
                  <div class="flex flex-col items-center justify-center h-full gap-2 text-surface-500">
                    <Loader2 class="animate-spin" />
                    <p class="text-sm">Loading body...</p>
                  </div>
                }
              >
                <div class="flex flex-col items-center justify-center h-full gap-3 text-red-500 p-8 text-center">
                  <ShieldAlert size={40} class="opacity-80" />
                  <h3 class="text-lg font-semibold">Failed to load message</h3>
                  <p class="text-sm text-surface-500 dark:text-surface-400 max-w-sm break-words">
                    {(emailBody.error as any)?.message ||
                      String(
                        emailBody.error ||
                          "An unknown error occurred while fetching the email body."
                      )}
                  </p>
                  <button
                    onClick={() => refetch()}
                    class="mt-2 px-4 py-2 bg-surface-200 dark:bg-surface-800 hover:bg-surface-300 dark:hover:bg-surface-700 text-surface-800 dark:text-surface-200 text-sm font-medium rounded-lg transition-colors"
                  >
                    Retry
                  </button>
                </div>
              </Show>
            }
          >
            <div class="max-w-4xl mx-auto p-6">
              <Show
                when={!imagesLoaded() && renderedHtml()?.includes("data-src")}
              >
                <div class="mb-4 p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg flex items-center justify-between shadow-sm">
                  <span class="text-sm text-amber-800 dark:text-amber-200 flex items-center gap-2">
                    <ShieldAlert size={16} /> Remote images blocked to protect
                    your privacy.
                  </span>
                  <button
                    onClick={triggerLoadRemoteImages}
                    class="text-sm font-semibold text-amber-600 dark:text-amber-400 hover:underline"
                  >
                    Load Images
                  </button>
                </div>
              </Show>

              <Show
                when={renderedHtml()}
                fallback={
                  <pre class="whitespace-pre-wrap font-sans text-sm bg-white dark:bg-surface-900 p-4 rounded-lg border border-surface-200 dark:border-surface-800 text-surface-800 dark:text-surface-200">
                    {emailBody()!.text_body}
                  </pre>
                }
              >
                <iframe
                  ref={iframeRef}
                  sandbox="allow-scripts"
                  class="w-full min-h-[400px] bg-transparent rounded-lg shadow-sm border border-surface-200 dark:border-surface-800 transition-[height] duration-300 ease-in-out"
                  srcdoc={srcdoc()}
                  title="Email Content"
                />
              </Show>

              <Show when={emailBody()!.attachments.length > 0}>
                <div class="mt-8 pt-6 border-t border-surface-200 dark:border-surface-800">
                  <h3 class="text-sm font-semibold mb-3 flex items-center gap-2 text-surface-700 dark:text-surface-300">
                    <Paperclip size={16} /> Attachments (
                    {emailBody()!.attachments.length})
                  </h3>
                  <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <For each={emailBody()!.attachments}>
                      {(att) => {
                        const [isDownloading, setIsDownloading] = createSignal(false);
                        const [localHash, setLocalHash] = createSignal(att.blob_hash);

                        const handleLazyDownload = async () => {
                          if (localHash()) {
                            await downloadAttachment(localHash()!, att.filename || "attachment", att.mime_type);
                            return;
                          }
                          setIsDownloading(true);
                          try {
                            const hash = await EmailApi.fetchAttachment(
                              state.selectedEmail!.account_id,
                              state.selectedEmail!.mailbox_name,
                              state.selectedEmail!.uid,
                              att.section_id
                            );
                            setLocalHash(hash);
                            await downloadAttachment(hash, att.filename || "attachment", att.mime_type);
                          } catch (e) {
                            toast("Failed to download attachment");
                          } finally {
                            setIsDownloading(false);
                          }
                        };

                        return (
                          <button
                            onClick={handleLazyDownload}
                            class="flex items-center gap-3 p-3 rounded-lg border border-surface-200 dark:border-surface-700 hover:bg-surface-100 dark:hover:bg-surface-800 text-left transition-colors"
                          >
                            <div class="w-10 h-10 rounded bg-brand-500/10 flex items-center justify-center text-brand-500">
                              {isDownloading() ? <Loader2 size={20} class="animate-spin" /> : <Download size={20} />}
                            </div>
                            <div class="min-w-0 flex-1">
                              <div class="text-sm font-medium truncate text-surface-800 dark:text-surface-200">
                                {att.filename || "Unknown"}
                              </div>
                              <div class="text-xs text-surface-500">
                                {localHash() ? "Click to save" : `Click to download (${((att.size ?? 0) / 1024).toFixed(1)} KB)`}
                              </div>
                            </div>
                          </button>
                        );
                      }}
                    </For>
                  </div>
                </div>
              </Show>
            </div>
          </Show>
        </div>
      </Show>
    </div>
  );
};

export default ReadingPane;