/**
 * Auto-saves compose state to the local SQLite database with a 1.5s debounce.
 * Explicitly ignores Quill's default empty state (`<p><br></p>`) to prevent
 * polluting the drafts folder with blank messages.
 */
import { createSignal, createEffect } from "solid-js";
import { EmailApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import { buildRawMime, utf8ToBase64 } from "../utils/mimeBuilder";

export function useComposeDraft(
  to: () => string[],
  cc: () => string[],
  bcc: () => string[],
  subject: () => string,
  textBody: () => string,
  htmlBody: () => string
) {
  const { state } = useAppContext();
  const [draftId, setDraftId] = createSignal<number | null>(null);
  let saveTimeout: ReturnType<typeof setTimeout> | undefined;

  createEffect(() => {
    if (!state.showCompose || !state.selectedAccountId) return;

    const _t = to();
    const _c = cc();
    const _b = bcc();
    const _s = subject();
    const _textBody = textBody();
    const _htmlBody = htmlBody();

    clearTimeout(saveTimeout);

    saveTimeout = setTimeout(async () => {
      // Prevent saving empty drafts
      if (
        _t.length === 0 &&
        _c.length === 0 &&
        _b.length === 0 &&
        _s === "" &&
        _textBody === "" &&
        _htmlBody.replace(/<p><br><\/p>/g, "").trim() === ""
      )
        return;

      const account = state.accounts.find(
        (a: any) => a.id === state.selectedAccountId
      );
      if (!account) return;

      const rawMime = buildRawMime(
        account.email,
        _t,
        _c,
        _b,
        _s,
        _textBody,
        _htmlBody,
        []
      );
      const base64Mime = utf8ToBase64(rawMime);

      try {
        const id = await EmailApi.saveDraft({
          accountId: account.id,
          to: _t,
          cc: _c,
          bcc: _b,
          subject: _s,
          body: _htmlBody, // Store HTML in the DB body field for draft restoration
          rawMimeBase64: base64Mime,
          draftId: draftId(),
        });
        setDraftId(id);
      } catch (e) {
        if (import.meta.env.DEV) console.error("Draft save failed", e);
      }
    }, 1500); // 1.5s debounce prevents hammering SQLite on every keystroke
  });

  return { draftId, setDraftId };
}
