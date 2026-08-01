import { EmailApi } from "@/core/ipc";

export const FALLBACK_PIXEL = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP";

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

  doc.querySelectorAll("style").forEach((el) => {
    el.setAttribute("nonce", "littmaily-internal");
  });

  return doc.head.innerHTML + doc.body.innerHTML;
}

export function buildSrcdoc(isDark: boolean): string {
  const canvasBg = isDark ? "#27272a" : "#e5e7eb";

  // NOTE: The <meta> CSP remains intentionally removed. WebKitGTK's srcdoc CSP
  // handling caused endless "Refused to apply a stylesheet" errors. The iframe
  // is isolated by sandbox="allow-scripts" (no allow-same-origin) plus the Rust
  // sanitization pipeline. The nonce attributes below are inert but harmless.

  return `<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<meta name="color-scheme" content="light only">
<meta name="supported-color-schemes" content="light">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body>
<div id="stage">
<div id="scaler">
<div id="paper"></div>
</div>
</div>
<style nonce="littmaily-internal">
* { box-sizing: border-box; }
html {
overflow-x: auto;
overflow-y: hidden;
background-color: ${canvasBg};
margin: 0;
padding: 0;
}
body {
margin: 0;
padding: 0;
overflow: hidden;
-webkit-font-smoothing: antialiased;
}
#stage {
min-width: 100%;
display: flex;
justify-content: center;
padding: 24px 0;
box-sizing: border-box;
transform: translateZ(0);
-webkit-transform: translateZ(0);
}
#scaler {
width: 100%;
max-width: 800px;
transform-origin: top center;
transform: scale(1);
transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
will-change: transform;
}
#paper {
width: 100%;
max-width: 800px;
background-color: #ffffff;
color: #18181b;
box-shadow: 0 4px 12px rgba(0,0,0,0.15);
padding: 48px;
font-family: system-ui, -apple-system, sans-serif;
line-height: 1.6;
word-wrap: break-word;
overflow-wrap: break-word;
overflow-x: hidden;
overflow-y: hidden;
margin: 0 auto;
transform: translateZ(0);
-webkit-transform: translateZ(0);
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
background-image: repeating-linear-gradient(45deg, rgba(0,0,0,0.05) 0, rgba(0,0,0,0.05) 6px, transparent 6px, transparent 12px) !important;
border: 1px dashed #a1a1aa !important;
border-radius: 6px !important;
cursor: pointer !important;
}
#paper img[data-retrying] {
animation: littmaily-retry-pulse 0.9s ease-in-out infinite !important;
}
@keyframes littmaily-retry-pulse {
0%, 100% { opacity: 0.4; }
50% { opacity: 0.9; }
}
::-webkit-scrollbar { width: 12px; height: 12px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background-color: rgba(150, 150, 150, 0.5); border-radius: 6px; border: 3px solid transparent; background-clip: content-box; }
::-webkit-scrollbar-thumb:hover { background-color: rgba(150, 150, 150, 0.8); }
::-webkit-scrollbar-corner { background: transparent; }
</style>
<script nonce="littmaily-internal">
const paper = document.getElementById('paper');
const scaler = document.getElementById('scaler');
const stage = document.getElementById('stage');
let currentZoom = 1;
let lastHeight = 0;
let isPanMode = false;
let isPanning = false;
let hasDragged = false;
let panStartX = 0, panStartY = 0, panScrollLeft = 0, panScrollTop = 0;
let retryCounter = 0;
let resizeScheduled = false;
/* Batch all resize triggers into a single rAF so we post at most one
   email-resize message per frame (no resize spam during image loads). */
function scheduleResize() {
if (resizeScheduled) return;
resizeScheduled = true;
requestAnimationFrame(() => {
resizeScheduled = false;
resize();
});
}
function updateCursor() {
if (isPanning) {
document.body.style.cursor = 'grabbing';
} else if (isPanMode && currentZoom > 1) {
document.body.style.cursor = 'grab';
} else {
document.body.style.cursor = 'default';
}
const canPan = isPanMode && currentZoom > 1;
document.body.style.userSelect = canPan ? 'none' : '';
document.body.style.webkitUserSelect = canPan ? 'none' : '';
document.body.style.touchAction = canPan ? 'none' : 'pan-x pan-y';
}
document.addEventListener('pointerdown', function(e) {
if (!isPanMode || currentZoom <= 1) return;
if (e.button !== 0) return;
if (e.target.closest('input, textarea, select, [contenteditable="true"]')) return;
isPanning = true;
hasDragged = false;
panStartX = e.pageX;
panStartY = e.pageY;
panScrollLeft = window.scrollX || document.documentElement.scrollLeft || document.body.scrollLeft || 0;
panScrollTop = window.scrollY || document.documentElement.scrollTop || document.body.scrollTop || 0;
e.preventDefault();
updateCursor();
try { document.documentElement.setPointerCapture(e.pointerId); } catch(err) {}
});
document.addEventListener('pointermove', function(e) {
if (!isPanning) return;
e.preventDefault();
const walkX = e.pageX - panStartX;
const walkY = e.pageY - panStartY;
if (!hasDragged && (Math.abs(walkX) > 3 || Math.abs(walkY) > 3)) {
hasDragged = true;
}
if (hasDragged) {
const newX = panScrollLeft - walkX;
const newY = panScrollTop - walkY;
window.scrollTo(newX, newY);
document.documentElement.scrollLeft = newX;
document.documentElement.scrollTop = newY;
document.body.scrollLeft = newX;
document.body.scrollTop = newY;
}
});
function endPan(e) {
if (!isPanning) return;
isPanning = false;
updateCursor();
try { document.documentElement.releasePointerCapture(e.pointerId); } catch(err) {}
if (!hasDragged) {
const clickEvent = new MouseEvent('click', {
bubbles: true,
cancelable: true,
view: window
});
e.target.dispatchEvent(clickEvent);
}
}
document.addEventListener('pointerup', endPan);
document.addEventListener('pointercancel', endPan);
function resize() {
if (!paper || !scaler || !stage) return;
const unscaledHeight = paper.offsetHeight;
const paperWidth = paper.offsetWidth;
scaler.style.transform = \`scale(\${currentZoom})\`;
const visualWidth = Math.floor(paperWidth * currentZoom);
const visualHeight = Math.floor((unscaledHeight * currentZoom) + 48);
stage.style.width = \`\${visualWidth}px\`;
stage.style.height = \`\${visualHeight}px\`;
if (Math.abs(visualHeight - lastHeight) > 2) {
lastHeight = visualHeight;
window.parent.postMessage({ type: 'email-resize', height: visualHeight }, '*');
}
updateCursor();
}
const observer = new MutationObserver(scheduleResize);
observer.observe(paper, { childList: true, subtree: true });
document.addEventListener('load', function(e) {
if (e.target.tagName === 'IMG') scheduleResize();
}, true);
document.addEventListener('click', function(e) {
let target = e.target;
while (target && target.tagName !== 'A') { target = target.parentElement; }
if (target && target.href) {
e.preventDefault();
window.parent.postMessage({ type: 'email-link-click', href: target.href }, '*');
}
});
/* Per-image retry: a failed placeholder is clickable. Capture phase +
   stopPropagation so it wins over the link handler above. */
paper.addEventListener('click', function(e) {
const img = e.target;
if (img && img.tagName === 'IMG' && img.hasAttribute('data-load-failed') && !img.hasAttribute('data-retrying')) {
const url = img.getAttribute('data-src');
if (!url) return;
e.preventDefault();
e.stopPropagation();
retryCounter++;
img.setAttribute('data-retry-id', String(retryCounter));
img.setAttribute('data-retrying', 'true');
window.parent.postMessage({ type: 'image-retry', url: url, requestId: retryCounter }, '*');
}
}, true);
window.addEventListener('message', function(event) {
if (event.data && event.data.type === 'email-content') {
paper.innerHTML = event.data.html;
scheduleResize();
} else if (event.data && event.data.type === 'zoom-update') {
currentZoom = event.data.zoom / 100;
let startTime = performance.now();
function step() {
resize();
if (performance.now() - startTime < 350) {
requestAnimationFrame(step);
}
}
requestAnimationFrame(step);
} else if (event.data && event.data.type === 'mode-update') {
isPanMode = event.data.panMode;
updateCursor();
} else if (event.data && event.data.type === 'retry-all-failed') {
paper.querySelectorAll('img[data-load-failed]:not([data-retrying])').forEach(function(img) {
const url = img.getAttribute('data-src');
if (!url) return;
retryCounter++;
img.setAttribute('data-retry-id', String(retryCounter));
img.setAttribute('data-retrying', 'true');
window.parent.postMessage({ type: 'image-retry', url: url, requestId: retryCounter }, '*');
});
} else if (event.data && event.data.type === 'image-retry-result') {
const img = paper.querySelector('img[data-retry-id="' + event.data.requestId + '"]');
if (img) {
img.removeAttribute('data-retry-id');
img.removeAttribute('data-retrying');
if (event.data.success && event.data.dataUri) {
img.setAttribute('src', event.data.dataUri);
img.removeAttribute('data-src');
img.removeAttribute('data-load-failed');
} else {
img.setAttribute('data-load-failed', 'true');
}
scheduleResize();
}
}
});
if (typeof ResizeObserver !== 'undefined') {
const ro = new ResizeObserver(scheduleResize);
ro.observe(document.documentElement);
}
window.parent.postMessage({ type: 'iframe-ready' }, '*');
window.addEventListener('resize', scheduleResize);
</script>
</body>
</html>`;
}

