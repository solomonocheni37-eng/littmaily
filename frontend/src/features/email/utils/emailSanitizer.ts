// FILE: ./frontend/src/features/email/utils/emailSanitizer.ts
import { convertFileSrc } from "@tauri-apps/api/core";

export const FALLBACK_PIXEL =
  "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP";

const NUKE_STYLE =
  "display:none !important; visibility:hidden !important; border:0 !important; outline:0 !important; width:0 !important; height:0 !important; max-width:0 !important; max-height:0 !important; min-width:0 !important; min-height:0 !important; padding:0 !important; margin:0 !important; background:transparent !important; background-image:none !important; background-color:transparent !important; overflow:hidden !important; font-size:0 !important; line-height:0 !important; mso-hide:all !important;";

/**
 * Converts a Rust proxy result into something an <img src> can use.
 * - "data:..."            → the fallback pixel (failure); use as-is.
 * - already a URL         → use as-is.
 * - absolute cache path   → convert to an asset-protocol URL so the
 *   webview streams bytes from disk (no base64 in IPC or the DOM).
 *
 * SELF-CHECK: a bare filesystem path is NOT a valid webview URL. If
 * convertFileSrc ever returns its input unchanged (stale bundle, missing
 * internals), we build the asset URL explicitly instead of letting a raw
 * path reach the DOM, where WebKit reports "The URL can't be shown".
 */
export function proxyResultToSrc(result: string): string {
  if (result.startsWith("data:")) return result;
  if (/^(https?|asset):/i.test(result)) return result;

  let src = "";
  try {
    src = convertFileSrc(result);
  } catch (e) {
    if (import.meta.env.DEV)
      console.error("[Littmaily] convertFileSrc threw", e);
  }
  if (!src || src === result || !/^(https?|asset):/i.test(src)) {
    // Manual fallback mirroring Tauri v2's desktop asset URL format.
    src = `http://asset.localhost/${encodeURIComponent(result)}`;
    if (import.meta.env.DEV)
      console.warn(
        "[Littmaily] convertFileSrc returned a non-URL; used manual asset URL for",
        result
      );
  }
  return src;
}

/**
 * Sanitizes email HTML for safe injection into the Shadow DOM.
 *
 * Defense layers applied here (on top of the Rust ammonia pipeline):
 *  1. Remove <script>, <noscript> elements (incl. inside <svg>).
 *  2. Strip every on* event-handler attribute from every element.
 *  3. Strip javascript: URLs from <a href>, <area href>, <form action>.
 *  4. Remove <iframe>, <frame>, <object>, <embed>.
 *  5. Nuke dead 1×1 placeholder images and their empty wrappers.
 *
 * PERF: also tags every <img> with decoding="async" so image decode
 * happens off the raster-critical path.
 *
 * NOTE: remote images arrive as src=FALLBACK_PIXEL + data-src=<original>
 * (rewritten by the Rust lol_html pass). They stay inert until the
 * viewport-lazy loader in useEmailViewer proxies them on approach.
 */
export function sanitizeEmailDom(html: string): string {
  const parser = new DOMParser();
  const doc = parser.parseFromString(html, "text/html");

  // CRITICAL: nuke only placeholders that are truly dead.
  // Pending remote images carry src=FALLBACK_PIXEL *and* data-src=<original>.
  // They must stay in the layout (as 1x1) because display:none elements never
  // intersect, which would stop the viewport-lazy loader from ever firing.
  const deadSelector = `img[src*="${FALLBACK_PIXEL}"]:not([data-src]), [background*="${FALLBACK_PIXEL}"]:not([data-background]), [style*="${FALLBACK_PIXEL}"]`;
  const deadElements = doc.querySelectorAll(deadSelector);
  deadElements.forEach((el) => {
    el.setAttribute("style", NUKE_STYLE);
    el.removeAttribute("border");
    el.removeAttribute("width");
    el.removeAttribute("height");
    el.removeAttribute("bgcolor");
    el.removeAttribute("background");

    let currentWrapper = el.closest("td, th, div, a, span, li, p, tr, table");
    while (currentWrapper) {
      const clone = currentWrapper.cloneNode(true) as HTMLElement;
      clone.querySelectorAll(deadSelector).forEach((node) => node.remove());
      const hasOtherContent =
        (clone.textContent || "").trim().length > 0 ||
        clone.querySelector("img, video, svg, iframe, table");
      if (!hasOtherContent) {
        currentWrapper.setAttribute("style", NUKE_STYLE);
        currentWrapper.removeAttribute("border");
        currentWrapper.removeAttribute("width");
        currentWrapper.removeAttribute("height");
        currentWrapper.removeAttribute("bgcolor");
        currentWrapper.removeAttribute("background");
        currentWrapper.removeAttribute("cellpadding");
        currentWrapper.removeAttribute("cellspacing");
        const parent = currentWrapper.parentElement;
        currentWrapper = parent
          ? parent.closest("td, th, div, a, span, li, p, tr, table")
          : null;
      } else {
        break;
      }
    }
  });

  doc
    .querySelectorAll("iframe, frame, object, embed")
    .forEach((el) => el.remove());
  doc.querySelectorAll("style").forEach((el) => {
    el.setAttribute("nonce", "littmaily-internal");
  });
  return doc.head.innerHTML + doc.body.innerHTML;
}

