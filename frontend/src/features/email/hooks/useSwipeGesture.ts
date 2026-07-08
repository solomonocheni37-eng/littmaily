import { createSignal } from "solid-js";
import { EmailApi } from "@/core/ipc";
import { appEvents } from "@/core/events/eventBus";

export function useSwipeGesture(
  accountId: () => string,
  mailboxName: () => string,
  uid: () => number
) {
  const [offset, setOffset] = createSignal(0);
  const [isDragging, setIsDragging] = createSignal(false);
  let startX = 0;
  let startY = 0;
  let isSwiping = false;

  const handlePointerDown = (e: PointerEvent) => {
    startX = e.clientX;
    startY = e.clientY;
    setIsDragging(true);
    isSwiping = false;
    // Ensures pointer events continue to fire even if the finger/mouse leaves the element bounds.
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };

  const handlePointerMove = (e: PointerEvent) => {
    if (!isDragging()) return;
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;

    // Cancel the swipe if the user is scrolling vertically, preventing accidental deletions while scrolling.
    if (!isSwiping && Math.abs(dy) > Math.abs(dx)) {
      setIsDragging(false);
      setOffset(0);
      return;
    }

    isSwiping = true;
    if (dx < 0) {
      const resistance = Math.max(dx, -150);
      setOffset(resistance);
    } else {
      setOffset(0);
    }
  };

  const handlePointerUp = (e: PointerEvent) => {
    if (!isDragging()) return;
    setIsDragging(false);
    const dx = e.clientX - startX;

    if (dx < -100) {
      appEvents.emit("email:action", { uid: uid(), action: "delete" });
      EmailApi.updateState(accountId(), mailboxName(), uid(), "delete").catch(
        console.error
      );
    }
    setOffset(0);
  };

  return {
    offset,
    isDragging,
    isSwiping: () => isSwiping,
    handlePointerDown,
    handlePointerMove,
    handlePointerUp,
  };
}
