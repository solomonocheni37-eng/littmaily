import { createSignal } from "solid-js";

export type Toast = {
  id: number;
  msg: string;
  actionLabel?: string;
  onAction?: () => void;
};

export const [toasts, setToasts] = createSignal<Toast[]>([]);
let nextId = 0;

/**
 * Triggers a global toast notification.
 * The 6-second timeout is intentionally longer than the industry standard (3-4s) to give users
 * enough time to read and click "Undo" for destructive or scheduled actions (e.g., unsending an email).
 */
export function toast(
  msg: string,
  actionLabel?: string,
  onAction?: () => void
) {
  const id = nextId++;
  setToasts((prev) => [...prev, { id, msg, actionLabel, onAction }]);
  setTimeout(() => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, 6000);
}
