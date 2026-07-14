// FILE: ./frontend/src/features/email/hooks/useListKeyboardNav.ts
import { useAppContext } from "@/core/store/AppStore";
import { useHotkeys } from "@/core/hooks/useHotkeys";
import { EmailApi } from "@/core/ipc";
import type { Message } from "@/core/types/generated";
import { appEvents } from "@/core/events/eventBus";

export function useListKeyboardNav(
  emails: () => Message[],
  hasMore: () => boolean,
  isLoading: () => boolean,
  fetchPage: () => Promise<void>
) {
  const { state, selectEmail, setFocusedUid } = useAppContext();

  const navigate = (direction: 1 | -1) => {
    const currentEmails = emails();
    if (currentEmails.length === 0) return;
    const currentIndex = state.focusedUid
      ? currentEmails.findIndex((e) => e.uid === state.focusedUid)
      : -1;
    let nextIndex = currentIndex + direction;
    if (nextIndex < 0) nextIndex = 0;
    if (nextIndex >= currentEmails.length) {
      nextIndex = currentEmails.length - 1;
      if (hasMore() && !isLoading()) fetchPage();
    }
    setFocusedUid(currentEmails[nextIndex].uid);
  };

  const openFocused = () => {
    const uid = state.focusedUid;
    if (!uid) return;
    const email = emails().find((e) => e.uid === uid);
    if (email) {
      // If already open, trigger reopen event instead of doing nothing
      if (
        state.selectedEmail &&
        state.selectedEmail.uid === email.uid &&
        state.selectedEmail.account_id === email.account_id &&
        state.selectedEmail.mailbox_name === email.mailbox_name
      ) {
        appEvents.emit("email:reopen", { uid: email.uid });
        return;
      }

      selectEmail(email);
      let isRead = false;
      try {
        const flags = JSON.parse(email.flags || "[]");
        isRead = flags.includes("Seen");
      } catch {
        isRead = email.flags.includes("Seen");
      }
      if (!isRead) {
        appEvents.emit("email:action", { uid: email.uid, action: "read" });
        EmailApi.updateState(
          email.account_id,
          email.mailbox_name,
          email.uid,
          "read"
        )
          .then(() => {
            appEvents.emit("mailboxes:refresh");
          })
          .catch(console.error);
      }
    }
  };

  const archiveFocused = () => {
    const uid = state.focusedUid;
    if (!uid) return;
    navigate(1);
    appEvents.emit("email:action", { uid, action: "delete" });
    EmailApi.updateState(
      state.selectedAccountId!,
      state.selectedMailboxName!,
      uid,
      "delete"
    )
      .then(() => {
        appEvents.emit("mailboxes:refresh");
      })
      .catch(console.error);
  };

  useHotkeys({
    j: () => navigate(1),
    k: () => navigate(-1),
    arrowdown: () => navigate(1),
    arrowup: () => navigate(-1),
    enter: openFocused,
    o: openFocused,
    e: archiveFocused,
  });
}
