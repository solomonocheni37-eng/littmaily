import { onCleanup } from "solid-js";

/**
 * Registers global keyboard shortcuts, automatically ignoring events when the user
 * is actively typing in an <input>, <textarea>, or contentEditable element to prevent
 * hijacking normal text entry.
 *
 * Supports a `mod` prefix that maps to `metaKey` on macOS and `ctrlKey` elsewhere.
 */
export function useHotkeys(
  handlers: Record<string, (e: KeyboardEvent) => void>
) {
  const listener = (e: KeyboardEvent) => {
    const tag = (e.target as HTMLElement).tagName;
    // Ignore events when the user is typing in form fields or contentEditable elements.
    if (
      tag === "INPUT" ||
      tag === "TEXTAREA" ||
      (e.target as HTMLElement).isContentEditable
    )
      return;

    const key = [
      e.ctrlKey || e.metaKey ? "mod" : "",
      e.shiftKey ? "shift" : "",
      e.altKey ? "alt" : "",
      e.key.toLowerCase(),
    ]
      .filter(Boolean)
      .join("+");

    if (handlers[key]) {
      e.preventDefault();
      handlers[key](e);
    }
  };

  window.addEventListener("keydown", listener);
  onCleanup(() => window.removeEventListener("keydown", listener));
}
