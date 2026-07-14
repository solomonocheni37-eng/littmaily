// Thin IPC wrappers for account management.
// All calls are routed through `unwrap` to centralize error handling and toast notifications.
import { commands } from "../types/generated";
import type { AddAccountPayload } from "../types/generated";
import { unwrap } from "./client";
import { appEvents } from "@/core/events/eventBus";

export const AccountsApi = {
  list: () => unwrap(commands.listAccounts()),

  add: async (payload: AddAccountPayload) => {
    const res = await unwrap(commands.addAccount(payload));
    appEvents.emit("mailboxes:refresh");
    try { await commands.updateBadgeCount(); } catch (e) {}
    return res;
  },

  delete: async (accountId: string) => {
    const res = await unwrap(commands.deleteAccount(accountId));
    appEvents.emit("mailboxes:refresh");
    try { await commands.updateBadgeCount(); } catch (e) {}
    return res;
  },

  discoverSettings: (email: string) =>
    unwrap(commands.discoverEmailSettings(email)),

  updateSyncWindow: async (accountId: string, syncWindow: string) => {
    const res = await unwrap(commands.updateSyncWindow(accountId, syncWindow));
    appEvents.emit("mailboxes:refresh");
    try { await commands.updateBadgeCount(); } catch (e) {}
    return res;
  },

  startOAuth2: (
    clientId: string,
    clientSecret: string | null,
    authUrl: string,
    tokenUrl: string,
    scopes: string[],
    extraAuthParams: [string, string][],
    redirectUri: string | null
  ) =>
    unwrap(
      commands.startOauth2Login(
        clientId,
        clientSecret,
        authUrl,
        tokenUrl,
        scopes,
        extraAuthParams,
        redirectUri
      )
    ),

  completeOAuth2: (
    email: string,
    clientId: string,
    clientSecret: string,
    tokenUrl: string,
    deepLinkCode: string | null,
    deepLinkState: string | null
  ) =>
    unwrap(
      commands.completeOauth2Login(
        email,
        clientId,
        clientSecret,
        tokenUrl,
        deepLinkCode,
        deepLinkState
      )
    ),
};
