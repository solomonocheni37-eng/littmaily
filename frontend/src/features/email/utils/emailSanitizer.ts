// FILE: ./frontend/src/features/email/utils/emailSanitizer.ts
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

  return doc.head.innerHTML + doc.body.innerHTML;
}

export function buildSrcdoc(isDark: boolean): string {
  const canvasBg = isDark ? '#27272a' : '#e5e7eb';

  const csp = `default-src 'none'; style-src 'unsafe-inline'; img-src * data:; media-src * data:; font-src * data:; script-src 'nonce-littmaily-internal'; base-uri 'none'; form-action 'none';`;

  return `<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<!-- Forces WebKit to treat this document as light-mode, preventing auto-inversion of the white paper -->
<meta name="color-scheme" content="light only">
<meta name="supported-color-schemes" content="light">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<style>
  * { box-sizing: border-box; }

  /* THE VIEWPORT (PDF Viewer Canvas) */
  html {
    width: 100%;
    background-color: ${canvasBg};
    margin: 0;
    padding: 0;
    overflow-x: auto; /* Shows horizontal scrollbar ONLY when zoomed paper exceeds viewport */
    overflow-y: hidden; /* Parent Tauri window handles vertical scrolling */
  }

  body {
    margin: 0;
    padding: 24px 0; /* Top/Bottom padding for the paper shadow */
    background: transparent;
  }

  /* THE SCROLL ANCHOR */
  #canvas {
    margin: 0 auto; /* Centers when small, anchors to left:0 when wider than viewport */
    width: 800px; /* Initial width, dynamically updated by JS */
    height: auto;
  }

  /* THE PAPER (Frozen Layout) */
  #paper {
    width: 800px; /* STRICT FIXED WIDTH. Text will NEVER reflow. */
    min-height: 500px;
    padding: 48px;
    background-color: #ffffff; /* ALWAYS white, like a real PDF */
    color: #18181b;
    box-shadow: ${isDark ? '0 4px 12px rgba(0,0,0,0.4)' : '0 4px 12px rgba(0,0,0,0.1)'};
    font-family: system-ui, -apple-system, sans-serif;
    line-height: 1.6;
    word-wrap: break-word;

    /* PDF Scaling Engine */
    transform-origin: top left; /* Anchors to top-left to prevent negative coordinate clipping */
    transform: scale(1);
    transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  }

  #paper a { color: #4f46e5; }
  img, svg, video, canvas { max-width: 100% !important; height: auto !important; }
  table { max-width: 100% !important; border-collapse: collapse; }
  img[width="1"], img[height="1"], img[width="0"], img[height="0"] { display: none !important; }

  /* Premium Scrollbar Styling */
  ::-webkit-scrollbar { width: 12px; height: 12px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb { background-color: rgba(150, 150, 150, 0.5); border-radius: 6px; border: 3px solid transparent; background-clip: content-box; }
  ::-webkit-scrollbar-thumb:hover { background-color: rgba(150, 150, 150, 0.8); }
  ::-webkit-scrollbar-corner { background: transparent; }
</style>
</head>
<body>
<div id="canvas">
  <div id="paper"></div>
</div>
<script nonce="littmaily-internal">
  const paper = document.getElementById('paper');
  const canvas = document.getElementById('canvas');
  let lastHeight = 0;

  function resize() {
    if (!paper || !canvas) return;

    // getBoundingClientRect returns the TRUE visual dimensions AFTER CSS transform: scale() is applied
    const rect = paper.getBoundingClientRect();

    // Force the canvas wrapper to exactly match the scaled paper's dimensions.
    // This allows the parent html element to show a horizontal scrollbar when scaled > 100%
    canvas.style.width = rect.width + 'px';
    canvas.style.height = rect.height + 'px';

    // Calculate total visual height including body padding
    const visualHeight = rect.height + 48;

    // CRITICAL: Only postMessage if height changed by more than 2px to prevent micro-thrashing
    if (Math.abs(visualHeight - lastHeight) > 2) {
      lastHeight = visualHeight;
      window.parent.postMessage({ type: 'email-resize', height: visualHeight }, '*');
    }
  }

  const observer = new MutationObserver(resize);
  observer.observe(paper, { childList: true, subtree: true });

  // Catch late-loading images that expand the layout
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
      const z = event.data.zoom / 100;

      // Apply CSS Transform instead of Zoom.
      // This freezes the internal layout at 800px, guaranteeing text NEVER reflows and images NEVER shift.
      paper.style.transform = \`scale(\${z})\`;

      // Recalculate dimensions continuously during the 300ms CSS transition
      let startTime = performance.now();
      function step() {
        resize();
        if (performance.now() - startTime < 350) {
          requestAnimationFrame(step);
        }
      }
      requestAnimationFrame(step);
    }
  });

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

  return doc.head.innerHTML + doc.body.innerHTML;
}
