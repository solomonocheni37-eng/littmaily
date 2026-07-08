import { commands } from "../types/generated";
import { unwrap } from "./client";

export const ContactsApi = {
  getAll: (accountId: string) => unwrap(commands.getContacts(accountId)),
};
