export interface ICalEvent {
  uid: string;
  summary: string;
  dtstart: Date | null;
  dtend: Date | null;
  location: string;
  description: string;
  isAllDay: boolean;
}

// Basic timezone offset map for common timezones.
// We use hardcoded offsets instead of full IANA rules to avoid shipping a massive timezone database;
// this may cause 1-hour errors during DST transition edge cases, but keeps the frontend bundle tiny.
const COMMON_TZ_OFFSETS: Record<string, number> = {
  "America/New_York": -5,
  "America/Chicago": -6,
  "America/Denver": -7,
  "America/Los_Angeles": -8,
  "Europe/London": 0,
  "Europe/Paris": 1,
  "Europe/Berlin": 1,
  "Asia/Tokyo": 9,
  "Asia/Shanghai": 8,
  "Australia/Sydney": 11,
  "Pacific/Auckland": 13,
  UTC: 0,
  GMT: 0,
};

/**
 * Parses raw iCalendar text into a structured event object.
 * Handles RFC 5545 line unfolding and extracts TZID parameters for basic timezone conversion.
 */
export function parseICal(ical: string): ICalEvent {
  // Unfold lines (RFC 5545 section 3.1)
  const lines = ical
    .replace(/\r\n\s/g, "")
    .replace(/\n\s/g, "")
    .split("\n");

  const event: ICalEvent = {
    uid: "",
    summary: "Untitled Event",
    dtstart: null,
    dtend: null,
    location: "",
    description: "",
    isAllDay: false,
  };

  for (const line of lines) {
    const colonIdx = line.indexOf(":");
    if (colonIdx === -1) continue;

    const keyPart = line.substring(0, colonIdx);
    const value = line.substring(colonIdx + 1);
    const keyLower = keyPart.toLowerCase().split(";")[0];
    const params = keyPart.toLowerCase();

    if (keyLower === "uid") event.uid = value;
    else if (keyLower === "summary") event.summary = value;
    else if (keyLower === "location") event.location = value;
    else if (keyLower === "description")
      event.description = value.replace(/\n/g, "\n").replace(/\\,/g, ",");
    else if (keyLower === "dtstart") {
      event.isAllDay = params.includes("value=date");
      // Extract TZID parameter if present (e.g., DTSTART;TZID=America/New_York:20231024T120000)
      const tzidMatch = keyPart.match(/TZID=([^:;]+)/i);
      event.dtstart = parseICalDate(
        value,
        tzidMatch ? tzidMatch[1] : undefined
      );
    } else if (keyLower === "dtend") {
      const tzidMatch = keyPart.match(/TZID=([^:;]+)/i);
      event.dtend = parseICalDate(value, tzidMatch ? tzidMatch[1] : undefined);
    }
  }

  return event;
}

function parseICalDate(val: string, tzid?: string): Date | null {
  if (!val) return null;

  // Format: 20231024 (All day event)
  if (val.length === 8) {
    const y = parseInt(val.substring(0, 4));
    const m = parseInt(val.substring(4, 6)) - 1;
    const d = parseInt(val.substring(6, 8));
    return new Date(y, m, d);
  }

  // Format: 20231024T120000Z or 20231024T120000
  if (val.length >= 15) {
    const y = parseInt(val.substring(0, 4));
    const m = parseInt(val.substring(4, 6)) - 1;
    const d = parseInt(val.substring(6, 8));
    const h = parseInt(val.substring(9, 11));
    const min = parseInt(val.substring(11, 13));
    const s = parseInt(val.substring(13, 15));

    // Explicit UTC marker
    if (val.endsWith("Z")) return new Date(Date.UTC(y, m, d, h, min, s));

    // Apply known timezone offset
    if (tzid && COMMON_TZ_OFFSETS[tzid] !== undefined) {
      const offsetHours = COMMON_TZ_OFFSETS[tzid];
      // Subtract the offset to get true UTC milliseconds, then create the Date object
      const utcMillis =
        Date.UTC(y, m, d, h, min, s) - offsetHours * 60 * 60 * 1000;
      return new Date(utcMillis);
    }

    // Fallback to local system time if TZID is missing or unknown
    return new Date(y, m, d, h, min, s);
  }

  return null;
}
