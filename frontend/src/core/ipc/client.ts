import type { AppError } from "../types/generated";
import { toast } from "@/core/ui/toast";

/**
 * Custom error class that preserves the discriminated union `kind` from the Rust backend.
 * Allows the frontend to pattern-match on specific error categories (e.g., "Auth", "Network")
 * rather than relying on brittle string parsing of error messages.
 */
export class IpcError extends Error {
  public kind: string;
  constructor(kind: string, message: string) {
    super(`[${kind}] ${message}`);
    this.name = "IpcError";
    this.kind = kind;
  }
}

/**
 * Unwraps the discriminated union result returned by Tauri Specta IPC calls.
 * On error, it automatically triggers a global toast notification and throws an `IpcError`,
 * preventing the need for repetitive error handling boilerplate in every API call.
 */
export async function unwrap<T>(
  promise: Promise<
    { status: "ok"; data: T } | { status: "error"; error: AppError }
  >
): Promise<T> {
  const result = await promise;
  if (result.status === "error") {
    if (import.meta.env.DEV) {
      console.error(`IPC Error [${result.error.kind}]:`, result.error.message);
    }
    toast(`Error: ${result.error.message}`);
    throw new IpcError(result.error.kind, result.error.message);
  }
  return result.data;
}
