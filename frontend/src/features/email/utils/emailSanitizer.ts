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

  // CRITICAL FIX: Inject nonce into all email <style> tags
  // WebKitGTK and WebView2 production builds enforce strict CSP on srcdoc iframes.
  // Without this nonce, the WebView will block all inline styles and render a white screen.
  doc.querySelectorAll("style").forEach((el) => {
    el.setAttribute("nonce", "littmaily-internal");
  });

  return doc.head.innerHTML + doc.body.innerHTML;
}

export function buildSrcdoc(isDark: boolean): string {
  const canvasBg = isDark ? "#27272a" : "#e5e7eb";
  // CRITICAL: Include 'nonce-littmaily-internal' in style-src to allow our shell styles
  const csp = `default-src 'none'; style-src 'unsafe-inline' 'nonce-littmaily-internal'; img-src * data:; media-src * data:; font-src * data:; script-src 'nonce-littmaily-internal'; base-uri 'none'; form-action 'none';`;

  return `<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<meta name="color-scheme" content="light only">
<meta name="supported-color-schemes" content="light">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
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
}
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
  box-shadow: 0 4px 12px rgba(0,0,0,0.15);
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
::-webkit-scrollbar { width: 12px; height: 12px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background-color: rgba(150, 150, 150, 0.5); border-radius: 6px; border: 3px solid transparent; background-clip: content-box; }
::-webkit-scrollbar-thumb:hover { background-color: rgba(150, 150, 150, 0.8); }
::-webkit-scrollbar-corner { background: transparent; }
</style>
</head>
<body>
<div id="stage">
  <div id="scaler">
    <div id="paper"></div>
  </div>
</div>
<script nonce="littmaily-internal">
const paper = document.getElementById('paper');
const scaler = document.getElementById('scaler');
const stage = document.getElementById('stage');
let currentZoom = 1;
let lastHeight = 0;
let resizeTimer = null;

// --- MODE & PAN LOGIC ---
let isPanMode = false;
let isPanning = false;
let hasDragged = false;
let panStartX = 0, panStartY = 0, panScrollLeft = 0, panScrollTop = 0;

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

// ------------------------

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

const observer = new MutationObserver(resize);
observer.observe(paper, { childList: true, subtree: true });

document.addEventListener('load', function(e) {
  if (e.target.tagName === 'IMG') setTimeout(resize, 50);
}, true);

document.addEventListener('click', function(e) {
  let target = e.target;
  while (target && target.tagName !== 'A') { target = target.parentElement; }
  if (target && target.href) {
    e.preventDefault();
    window.parent.postMessage({ type: 'email-link-click', href: target.href }, '*');
  }
});

window.addEventListener('message', function(event) {
  if (event.data && event.data.type === 'email-content') {
    paper.innerHTML = event.data.html;
    setTimeout(resize, 0);
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
  }
});

if (typeof ResizeObserver !== 'undefined') {
  const ro = new ResizeObserver(() => {
    if (resizeTimer) clearTimeout(resizeTimer);
    resizeTimer = setTimeout(resize, 50);
  });
  ro.observe(document.documentElement);
}

window.parent.postMessage({ type: 'iframe-ready' }, '*');
window.addEventListener('resize', resize);
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

  // CRITICAL FIX: Inject nonce into all email <style> tags after loading remote images
  doc.querySelectorAll("style").forEach((el) => {
    el.setAttribute("nonce", "littmaily-internal");
  });

  return doc.head.innerHTML + doc.body.innerHTML;
}
