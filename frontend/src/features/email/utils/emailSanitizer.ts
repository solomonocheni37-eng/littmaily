// FILE: ./frontend/src/features/email/utils/emailSanitizer.ts
import { EmailApi } from "@/core/ipc";

export const FALLBACK_PIXEL = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP";

// Completely hides 1x1 tracking pixels so they don't break layout or show broken image icons,
// while preserving the DOM structure for legitimate content.
const NUKE_STYLE =
  "display:none !important; visibility:hidden !important; border:0 !important; outline:0 !important; width:0 !important; height:0 !important; max-width:0 !important; max-height:0 !important; min-width:0 !important; min-height:0 !important; padding:0 !important; margin:0 !important; background:transparent !important; background-image:none !important; background-color:transparent !important; overflow:hidden !important; font-size:0 !important; line-height:0 !important; mso-hide:all !important;";

export function sanitizeEmailDom(html: string): string {
  const parser = new DOMParser();
  const doc = parser.parseFromString(html, "text/html");

  const deadElements = doc.querySelectorAll(
    `img[src*="${FALLBACK_PIXEL}"], [background*="${FALLBACK_PIXEL}"], [style*="${FALLBACK_PIXEL}"]`
  );
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
      clone
        .querySelectorAll(
          `img[src*="${FALLBACK_PIXEL}"], [background*="${FALLBACK_PIXEL}"], [style*="${FALLBACK_PIXEL}"]`
        )
        .forEach((node) => node.remove());

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

  return doc.head.innerHTML + doc.body.innerHTML;
}

// CHANGED: Removed `zoom` parameter to prevent iframe reloads and enable smooth transitions
export function buildSrcdoc(isDark: boolean): string {
  const darkModeStyles = isDark
    ? `body { background-color: #18181b; color: #fafafa; } a { color: #818cf8; }`
    : `body { background-color: #ffffff; color: #18181b; } a { color: #4f46e5; }`;

  // Inject CSP into the iframe to block external resources unless explicitly proxied.
  const csp = `default-src 'none'; style-src 'unsafe-inline'; img-src * data:; media-src * data:; font-src * data:; script-src 'nonce-littmaily-internal'; base-uri 'none'; form-action 'none';`;

  return `<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<style>
* { box-sizing: border-box; }
body {
  margin: 0; padding: 24px; font-family: system-ui, -apple-system, sans-serif; line-height: 1.6; word-wrap: break-word;
  ${darkModeStyles}
  zoom: 1;
  transform-origin: top left;
  /* Premium spring-like transition for zooming */
  transition: zoom 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  cursor: grab;
}
body:active { cursor: grabbing; }
img, svg, video, canvas { max-width: 100% !important; height: auto !important; }
table { max-width: 100% !important; border-collapse: collapse; }
img[width="1"], img[height="1"], img[width="0"], img[height="0"] { display: none !important; }
</style>
</head>
<body>
<script nonce="littmaily-internal">
function resize() {
  const height = document.documentElement.scrollHeight;
  window.parent.postMessage({ type: 'email-resize', height: height }, '*');
}

const observer = new MutationObserver(resize);
observer.observe(document.body, { childList: true, subtree: true });

document.addEventListener('click', function(e) {
  let target = e.target;
  while (target && target.tagName !== 'A') { target = target.parentElement; }
  if (target && target.href) {
    e.preventDefault();
    window.parent.postMessage({ type: 'email-link-click', href: target.href }, '*');
  }
});

let isDown = false;
let startY = 0;
let scrollTop = 0;
document.addEventListener('mousedown', (e) => {
  if (e.target.closest('a, button, input, textarea, select, img')) return;
  isDown = true; startY = e.pageY; scrollTop = window.scrollY;
});
document.addEventListener('mouseup', () => { isDown = false; });
document.addEventListener('mouseleave', () => { isDown = false; });
document.addEventListener('mousemove', (e) => {
  if (!isDown) return;
  e.preventDefault();
  window.scrollTo(0, scrollTop - (e.pageY - startY) * 1.5);
});

window.addEventListener('message', function(event) {
  if (event.data && event.data.type === 'email-content') {
    document.body.innerHTML = event.data.html;
    setTimeout(resize, 0);
  } else if (event.data && event.data.type === 'zoom-update') {
    // Apply zoom dynamically without reloading
    document.body.style.zoom = event.data.zoom / 100;

    // Recalculate height continuously during the 300ms transition so the iframe container
    // resizes smoothly without jumping or leaving empty space at the bottom.
    let transitions = 0;
    const interval = setInterval(() => {
      resize();
      transitions++;
      if (transitions > 10) clearInterval(interval);
    }, 30);
  }
});

window.parent.postMessage({ type: 'iframe-ready' }, '*');
</script>
</body>
</html>`;
}

export async function loadRemoteImages(html: string): Promise<string> {
  const parser = new DOMParser();
  const doc = parser.parseFromString(html, "text/html");

  const imgElements = doc.querySelectorAll("[data-src]");
  const imgPromises = Array.from(imgElements).map(async (el) => {
    const originalUrl = el.getAttribute("data-src");
    if (originalUrl) {
      try {
        const dataUri = await EmailApi.proxyRemoteImage(originalUrl);
        el.setAttribute("src", dataUri);
        el.removeAttribute("data-src");
      } catch (e) {
        console.error("[Littmaily] Failed to proxy image", originalUrl, e);
      }
    }
  });

  const bgElements = doc.querySelectorAll("[data-background]");
  const bgPromises = Array.from(bgElements).map(async (el) => {
    const originalUrl = el.getAttribute("data-background");
    if (originalUrl) {
      try {
        const dataUri = await EmailApi.proxyRemoteImage(originalUrl);
        const existingStyle = el.getAttribute("style") || "";
        el.setAttribute(
          "style",
          `${existingStyle}; background-image: url('${dataUri}') !important;`
        );
        el.setAttribute("background", dataUri);
        el.removeAttribute("data-background");
      } catch (e) {
        console.error("[Littmaily] Failed to proxy background", originalUrl, e);
      }
    }
  });

  await Promise.all([...imgPromises, ...bgPromises]);

  const deadElements = doc.querySelectorAll(
    `img[src*="${FALLBACK_PIXEL}"], [background*="${FALLBACK_PIXEL}"], [style*="${FALLBACK_PIXEL}"]`
  );
  deadElements.forEach((el) => {
    el.setAttribute("style", NUKE_STYLE);
    let currentWrapper = el.closest("td, th, div, a, span, li, p, tr, table");
    while (currentWrapper) {
      const clone = currentWrapper.cloneNode(true) as HTMLElement;
      clone
        .querySelectorAll(
          `img[src*="${FALLBACK_PIXEL}"], [background*="${FALLBACK_PIXEL}"], [style*="${FALLBACK_PIXEL}"]`
        )
        .forEach((node) => node.remove());

      const hasOtherContent =
        (clone.textContent || "").trim().length > 0 ||
        clone.querySelector("img, video, svg, iframe, table");

      if (!hasOtherContent) {
        currentWrapper.setAttribute("style", NUKE_STYLE);
        const parent = currentWrapper.parentElement;
        currentWrapper = parent
          ? parent.closest("td, th, div, a, span, li, p, tr, table")
          : null;
      } else break;
    }
  });

  doc
    .querySelectorAll("iframe, frame, object, embed")
    .forEach((el) => el.remove());

  return doc.head.innerHTML + doc.body.innerHTML;
}
