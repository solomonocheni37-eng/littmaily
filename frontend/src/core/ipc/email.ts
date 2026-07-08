import { commands } from "../types/generated";
import { unwrap } from "./client";

export type EmailAction =
  | "read"
  | "unread"
  | "delete"
  | "move"
  | "star"
  | "unstar"
  | "archive";

export interface QueueEmailPayload {
  accountId: string;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  rawMimeBase64: string;
  scheduledFor?: number | null;
}

export interface SaveDraftPayload {
  accountId: string;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  body: string;
  rawMimeBase64: string;
  draftId: number | null;
}

export const EmailApi = {
  getMailboxes: (accountId: string) => unwrap(commands.getMailboxes(accountId)),
  getPaginated: (
    accountId: string,
    mailboxName: string,
    beforeId: number | null,
    pageSize: number
  ) =>
    unwrap(
      commands.getEmailsPaginated(accountId, mailboxName, beforeId, pageSize)
    ),
  getThreadMessages: (threadId: string) =>
    unwrap(commands.getThreadMessages(threadId)),
  backfillOlderEmails: (
    accountId: string,
    mailboxName: string,
    beforeUid: number,
    limit: number
  ) =>
    unwrap(
      commands.backfillOlderEmails(accountId, mailboxName, beforeUid, limit)
    ),
  fetchViewportSnippets: (
    accountId: string,
    mailboxName: string,
    uids: number[]
  ) => unwrap(commands.fetchViewportSnippets(accountId, mailboxName, uids)),
  fetchBody: (accountId: string, mailboxName: string, uid: number) =>
    unwrap(commands.fetchEmailBody(accountId, mailboxName, uid)),
  getCachedBody: (accountId: string, mailboxName: string, uid: number) =>
    unwrap(commands.getCachedEmailBody(accountId, mailboxName, uid)),
  updateState: (
    accountId: string,
    mailboxName: string,
    uid: number,
    action: EmailAction,
    destMailbox?: string
  ) =>
    unwrap(
      commands.updateEmailState(
        accountId,
        mailboxName,
        uid,
        action,
        destMailbox ?? null
      )
    ),
  // Tauri Specta's TS generator produces slightly mismatched types for complex nested
  // payloads (like arrays of objects with optional fields). The `as any` cast bridges
  // this boundary without altering the actual runtime payload.
  queue: (payload: QueueEmailPayload) =>
    unwrap(commands.queueEmail(payload as any)),
  cancelScheduled: (id: number) => unwrap(commands.cancelScheduledEmail(id)),
  saveDraft: (payload: SaveDraftPayload) =>
    unwrap(commands.saveDraft(payload as any)),
  getDrafts: (accountId: string) => unwrap(commands.getDrafts(accountId)),
  deleteDraft: (draftId: number) => unwrap(commands.deleteDraft(draftId)),
  getAttachmentPath: (blobHash: string) =>
    unwrap(commands.getAttachmentPath(blobHash)),
  saveAttachmentDialog: (blobHash: string, filename: string) =>
    unwrap(commands.saveAttachmentDialog(blobHash, filename)),
  checkForNew: (accountId: string, mailboxName: string) =>
    unwrap(commands.checkForNewEmails(accountId, mailboxName)),
  proxyRemoteImage: (url: string) => unwrap(commands.proxyRemoteImage(url)),
  createFolder: (accountId: string, name: string) =>
    unwrap(commands.createFolder(accountId, name)),
  deleteFolder: (accountId: string, name: string) =>
    unwrap(commands.deleteFolder(accountId, name)),
  renameFolder: (accountId: string, oldName: string, newName: string) =>
    unwrap(commands.renameFolder(accountId, oldName, newName)),
};
