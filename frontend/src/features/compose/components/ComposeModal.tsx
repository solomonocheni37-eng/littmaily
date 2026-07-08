/**
 * Rich-text email composition modal using Quill.
 * Quill is initialized imperatively inside `createEffect` because its direct DOM
 * manipulation conflicts with SolidJS's fine-grained reactivity if mounted declaratively.
 */
import { createSignal, Show, For, createEffect, onCleanup } from "solid-js";
import { Portal } from "solid-js/web";
import { useAppContext } from "@/core/store/AppStore";
import { X, Send, Loader2, Paperclip } from "lucide-solid";
import { useComposeDraft } from "../hooks/useComposeDraft";
import { useComposeSend } from "../hooks/useComposeSend";
import Quill from "quill";
import "quill/dist/quill.snow.css";

export default function ComposeModal() {
  const { state, setShowCompose } = useAppContext();

  const [to, setTo] = createSignal<string[]>([]);
  const [cc, setCc] = createSignal<string[]>([]);
  const [bcc, setBcc] = createSignal<string[]>([]);
  const [subject, setSubject] = createSignal("");

  // Split body into text and html to support multipart/alternative MIME construction
  const [textBody, setTextBody] = createSignal("");
  const [htmlBody, setHtmlBody] = createSignal("");

  const [attachments, setAttachments] = createSignal<File[]>([]);

  let editorRef: HTMLDivElement | undefined;
  let quillInstance: Quill | undefined;

  const { draftId, setDraftId } = useComposeDraft(
    to,
    cc,
    bcc,
    subject,
    textBody,
    htmlBody
  );

  const resetForm = () => {
    setTo([]);
    setCc([]);
    setBcc([]);
    setSubject("");
    setTextBody("");
    setHtmlBody("");
    setAttachments([]);
    setDraftId(null);
    if (quillInstance) quillInstance.setText("");
  };

  const { loading, handleSend } = useComposeSend(
    to,
    cc,
    bcc,
    subject,
    textBody,
    htmlBody,
    attachments,
    draftId,
    resetForm
  );

  // Initialize Quill when modal opens
  createEffect(() => {
    if (state.showCompose && editorRef && !quillInstance) {
      const q = new Quill(editorRef, {
        theme: "snow",
        modules: {
          toolbar: [
            ["bold", "italic", "underline", "strike"],
            [{ list: "ordered" }, { list: "bullet" }],
            ["blockquote", "code-block", "link"],
            [{ align: [] }],
            ["clean"],
          ],
        },
        placeholder: "Write your message...",
      });

      q.on("text-change", () => {
        setTextBody(q.getText().trim());
        setHtmlBody(q.root.innerHTML);
      });

      // Handle Reply/Forward payloads
      const payload = state.composePayload;
      if (payload && payload.email) {
        const email = payload.email;
        const sender = email.sender || "";
        const subj = email.subject || "";
        let html = "";

        if (payload.type === "reply") {
          html = `<p><br></p><blockquote><p>--- Original Message ---</p><p>From: ${sender}</p></blockquote>`;
        } else if (payload.type === "forward") {
          html = `<p><br></p><blockquote><p>--- Forwarded Message ---</p><p>From: ${sender}</p><p>Subject: ${subj}</p></blockquote>`;
        }

        if (html) {
          q.root.innerHTML = html;
          // Place cursor exactly at the top of the editable area, above the quoted text
          q.setSelection(1, 0);
        }
      }

      quillInstance = q;

      onCleanup(() => {
        quillInstance = undefined;
      });
    }
  });

  // Handle Contact-Email payload (prefill To)
  createEffect(() => {
    const payload = state.composePayload;
    if (
      state.showCompose &&
      payload?.type === "new" &&
      payload.to &&
      payload.to.length > 0
    ) {
      setTo(payload.to);
    }
  });

  const handleFileSelect = async (e: Event) => {
    const files = (e.target as HTMLInputElement).files;
    if (files) setAttachments((prev) => [...prev, ...Array.from(files)]);
  };

  return (
    <Show when={state.showCompose}>
      <Portal>
        <div
          class="fixed inset-0 bg-black/40 flex items-center justify-center z-50 p-4"
          onClick={() => setShowCompose(false)}
        >
          <div
            class="bg-white dark:bg-surface-900 rounded-xl shadow-2xl w-full max-w-2xl h-[600px] flex flex-col border border-surface-200 dark:border-surface-800"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header & Inputs (Remains identical to your original code) */}
            <div class="flex justify-between items-center p-4 border-b border-surface-200 dark:border-surface-800">
              <h2 class="font-semibold capitalize text-surface-900 dark:text-surface-50">
                {state.composePayload?.type || "New"} Message
              </h2>
              <button
                onClick={() => setShowCompose(false)}
                class="text-surface-500 hover:text-surface-900 dark:hover:text-white"
              >
                <X size={20} />
              </button>
            </div>

            {/* ... (Keep your existing To/Cc/Bcc/Subject inputs here) ... */}

            {/* NEW: Quill Editor Container */}
            <div class="flex-1 overflow-hidden flex flex-col">
              <div
                ref={editorRef}
                class="flex-1 overflow-y-auto bg-transparent text-surface-900 dark:text-surface-50"
              ></div>
            </div>

            {/* Attachments & Footer (Remains identical) */}
            <Show when={attachments().length > 0}>
              <div class="px-4 py-2 border-t border-surface-200 dark:border-surface-800 flex flex-wrap gap-2">
                <For each={attachments()}>
                  {(file, index) => (
                    <div class="flex items-center gap-2 bg-surface-100 dark:bg-surface-800 px-3 py-1.5 rounded-full text-xs text-surface-700 dark:text-surface-300">
                      <Paperclip size={12} />
                      <span class="truncate max-w-[120px]">{file.name}</span>
                      <button
                        onClick={() =>
                          setAttachments((prev) =>
                            prev.filter((_, i) => i !== index())
                          )
                        }
                        class="hover:text-red-500"
                      >
                        <X size={12} />
                      </button>
                    </div>
                  )}
                </For>
              </div>
            </Show>
            <div class="p-4 border-t border-surface-200 dark:border-surface-800 flex justify-between items-center">
              <label class="cursor-pointer p-2 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg text-surface-500">
                <Paperclip size={18} />
                <input
                  type="file"
                  multiple
                  class="hidden"
                  onChange={handleFileSelect}
                />
              </label>
              <button
                onClick={handleSend}
                disabled={loading() || to().length === 0 || !subject()}
                class="px-6 py-2 bg-brand-500 hover:bg-brand-600 text-white rounded-lg font-medium flex items-center gap-2 disabled:opacity-50"
              >
                {loading() ? (
                  <Loader2 class="animate-spin" size={16} />
                ) : (
                  <>
                    <Send size={16} /> Send
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      </Portal>
    </Show>
  );
}
