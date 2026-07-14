// FILE: ./frontend/src/core/events/eventBus.ts
export type AppEvents = {
  "inbox:refresh": undefined;
  "mailboxes:refresh": undefined;
  "email:action": { uid: number; action: string; destMailbox?: string };
  "email:reopen": { uid: number };
};

type EventHandler<T> = (payload: T) => void;

/**
 * Lightweight, strictly-typed pub/sub event bus.
 * Used for cross-feature communication (e.g., triggering an inbox refresh from a deep link)
 * where using SolidJS signals/stores would cause unnecessary re-renders in unrelated components.
 */
class TypedEventBus {
  private listeners = new Map<keyof AppEvents, Set<EventHandler<any>>>();

  on<K extends keyof AppEvents>(event: K, handler: EventHandler<AppEvents[K]>) {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(handler);
    // Returns a cleanup function specifically designed to be passed directly into
    // SolidJS's `onCleanup()`, preventing memory leaks when components unmount.
    return () => this.off(event, handler);
  }

  off<K extends keyof AppEvents>(
    event: K,
    handler: EventHandler<AppEvents[K]>
  ) {
    this.listeners.get(event)?.delete(handler);
  }

  emit<K extends keyof AppEvents>(
    event: K,
    ...args: AppEvents[K] extends undefined ? [] : [AppEvents[K]]
  ) {
    const payload = args[0];
    this.listeners.get(event)?.forEach((handler) => handler(payload));
  }
}

export const appEvents = new TypedEventBus();
