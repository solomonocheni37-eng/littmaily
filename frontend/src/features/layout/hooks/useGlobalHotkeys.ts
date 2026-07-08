import { useAppContext } from "@/core/store/AppStore";
import { useHotkeys } from "@/core/hooks/useHotkeys";
import { EmailApi } from "@/core/ipc";
import { appEvents } from "@/core/events/eventBus";

export function useGlobalHotkeys() {
  const { state, openCompose, setShowSearch, selectEmail } = useAppContext();

  useHotkeys({
    "mod+k": () => setShowSearch(true),
    c: () => openCompose({ type: "new" }),
    r: () => {
      if (state.selectedEmail)
        openCompose({ type: "reply", email: state.selectedEmail });
    },
    a: () => {
      if (state.selectedEmail)
        openCompose({ type: "replyAll", email: state.selectedEmail });
    },
    f: () => {
      if (state.selectedEmail)
        openCompose({ type: "forward", email: state.selectedEmail });
    },
    delete: () => {
      if (state.selectedEmail) {
        appEvents.emit("email:action", {
          uid: state.selectedEmail.uid,
          action: "delete",
        });
        EmailApi.updateState(
          state.selectedEmail.account_id,
          state.selectedEmail.mailbox_name,
          state.selectedEmail.uid,
          "delete"
        ).then(() => {
          appEvents.emit("mailboxes:refresh");
        });
        selectEmail(null);
      }
    },
    "shift+u": () => {
      if (state.selectedEmail) {
        appEvents.emit("email:action", {
          uid: state.selectedEmail.uid,
          action: "unread",
        });
        EmailApi.updateState(
          state.selectedEmail.account_id,
          state.selectedEmail.mailbox_name,
          state.selectedEmail.uid,
          "unread"
        ).then(() => {
          appEvents.emit("mailboxes:refresh");
        });
      }
    },
  });
}
