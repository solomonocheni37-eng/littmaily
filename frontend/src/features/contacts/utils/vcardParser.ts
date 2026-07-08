export interface VCardData {
  fn: string;
  emails: string[];
  tels: string[];
  org: string;
  title: string;
  adr: string[];
  note: string;
  url: string;
}

/**
 * Extracts structured fields from raw vCard 3.0/4.0 text.
 * Flattens semicolon-delimited structured fields (like `ADR` and `ORG`) into
 * comma-separated strings for simplified UI rendering.
 */
export function parseVCard(vcard: string): VCardData {
  // Unfold lines (RFC 6350 section 3.2)
  const lines = vcard
    .replace(/\r\n\s/g, "")
    .replace(/\n\s/g, "")
    .split("\n");

  const data: VCardData = {
    fn: "Unknown",
    emails: [],
    tels: [],
    org: "",
    title: "",
    adr: [],
    note: "",
    url: "",
  };

  for (const line of lines) {
    const colonIdx = line.indexOf(":");
    if (colonIdx === -1) continue;

    const keyPart = line.substring(0, colonIdx);
    const value = line.substring(colonIdx + 1);
    const keyLower = keyPart.toLowerCase().split(";")[0];

    if (keyLower === "fn") data.fn = value;
    else if (keyLower === "email") data.emails.push(value);
    else if (keyLower === "tel") data.tels.push(value);
    else if (keyLower === "org") data.org = value.replace(/;/g, " ");
    else if (keyLower === "title") data.title = value;
    else if (keyLower === "adr")
      data.adr.push(value.split(";").filter(Boolean).join(", "));
    else if (keyLower === "note") data.note = value.replace(/\n/g, "\n");
    else if (keyLower === "url") data.url = value;
  }

  return data;
}
