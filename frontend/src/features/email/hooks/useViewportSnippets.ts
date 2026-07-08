import { createEffect } from "solid-js";
import { EmailApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import type { Message } from "@/core/types/generated";
import type { Virtualizer } from "@tanstack/solid-virtual";

export function useViewportSnippets(
  emails: () => Message[],
  setEmails: (fn: (prev: Message[]) => Message[]) => void,
  virtualizer: Virtualizer<HTMLDivElement, Element>
) {
  const { state } = useAppContext();
  let snippetTimeout: ReturnType<typeof setTimeout> | undefined;

  // Prevents duplicate network requests for the same UID.
  const requestedUids = new Set<number>();

  const fetchSnippets = (uids: number[]) => {
    const uniqueUids = uids.filter((u) => !requestedUids.has(u));
    if (
      uniqueUids.length === 0 ||
      !state.selectedAccountId ||
      !state.selectedMailboxName
    )
      return;

    uniqueUids.forEach((u) => requestedUids.add(u));

    clearTimeout(snippetTimeout);
    // 300ms debounce prevents hammering the IMAP server when the user scrolls rapidly through the virtualized list.
    snippetTimeout = setTimeout(() => {
      EmailApi.fetchViewportSnippets(
        state.selectedAccountId!,
        state.selectedMailboxName!,
        uniqueUids
      )
        .then((snippetMap) => {
          setEmails((prev) =>
            prev.map((e) => {
              if (uniqueUids.includes(e.uid)) {
                return { ...e, snippet: snippetMap[e.uid] ?? "" };
              }
              return e;
            })
          );
        })
        .catch((err) => {
          if (import.meta.env.DEV) console.error("Snippet fetch failed", err);
        });
    }, 300);
  };

  // Trigger on virtualizer scroll / viewport change
  createEffect(() => {
    const items = virtualizer.getVirtualItems();
    if (items.length === 0) return;

    const visibleUids = items
      .map((item) => emails()[item.index]?.uid)
      .filter((uid): uid is number => {
        if (uid === undefined) return false;
        const email = emails().find((e) => e.uid === uid);
        return email !== undefined && email.snippet == null;
      });

    fetchSnippets(visibleUids);
  });

  // Clear the cache when mailbox changes so new emails can fetch snippets
  createEffect(() => {
    if (state.selectedAccountId && state.selectedMailboxName) {
      requestedUids.clear();
    }
  });
}
