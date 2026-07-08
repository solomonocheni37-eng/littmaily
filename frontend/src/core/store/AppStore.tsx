import { createContext, useContext } from "solid-js";
import type { ParentComponent } from "solid-js";
import { createStore } from "solid-js/store";
import type { Account, Mailbox, Message } from "@/core/types/generated";

// Kept global to guarantee that opening a new context menu automatically closes any
// previously open menu, preventing overlapping UI states.
type ContextMenuState = { x: number; y: number; email: Message } | null;

export type ComposePayload = {
  type: "new" | "reply" | "replyAll" | "forward";
  email?: Message;
  // Optional prefill for the "To" field, used when composing directly from the Contacts view.
  to?: string[];
};

export type AppView = "mail" | "calendar" | "contacts";

type AppState = {
  accounts: Account[];
  selectedAccountId: string | null;
  mailboxes: Mailbox[];
  selectedMailboxName: string | null;
  selectedEmail: Message | null;
  showAddAccount: boolean;
  showCompose: boolean;
  showSearch: boolean;
  showSettings: boolean;
  composePayload: ComposePayload | null;
  contextMenu: ContextMenuState;
  currentView: AppView;
  isListPaneCollapsed: boolean;
  // Tracks the currently focused email UID for keyboard navigation (j/k/arrow keys).
  // Kept separate from `selectedEmail` because a user can navigate the list with
  // the keyboard without necessarily opening the reading pane.
  focusedUid: number | null;
};

const AppContext = createContext<any>();

export const AppProvider: ParentComponent = (props) => {
  const [state, setState] = createStore<AppState>({
    accounts: [],
    selectedAccountId: null,
    mailboxes: [],
    selectedMailboxName: null,
    selectedEmail: null,
    showAddAccount: false,
    showCompose: false,
    showSearch: false,
    showSettings: false,
    composePayload: null,
    contextMenu: null,
    currentView: "mail",
    isListPaneCollapsed: false,
    focusedUid: null,
  });

  const value = {
    state,
    setAccounts: (accs: Account[]) => setState("accounts", accs),
    selectAccount: (id: string | null) =>
      setState({ selectedAccountId: id, selectedEmail: null }),
    setMailboxes: (mbs: Mailbox[]) => setState("mailboxes", mbs),
    selectMailbox: (name: string) =>
      setState({ selectedMailboxName: name, selectedEmail: null }),
    selectEmail: (email: Message | null) => setState("selectedEmail", email),
    setShowAddAccount: (show: boolean) => setState("showAddAccount", show),
    setShowCompose: (show: boolean) => setState("showCompose", show),
    setShowSearch: (show: boolean) => setState("showSearch", show),
    setShowSettings: (show: boolean) => setState("showSettings", show),
    openCompose: (payload: ComposePayload) => {
      setState("composePayload", payload);
      setState("showCompose", true);
    },
    setContextMenu: (menu: ContextMenuState) => setState("contextMenu", menu),
    setCurrentView: (view: AppView) => setState("currentView", view),
    toggleListPane: () => setState("isListPaneCollapsed", (prev) => !prev),
    setFocusedUid: (uid: number | null) => setState("focusedUid", uid),
  };

  return (
    <AppContext.Provider value={value}>{props.children}</AppContext.Provider>
  );
};

export const useAppContext = () => {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useAppContext must be used within AppProvider");
  return ctx;
};
