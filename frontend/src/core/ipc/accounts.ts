// Thin IPC wrappers for account management.
// All calls are routed through `unwrap` to centralize error handling and toast notifications.
import { commands } from "../types/generated";
import type { AddAccountPayload } from "../types/generated";
import { unwrap } from "./client";

export const AccountsApi = {
  list: () => unwrap(commands.listAccounts()),
  add: (payload: AddAccountPayload) => unwrap(commands.addAccount(payload)),
  delete: (accountId: string) => unwrap(commands.deleteAccount(accountId)),
  discoverSettings: (email: string) =>
    unwrap(commands.discoverEmailSettings(email)),
  updateSyncWindow: (accountId: string, syncWindow: string) =>
    unwrap(commands.updateSyncWindow(accountId, syncWindow)),
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
