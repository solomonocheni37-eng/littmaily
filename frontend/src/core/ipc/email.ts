import { commands } from "../types/generated";
import { unwrap } from "./client";
import { appEvents } from "@/core/events/eventBus";

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

  backfillOlderEmails: async (
    accountId: string,
    mailboxName: string,
    beforeUid: number,
    limit: number
  ) => {
    const res = await unwrap(
      commands.backfillOlderEmails(accountId, mailboxName, beforeUid, limit)
    );
    if (res.length > 0) {
      appEvents.emit("mailboxes:refresh");
      try { await commands.updateBadgeCount(); } catch (e) {}
    }
    return res;
  },

  fetchViewportSnippets: (
    accountId: string,
    mailboxName: string,
    uids: number[]
  ) => unwrap(commands.fetchViewportSnippets(accountId, mailboxName, uids)),

  fetchBody: (accountId: string, mailboxName: string, uid: number) =>
    unwrap(commands.fetchEmailBody(accountId, mailboxName, uid)),

  getCachedBody: (accountId: string, mailboxName: string, uid: number) =>
    unwrap(commands.getCachedEmailBody(accountId, mailboxName, uid)),

  updateState: async (
    accountId: string,
    mailboxName: string,
    uid: number,
    action: EmailAction,
    destMailbox?: string
  ) => {
    const res = await unwrap(
      commands.updateEmailState(
        accountId,
        mailboxName,
        uid,
        action,
        destMailbox ?? null
      )
    );
    // Auto-refresh Sidebar and OS Badge after state changes (read/unread/delete/move)
    appEvents.emit("mailboxes:refresh");
    try { await commands.updateBadgeCount(); } catch (e) {}
    return res;
  },

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

  checkForNew: async (accountId: string, mailboxName: string) => {
    const res = await unwrap(commands.checkForNewEmails(accountId, mailboxName));
    if (res > 0) {
      appEvents.emit("mailboxes:refresh");
      try { await commands.updateBadgeCount(); } catch (e) {}
    }
    return res;
  },

  proxyRemoteImage: (url: string) => unwrap(commands.proxyRemoteImage(url)),

  createFolder: async (accountId: string, name: string) => {
    const res = await unwrap(commands.createFolder(accountId, name));
    appEvents.emit("mailboxes:refresh");
    return res;
  },
  deleteFolder: async (accountId: string, name: string) => {
    const res = await unwrap(commands.deleteFolder(accountId, name));
    appEvents.emit("mailboxes:refresh");
    return res;
  },
  renameFolder: async (accountId: string, oldName: string, newName: string) => {
    const res = await unwrap(commands.renameFolder(accountId, oldName, newName));
    appEvents.emit("mailboxes:refresh");
    return res;
  },
};