export async function loadRemoteImages(
  html: string
): Promise<{ html: string; failedImages: number }> {
  const parser = new DOMParser();
  const doc = parser.parseFromString(html, "text/html");
  let failedImages = 0;

  const imgElements = doc.querySelectorAll("img[data-src]");
  const imgPromises = Array.from(imgElements).map(async (el) => {
    const originalUrl = el.getAttribute("data-src");
    if (!originalUrl) return;
    try {
      const dataUri = await EmailApi.proxyRemoteImage(originalUrl);
      // The Rust proxy returns the 1x1 fallback pixel whenever the fetch fails
      // (offline, timeout, SSRF block, size limit). Detect that and keep the
      // image retryable (data-src stays) instead of silently swallowing it.
      if (dataUri.startsWith(FALLBACK_PIXEL)) {
        failedImages++;
        el.setAttribute("data-load-failed", "true");
      } else {
        el.setAttribute("src", dataUri);
        el.removeAttribute("data-src");
        el.removeAttribute("data-load-failed");
      }
    } catch (e) {
      failedImages++;
      el.setAttribute("data-load-failed", "true");
      if (import.meta.env.DEV)
        console.error("[Littmaily] Failed to proxy image", originalUrl, e);
    }
  });

  const bgElements = doc.querySelectorAll("[data-background]");
  const bgPromises = Array.from(bgElements).map(async (el) => {
    const originalUrl = el.getAttribute("data-background");
    if (!originalUrl) return;
    try {
      const dataUri = await EmailApi.proxyRemoteImage(originalUrl);
      if (dataUri.startsWith(FALLBACK_PIXEL)) {
        // Background failed: leave data-background in place, don't apply it.
      } else {
        const existingStyle = el.getAttribute("style") || "";
        el.setAttribute(
          "style",
          `${existingStyle}; background-image: url('${dataUri}') !important;`
        );
        el.setAttribute("background", dataUri);
        el.removeAttribute("data-background");
      }
    } catch (e) {
      if (import.meta.env.DEV)
        console.error("[Littmaily] Failed to proxy background", originalUrl, e);
    }
  });

  await Promise.all([...imgPromises, ...bgPromises]);

  // Clean up dead placeholders, but PRESERVE images that failed to load (they
  // still carry data-src) so the user can click them to retry.
  const deadSelector = `img[src*="${FALLBACK_PIXEL}"]:not([data-src]), [background*="${FALLBACK_PIXEL}"]:not([data-background]), [style*="${FALLBACK_PIXEL}"]`;
  const deadElements = doc.querySelectorAll(deadSelector);
  deadElements.forEach((el) => {
    el.setAttribute("style", NUKE_STYLE);
    let currentWrapper = el.closest("td, th, div, a, span, li, p, tr, table");
    while (currentWrapper) {
      const clone = currentWrapper.cloneNode(true) as HTMLElement;
      clone.querySelectorAll(deadSelector).forEach((node) => node.remove());
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

  doc.querySelectorAll("style").forEach((el) => {
    el.setAttribute("nonce", "littmaily-internal");
  });

  return { html: doc.head.innerHTML + doc.body.innerHTML, failedImages };
}
