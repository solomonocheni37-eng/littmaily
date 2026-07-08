export type EmailFlag =
  | "Seen"
  | "Answered"
  | "Flagged"
  | "Deleted"
  | "Draft"
  | "Recent"
  | string;

export function parseFlags(flagsJson: string | null | undefined): EmailFlag[] {
  if (!flagsJson) return [];
  try {
    const parsed = JSON.parse(flagsJson);
    if (Array.isArray(parsed)) {
      return parsed.map(String); // Ensure all elements are strings
    }
    return [];
  } catch {
    // Fallback for malformed or legacy raw strings
    return flagsJson.includes("Seen") ? ["Seen"] : [];
  }
}

export function hasFlag(
  flagsJson: string | null | undefined,
  flag: EmailFlag
): boolean {
  return parseFlags(flagsJson).includes(flag);
}

export function serializeFlags(flags: EmailFlag[]): string {
  return JSON.stringify(flags);
}
