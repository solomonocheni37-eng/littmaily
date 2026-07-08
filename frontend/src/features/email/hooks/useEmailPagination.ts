import { createSignal, createEffect, untrack } from "solid-js";
import { EmailApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import type { Message } from "@/core/types/generated";

export function useEmailPagination() {
  const { state, setFocusedUid } = useAppContext();
  const [emails, setEmails] = createSignal<Message[]>([]);
  const [cursor, setCursor] = createSignal<number | null>(null);
  const [hasMore, setHasMore] = createSignal(true);
  const [isLoading, setIsLoading] = createSignal(false);
  const [isSwitching, setIsSwitching] = createSignal(false);

  const fetchPage = async (reset = false) => {
    if (!state.selectedAccountId || !state.selectedMailboxName) return;
    if (isLoading()) return;
    setIsLoading(true);

    const currentCursor = reset ? null : cursor();
    const pageSize = 50;

    try {
      let newEmails = await EmailApi.getPaginated(
        state.selectedAccountId,
        state.selectedMailboxName,
        currentCursor,
        pageSize
      );

      // Backfill logic for sparse mailboxes: if a page returns 0 results but hasMore is true,
      // fetch older emails from IMAP and retry. This handles gaps in UID sequences.
      if (newEmails.length === 0 && !reset && hasMore() && currentCursor) {
        const lastUid = emails()[emails().length - 1]?.uid;
        if (lastUid) {
          try {
            const backfilled = await EmailApi.backfillOlderEmails(
              state.selectedAccountId,
              state.selectedMailboxName,
              lastUid,
              pageSize
            );
            if (backfilled.length > 0) {
              newEmails = await EmailApi.getPaginated(
                state.selectedAccountId,
                state.selectedMailboxName,
                currentCursor,
                pageSize
              );
            } else {
              setHasMore(false);
            }
          } catch (e) {
            if (import.meta.env.DEV) console.error("Backfill failed", e);
            setHasMore(false);
          }
        }
      }

      if (reset) {
        setEmails(newEmails);
      } else {
        const existingIds = new Set(emails().map((e) => e.id));
        const uniqueNew = newEmails.filter((e) => !existingIds.has(e.id));
        setEmails((prev) => [...prev, ...uniqueNew]);
      }

      // CRITICAL FIX: Paginate by strictly monotonic ID instead of date_timestamp.
      // This prevents skipping emails when multiple messages arrive at the exact same second.
      if (newEmails.length > 0 && newEmails[newEmails.length - 1].id) {
        setCursor(newEmails[newEmails.length - 1].id);
      }

      if (newEmails.length < pageSize) setHasMore(false);
      else if (newEmails.length > 0) setHasMore(true);
    } catch (e) {
      if (import.meta.env.DEV) console.error("Failed to fetch emails:", e);
    } finally {
      setIsLoading(false);
    }
  };

  // Reactively reset and fetch on mailbox/account change
  createEffect(() => {
    const accId = state.selectedAccountId;
    const mbName = state.selectedMailboxName;
    if (accId && mbName) {
      untrack(() => {
        setIsSwitching(true);
        setEmails([]);
        setCursor(null);
        setHasMore(true);
        setFocusedUid(null);
        fetchPage(true).finally(() => setIsSwitching(false));
      });
    }
  });

  return {
    emails,
    setEmails,
    cursor,
    hasMore,
    isLoading,
    isSwitching,
    fetchPage,
  };
}