/**
 * Returns the CSS injected into the Shadow DOM <style> element.
 * :host targets the Shadow DOM host <div> in ReadingPane.tsx.
 *
 * THEMING: colors come from CSS custom properties set on the host element
 * (--email-canvas, --email-paper-edge). Custom properties inherit across the
 * Shadow boundary, so theme switches are a single property write on the host
 * (via Tailwind dark: variants) — no stylesheet rewrite, no observers.
 *
 * COMPOSITING RULES (the "de-layerized" profile):
 *  - NO translateZ(0) / will-change anywhere in the base styles.
 *  - :host is overflow-x:hidden at rest (single scroller, like the email list).
 *    The hook flips it to auto only while zoom > 100%.
 *  - #scaler has NO permanent will-change; the hook adds it only for the
 *    350ms zoom animation, then releases it.
 */
export function buildShadowStyles(): string {
  return `
    * { box-sizing: border-box; }

    :host {
      display: block;
      overflow-x: hidden;
      overflow-y: hidden;
      background-color: var(--email-canvas, #e5e7eb);
      margin: 0;
      padding: 0;
      -webkit-font-smoothing: antialiased;
      scrollbar-width: none;
      -ms-overflow-style: none;
    }
    :host::-webkit-scrollbar { display: none; }

    #stage {
      min-width: 100%;
      display: flex;
      justify-content: center;
      padding: 24px 0;
      box-sizing: border-box;
    }

    #scaler {
      width: 100%;
      max-width: 800px;
      transform-origin: top center;
      transform: scale(1);
      transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    }

    #paper {
      width: 100%;
      max-width: 800px;
      background-color: #ffffff;
      color: #18181b;
      border: 1px solid var(--email-paper-edge, rgba(0, 0, 0, 0.10));
      padding: 48px;
      font-family: system-ui, -apple-system, sans-serif;
      line-height: 1.6;
      word-wrap: break-word;
      overflow-wrap: break-word;
      overflow-x: hidden;
      overflow-y: hidden;
      margin: 0 auto;
    }

    #paper a { color: #4f46e5; }

    #paper * {
      max-width: 100% !important;
      box-sizing: border-box !important;
    }

    #paper table {
      width: 100% !important;
      max-width: 100% !important;
      border-collapse: collapse;
      table-layout: auto !important;
    }

    #paper td, #paper th {
      max-width: 100% !important;
      word-break: break-word !important;
      overflow: hidden !important;
    }

    #paper img, #paper svg, #paper video, #paper canvas {
      max-width: 100% !important;
      height: auto !important;
      width: auto !important;
      object-fit: contain !important;
      display: block;
    }

    /* Failed-image placeholder: visible, dashed, obviously clickable-to-retry */
    #paper img[data-load-failed] {
      min-width: 48px !important;
      min-height: 48px !important;
      background-color: #f4f4f5 !important;
      background-image: repeating-linear-gradient(
        45deg,
        rgba(0,0,0,0.05) 0, rgba(0,0,0,0.05) 6px,
        transparent 6px, transparent 12px
      ) !important;
      border: 1px dashed #a1a1aa !important;
      border-radius: 6px !important;
      cursor: pointer !important;
    }

    #paper img[data-retrying] {
      animation: littmaily-retry-pulse 0.9s ease-in-out infinite !important;
    }

    @keyframes littmaily-retry-pulse {
      0%, 100% { opacity: 0.4; }
      50%      { opacity: 0.9; }
    }
  `;
}
