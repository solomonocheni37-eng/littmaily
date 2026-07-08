// 1. Define dangerous extensions for security checks
const DANGEROUS_EXTENSIONS = [
  "exe",
  "msi",
  "bat",
  "cmd",
  "com",
  "cpl",
  "hta",
  "inf",
  "ins",
  "isp",
  "jse",
  "lnk",
  "msc",
  "msp",
  "mst",
  "pif",
  "ps1",
  "ps2",
  "reg",
  "rgs",
  "scr",
  "sct",
  "shb",
  "shs",
  "vb",
  "vbe",
  "vbs",
  "wsc",
  "wsf",
  "wsh",
  "sh",
  "bash",
  "csh",
  "ksh",
  "dmg",
  "app",
  "deb",
  "rpm",
  "apk",
];

// 2. Helper to check if a file is an executable
export const isExecutable = (filename: string) =>
  DANGEROUS_EXTENSIONS.includes(filename.split(".").pop()?.toLowerCase() || "");

// 3. Map MIME types to correct file extensions
const getExtensionFromMime = (mimeType: string): string => {
  const map: Record<string, string> = {
    "application/pdf": "pdf",
    "image/jpeg": "jpg",
    "image/png": "png",
    "image/gif": "gif",
    "image/webp": "webp",
    "image/svg+xml": "svg",
    "text/plain": "txt",
    "text/html": "html",
    "text/csv": "csv",
    "application/zip": "zip",
    "application/x-rar-compressed": "rar",
    "application/gzip": "gz",
    "application/msword": "doc",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document":
      "docx",
    "application/vnd.ms-excel": "xls",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": "xlsx",
    "application/vnd.ms-powerpoint": "ppt",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation":
      "pptx",
    "application/octet-stream": "bin",
    "message/rfc822": "eml",
  };
  const baseMime = mimeType.split(";")[0].trim().toLowerCase();
  return (
    map[baseMime] || baseMime.split("/")[1]?.replace(/[^a-z0-9]/g, "") || "bin"
  );
};

// 4. Sanitize, force correct extension, and guarantee uniqueness
export const getSafeFilename = (
  filename: string | null,
  mimeType: string,
  blobHash: string
): string => {
  const ext = getExtensionFromMime(mimeType);

  // Strip Unicode Bidi control characters to prevent Right-To-Left Override (RTLO) spoofing
  // (e.g., rendering "codexe.pdf" as "fdp.exe").
  const BIDI_REGEX = /[\u202A-\u202E\u2066-\u2069\u200E\u200F]/g;
  let name = filename?.trim().replace(BIDI_REGEX, "") || `attachment`;

  // Sanitize illegal filesystem characters
  name = name.replace(/[\/\\:*?"<>|\n\r\t]/g, "_").trim();

  // CRITICAL: Strip trailing dots and spaces.
  // Windows API automatically strips these, so "malware.exe." becomes "malware.exe"
  // but the UI might display it as "malware.exe." tricking the user.
  name = name.replace(/[\s.]+$/, "");

  // Remove existing extension to force the correct one based on MIME type.
  // This prevents extension spoofing where an attacker names a `.exe` as `.txt`.
  name = name.replace(/\.[^/.]+$/, "");

  const shortHash = blobHash.substring(0, 6);
  name = `${name}_${shortHash}.${ext}`;

  if (name.length > 100) {
    const extPart = `.${ext}`;
    name =
      name.substring(0, 100 - extPart.length - 7) + `_${shortHash}${extPart}`;
  }

  return name.replace(/[\s.]+$/, "") || `attachment_${shortHash}.${ext}`;
};
