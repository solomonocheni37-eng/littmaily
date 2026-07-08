/**
 * Encodes a UTF-8 string to Base64 safely.
 * Uses `TextEncoder` because the native `btoa()` function throws a DOMException
 * when passed characters outside the Latin1 range (e.g., emojis, CJK).
 */
export function utf8ToBase64(str: string): string {
  const utf8Bytes = new TextEncoder().encode(str);
  let binaryString = "";
  for (let i = 0; i < utf8Bytes.length; i++) {
    binaryString += String.fromCharCode(utf8Bytes[i]);
  }
  return btoa(binaryString);
}

export const readFileAsBase64 = (file: File): Promise<string> =>
  new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve((reader.result as string).split(",")[1]);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });

export function generateBoundary(): string {
  return (
    "----=_Part_" +
    Math.random().toString(36).substring(2) +
    Date.now().toString(36)
  );
}

/**
 * Constructs a raw RFC 2046 compliant MIME message from structured inputs.
 * Automatically nests `multipart/alternative` inside `multipart/mixed` when both
 * HTML/text bodies and attachments are present, ensuring correct rendering in all clients.
 */
export function buildRawMime(
  from: string,
  to: string[],
  cc: string[],
  bcc: string[],
  subject: string,
  textBody: string,
  htmlBody: string,
  attachments: { name: string; type: string; base64: string }[]
): string {
  const mixedBoundary = generateBoundary();
  const altBoundary = generateBoundary();
  const date = new Date().toUTCString();
  const messageId = `<${Date.now()}.${Math.random()
    .toString(36)
    .substring(2)}@littmaily.app>`;

  let mime = `From: ${from}\r\nTo: ${to.join(", ")}\r\n`;
  if (cc.length > 0) mime += `Cc: ${cc.join(", ")}\r\n`;
  if (bcc.length > 0) mime += `Bcc: ${bcc.join(", ")}\r\n`;
  mime += `Subject: ${subject}\r\nDate: ${date}\r\nMessage-ID: ${messageId}\r\nMIME-Version: 1.0\r\n`;

  const hasHtml =
    htmlBody && htmlBody.trim() !== "" && htmlBody !== "<p><br></p>";
  const hasAttachments = attachments.length > 0;

  if (!hasHtml && !hasAttachments) {
    // Plain text only
    mime += `Content-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n${textBody}`;
  } else if (hasHtml && !hasAttachments) {
    // Multipart Alternative (Text + HTML)
    mime += `Content-Type: multipart/alternative; boundary="${altBoundary}"\r\n\r\n`;
    mime += `--${altBoundary}\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n${textBody}\r\n\r\n`;
    mime += `--${altBoundary}\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n${htmlBody}\r\n\r\n`;
    mime += `--${altBoundary}--\r\n`;
  } else if (!hasHtml && hasAttachments) {
    // Multipart Mixed (Plain Text + Attachments)
    mime += `Content-Type: multipart/mixed; boundary="${mixedBoundary}"\r\n\r\n`;
    mime += `--${mixedBoundary}\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n${textBody}\r\n\r\n`;
    for (const att of attachments) {
      mime += `--${mixedBoundary}\r\nContent-Type: ${att.type}; name="${
        att.name
      }"\r\nContent-Disposition: attachment; filename="${
        att.name
      }"\r\nContent-Transfer-Encoding: base64\r\n\r\n${(
        att.base64.match(/.{1,76}/g) || []
      ).join("\r\n")}\r\n\r\n`;
    }
    mime += `--${mixedBoundary}--\r\n`;
  } else {
    // Multipart Mixed wrapping Multipart Alternative + Attachments (RFC 2046 compliant)
    mime += `Content-Type: multipart/mixed; boundary="${mixedBoundary}"\r\n\r\n`;
    mime += `--${mixedBoundary}\r\nContent-Type: multipart/alternative; boundary="${altBoundary}"\r\n\r\n`;
    mime += `--${altBoundary}\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n${textBody}\r\n\r\n`;
    mime += `--${altBoundary}\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n${htmlBody}\r\n\r\n`;
    mime += `--${altBoundary}--\r\n\r\n`;
    for (const att of attachments) {
      mime += `--${mixedBoundary}\r\nContent-Type: ${att.type}; name="${
        att.name
      }"\r\nContent-Disposition: attachment; filename="${
        att.name
      }"\r\nContent-Transfer-Encoding: base64\r\n\r\n${(
        att.base64.match(/.{1,76}/g) || []
      ).join("\r\n")}\r\n\r\n`;
    }
    mime += `--${mixedBoundary}--\r\n`;
  }

  return mime;
}
