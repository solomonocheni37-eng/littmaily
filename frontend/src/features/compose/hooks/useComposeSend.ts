/**
 * Handles the final MIME assembly and outbox queueing.
 * Implements a 10-second "Undo Send" delay by scheduling the message in the future,
 * allowing the background outbox worker to cancel it if the user clicks Undo.
 */
import { createSignal } from "solid-js";
import { EmailApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import { toast } from "@/core/ui/toast";
import {
  buildRawMime,
  utf8ToBase64,
  readFileAsBase64,
} from "../utils/mimeBuilder";

export function useComposeSend(
  to: () => string[],
  cc: () => string[],
  bcc: () => string[],
  subject: () => string,
  textBody: () => string,
  htmlBody: () => string,
  attachments: () => File[],
  draftId: () => number | null,
  resetForm: () => void
) {
  const { state, setShowCompose } = useAppContext();
  const [loading, setLoading] = createSignal(false);

  const handleSend = async () => {
    if (!state.selectedAccountId) return;
    setLoading(true);

    try {
      const account = state.accounts.find(
        (a: any) => a.id === state.selectedAccountId
      );
      if (!account) throw new Error("No account selected");

      const attPromises = attachments().map(async (f) => ({
        name: f.name,
        type: f.type || "application/octet-stream",
        base64: await readFileAsBase64(f),
      }));
      const atts = await Promise.all(attPromises);

      // Pass both text and html bodies to the MIME builder
      const rawMime = buildRawMime(
        account.email,
        to(),
        cc(),
        bcc(),
        subject(),
        textBody(),
        htmlBody(),
        atts
      );
      const base64Mime = utf8ToBase64(rawMime);

      // Calculate 10 seconds from now for "Undo Send"
      const scheduledFor = Math.floor(Date.now() / 1000) + 10;

      const id = await EmailApi.queue({
        accountId: account.id,
        to: to(),
        cc: cc(),
        bcc: bcc(),
        subject: subject(),
        rawMimeBase64: base64Mime,
        scheduledFor: scheduledFor,
      });

      if (draftId()) {
        await EmailApi.deleteDraft(draftId()!);
      }

      setShowCompose(false);
      resetForm();

      toast("Message scheduled.", "Undo", async () => {
        try {
          if (id) await EmailApi.cancelScheduled(id);
          toast("Send cancelled.");
        } catch (e) {
          toast("Failed to cancel. Message may have already sent.");
        }
      });
    } catch (e) {
      if (import.meta.env.DEV) console.error(e);
      toast("Failed to send message");
    } finally {
      setLoading(false);
    }
  };

  return { loading, handleSend };
}
