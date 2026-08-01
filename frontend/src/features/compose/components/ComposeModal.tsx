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
  const [textBody, setTextBody] = createSignal("");
  const [htmlBody, setHtmlBody] = createSignal("");
  const [attachments, setAttachments] = createSignal<File[]>([]);
  const [showCc, setShowCc] = createSignal(false);
  const [showBcc, setShowBcc] = createSignal(false);

  let editorRef: HTMLDivElement | undefined;
  let quillInstance: Quill | undefined;

  const { draftId, setDraftId } = useComposeDraft(
    to, cc, bcc, subject, textBody, htmlBody
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
    setShowCc(false);
    setShowBcc(false);
    if (quillInstance) quillInstance.setText("");
  };

  const { loading, handleSend } = useComposeSend(
    to, cc, bcc, subject, textBody, htmlBody, attachments, draftId, resetForm
  );

  const extractEmail = (senderStr: string) => {
    if (!senderStr) return "";
    const match = senderStr.match(/<(.+?)>/);
    return match ? match[1] : senderStr.trim();
  };

  const handleTagInput = (
    e: KeyboardEvent,
    getter: () => string[],
    setter: (v: string[]) => void
  ) => {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      const val = (e.target as HTMLInputElement).value.trim().replace(/,$/, "");
      if (val && !getter().includes(val)) {
        setter([...getter(), val]);
      }
      (e.target as HTMLInputElement).value = "";
    }
  };

  const removeTag = (
    tag: string,
    getter: () => string[],
    setter: (v: string[]) => void
  ) => {
    setter(getter().filter((t) => t !== tag));
  };

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

      quillInstance = q;

      const payload = state.composePayload;
      if (payload && payload.email) {
        const email = payload.email;
        const senderEmail = extractEmail(email.sender || "");
        const subj = email.subject || "";
        const date = email.date || "";

        if (payload.type === "reply" || payload.type === "replyAll") {
          setTo(senderEmail ? [senderEmail] : []);
          setSubject(subj.toLowerCase().startsWith("re:") ? subj : `Re: ${subj}`);
          const html = `<p><br></p><blockquote style="border-left: 2px solid #ccc; padding-left: 10px; color: #666;"><p>--- Original Message ---</p><p>From: ${email.sender}</p><p>Date: ${date}</p><p>Subject: ${subj}</p></blockquote>`;
          q.root.innerHTML = html;
          q.setSelection(1, 0);
        } else if (payload.type === "forward") {
          setSubject(subj.toLowerCase().startsWith("fwd:") ? subj : `Fwd: ${subj}`);
          const html = `<p><br></p><blockquote style="border-left: 2px solid #ccc; padding-left: 10px; color: #666;"><p>--- Forwarded Message ---</p><p>From: ${email.sender}</p><p>Date: ${date}</p><p>Subject: ${subj}</p></blockquote>`;
          q.root.innerHTML = html;
          q.setSelection(1, 0);
        }
      }

      onCleanup(() => {
        quillInstance = undefined;
      });
    } else if (!state.showCompose && quillInstance) {
      quillInstance = undefined;
    }
  });

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
            <div class="flex justify-between items-center p-4 border-b border-surface-200 dark:border-surface-800">
              <h2 class="font-semibold capitalize text-surface-900 dark:text-surface-50">
                {state.composePayload?.type === "reply" ? "Reply" :
                 state.composePayload?.type === "replyAll" ? "Reply All" :
                 state.composePayload?.type === "forward" ? "Forward" : "New"} Message
              </h2>
              <button
                onClick={() => setShowCompose(false)}
                class="text-surface-500 hover:text-surface-900 dark:hover:text-white"
              >
                <X size={20} />
              </button>
            </div>

            <div class="px-4 py-2 space-y-2 border-b border-surface-200 dark:border-surface-800">
              <div class="flex items-center gap-2">
                <span class="text-sm text-surface-500 w-12">To:</span>
                <div class="flex-1 flex flex-wrap items-center gap-1 bg-surface-50 dark:bg-surface-800 rounded px-2 py-1 border border-transparent focus-within:border-brand-500">
                  <For each={to()}>
                    {(tag) => (
                      <span class="bg-surface-200 dark:bg-surface-700 text-surface-800 dark:text-surface-200 text-xs px-2 py-0.5 rounded-full flex items-center gap-1">
                        {tag}
                        <button onClick={() => removeTag(tag, to, setTo)} class="hover:text-red-500"><X size={10} /></button>
                      </span>
                    )}
                  </For>
                  <input
                    type="email"
                    class="flex-1 min-w-[120px] bg-transparent outline-none text-sm text-surface-900 dark:text-surface-50 py-0.5"
                    placeholder={to().length === 0 ? "Recipients" : ""}
                    onKeyDown={(e) => handleTagInput(e, to, setTo)}
                    onBlur={(e) => {
                       const val = e.currentTarget.value.trim();
                       if(val) { setTo([...to(), val]); e.currentTarget.value = ""; }
                    }}
                  />
                </div>
                <div class="flex gap-2 text-xs text-surface-500">
                  <Show when={!showCc()}><button onClick={() => setShowCc(true)} class="hover:text-brand-500">Cc</button></Show>
                  <Show when={!showBcc()}><button onClick={() => setShowBcc(true)} class="hover:text-brand-500">Bcc</button></Show>
                </div>
              </div>

              <Show when={showCc() || cc().length > 0}>
                <div class="flex items-center gap-2">
                  <span class="text-sm text-surface-500 w-12">Cc:</span>
                  <div class="flex-1 flex flex-wrap items-center gap-1 bg-surface-50 dark:bg-surface-800 rounded px-2 py-1 border border-transparent focus-within:border-brand-500">
                    <For each={cc()}>
                      {(tag) => (
                        <span class="bg-surface-200 dark:bg-surface-700 text-surface-800 dark:text-surface-200 text-xs px-2 py-0.5 rounded-full flex items-center gap-1">
                          {tag}
                          <button onClick={() => removeTag(tag, cc, setCc)} class="hover:text-red-500"><X size={10} /></button>
                        </span>
                      )}
                    </For>
                    <input type="email" class="flex-1 min-w-[120px] bg-transparent outline-none text-sm text-surface-900 dark:text-surface-50 py-0.5" onKeyDown={(e) => handleTagInput(e, cc, setCc)} onBlur={(e) => { const val = e.currentTarget.value.trim(); if(val) { setCc([...cc(), val]); e.currentTarget.value = ""; } }} />
                  </div>
                </div>
              </Show>

              <Show when={showBcc() || bcc().length > 0}>
                <div class="flex items-center gap-2">
                  <span class="text-sm text-surface-500 w-12">Bcc:</span>
                  <div class="flex-1 flex flex-wrap items-center gap-1 bg-surface-50 dark:bg-surface-800 rounded px-2 py-1 border border-transparent focus-within:border-brand-500">
                    <For each={bcc()}>
                      {(tag) => (
                        <span class="bg-surface-200 dark:bg-surface-700 text-surface-800 dark:text-surface-200 text-xs px-2 py-0.5 rounded-full flex items-center gap-1">
                          {tag}
                          <button onClick={() => removeTag(tag, bcc, setBcc)} class="hover:text-red-500"><X size={10} /></button>
                        </span>
                      )}
                    </For>
                    <input type="email" class="flex-1 min-w-[120px] bg-transparent outline-none text-sm text-surface-900 dark:text-surface-50 py-0.5" onKeyDown={(e) => handleTagInput(e, bcc, setBcc)} onBlur={(e) => { const val = e.currentTarget.value.trim(); if(val) { setBcc([...bcc(), val]); e.currentTarget.value = ""; } }} />
                  </div>
                </div>
              </Show>

              <div class="flex items-center gap-2">
                <span class="text-sm text-surface-500 w-12">Subject:</span>
                <input
                  type="text"
                  value={subject()}
                  onInput={(e) => setSubject(e.currentTarget.value)}
                  class="flex-1 bg-transparent outline-none text-sm text-surface-900 dark:text-surface-50 py-1"
                  placeholder="Subject"
                />
              </div>
            </div>

            <div class="flex-1 overflow-hidden flex flex-col">
              <div ref={editorRef} class="flex-1 overflow-y-auto bg-transparent text-surface-900 dark:text-surface-50"></div>
            </div>

            <Show when={attachments().length > 0}>
              <div class="px-4 py-2 border-t border-surface-200 dark:border-surface-800 flex flex-wrap gap-2">
                <For each={attachments()}>
                  {(file, index) => (
                    <div class="flex items-center gap-2 bg-surface-100 dark:bg-surface-800 px-3 py-1.5 rounded-full text-xs text-surface-700 dark:text-surface-300">
                      <Paperclip size={12} />
                      <span class="truncate max-w-[120px]">{file.name}</span>
                      <button onClick={() => setAttachments((prev) => prev.filter((_, i) => i !== index()))} class="hover:text-red-500"><X size={12} /></button>
                    </div>
                  )}
                </For>
              </div>
            </Show>

            <div class="p-4 border-t border-surface-200 dark:border-surface-800 flex justify-between items-center">
              <label class="cursor-pointer p-2 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg text-surface-500">
                <Paperclip size={18} />
                <input type="file" multiple class="hidden" onChange={handleFileSelect} />
              </label>
              <button
                onClick={handleSend}
                disabled={loading() || to().length === 0 || !subject()}
                class="px-6 py-2 bg-brand-500 hover:bg-brand-600 text-white rounded-lg font-medium flex items-center gap-2 disabled:opacity-50"
              >
                {loading() ? <Loader2 class="animate-spin" size={16} /> : <><Send size={16} /> Send</>}
              </button>
            </div>
          </div>
        </div>
      </Portal>
    </Show>
  );
}
