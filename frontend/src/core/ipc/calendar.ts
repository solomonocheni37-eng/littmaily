import { commands } from "../types/generated";
import { unwrap } from "./client";

export const CalendarApi = {
  getEvents: (accountId: string) =>
    unwrap(commands.getCalendarEvents(accountId)),
};
