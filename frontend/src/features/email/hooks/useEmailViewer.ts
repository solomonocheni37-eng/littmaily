// FILE: ./frontend/src/features/email/hooks/useEmailViewer.ts
import {
  createResource,
  createSignal,
  onCleanup,
  createEffect,
} from "solid-js";
import { open } from "@tauri-apps/plugin-shell";
import { confirm } from "@tauri-apps/plugin-dialog";
import { EmailApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import { appEvents } from "@/core/events/eventBus";
import {
  sanitizeEmailDom,
  buildShadowStyles,
  proxyResultToSrc,
  FALLBACK_PIXEL,
} from "../utils/emailSanitizer";

/**
 * useEmailViewer – Shadow DOM edition, de-layerized, asset-protocol images,
 * viewport-lazy display + background prewarm.
 *
 * Image loading model (three cooperating paths):
 *  1. PREWARM (Lever 1): on opt-in, one fire-and-forget Rust task fetches ALL
 *     placeholder URLs in DOM order into the disk cache.
 *  2. FIRST BATCH (Lever 4): on opt-in, placeholders already inside the
 *     viewport (+600px) are dispatched synchronously — no observer wait.
 *  3. OBSERVER: everything else loads as it approaches the viewport; by then
 *     the prewarm has usually filled the cache, so these are disk hits.
 * Rust-side dedup (per-URL OnceCell) guarantees a URL is fetched at most once
 * per session even when paths 1–3 overlap.
 */
export function useEmailViewer(
  getScrollContainer?: () => HTMLElement | undefined
) {
  const { state, openCompose } = useAppContext();

  // ── signals ──────────────────────────────────────────────────
  const [imagesLoaded, setImagesLoaded] = createSignal(false);
  const [loadingImages, setLoadingImages] = createSignal(false);
  const [failedImageCount, setFailedImageCount] = createSignal(0);
  const [renderedHtml, setRenderedHtml] = createSignal<string | null>(null);
  const [zoom, setZoom] = createSignal(100);
  const [panMode, setPanMode] = createSignal(false);

  // ── Shadow DOM refs ──────────────────────────────────────────
  let shadowHostEl: HTMLDivElement | null = null;
  let shadowRootRef: ShadowRoot | null = null;
  let stageRef: HTMLElement | null = null;
  let scalerRef: HTMLElement | null = null;
  let paperRef: HTMLElement | null = null;

  let baseWidth = 0;
  let baseHeight = 0;

  let anchorEl: Element | null = null;
  let anchorDocTop = 0;
  let anchorZoom = 100;

  let resizeObserver: ResizeObserver | null = null;
  let watchdog: MutationObserver | null = null;
  let lazyObserver: IntersectionObserver | null = null;
  const cleanupFns: (() => void)[] = [];

  let injectedHtml: string | null = null;

  let isPanning = false;
  let hasDragged = false;
  let panStartX = 0;
  let panStartY = 0;
  let panScrollLeft = 0;
  let panScrollTop = 0;

  const PLACEHOLDER_SELECTOR =
    'img[data-src]:not([data-load-failed]):not([data-retrying]), [data-background]:not([data-bg-failed]):not([data-bg-pending])';

  // ── resource ─────────────────────────────────────────────────
  const [emailBody, { refetch }] = createResource(
    () =>
      state.selectedEmail
        ? `${state.selectedEmail.account_id}-${state.selectedEmail.mailbox_name}-${state.selectedEmail.uid}`
        : null,
    async (key) => {
      if (!key || !state.selectedEmail) return null;
      setImagesLoaded(false);
      setLoadingImages(false);
      setFailedImageCount(0);
      setRenderedHtml(null);
      setZoom(100);

      const email = state.selectedEmail;
      let body = await EmailApi.getCachedBody(
        email.account_id,
        email.mailbox_name,
        email.uid
      );
      if (!body) {
        body = await EmailApi.fetchBody(
          email.account_id,
          email.mailbox_name,
          email.uid
        );
      }
      if (body?.html_body) {
        setRenderedHtml(sanitizeEmailDom(body.html_body));
      }
      return body;
    }
  );

  // ── dimension helpers ────────────────────────────────────────
  const measureBase = () => {
    if (!paperRef) return;
    baseWidth = paperRef.offsetWidth;
    baseHeight = paperRef.offsetHeight;
  };

  const updateDimensions = (scale: number) => {
    if (!stageRef || !shadowHostEl) return;
    const visualWidth = Math.floor(baseWidth * scale);
    const visualHeight = Math.floor(baseHeight * scale + 48);
    stageRef.style.width = `${visualWidth}px`;
    stageRef.style.height = `${visualHeight}px`;
    shadowHostEl.style.height = `${visualHeight + 40}px`;
    updatePanCursor();
  };

  const recordAnchor = () => {
    try {
      const sc = getScrollContainer?.();
      if (!sc || !shadowRootRef || !shadowHostEl) return;
      const rect = sc.getBoundingClientRect();
      const picked = shadowRootRef.elementFromPoint(
        rect.left + Math.min(80, rect.width / 2),
        rect.top + 8
      );
      anchorEl = picked && shadowHostEl.contains(picked) ? picked : paperRef;
      if (!anchorEl) return;
      anchorDocTop = anchorEl.getBoundingClientRect().top + sc.scrollTop;
      anchorZoom = zoom();
    } catch (_) {
      anchorEl = null;
    }
  };

  const compensateAnchor = () => {
    const sc = getScrollContainer?.();
    if (!sc || !anchorEl || !anchorEl.isConnected) return;
    if (zoom() !== anchorZoom) return;
    if (sc.scrollTop <= 0) return;
    const newDocTop = anchorEl.getBoundingClientRect().top + sc.scrollTop;
    const drift = newDocTop - anchorDocTop;
    if (Math.abs(drift) > 1) {
      sc.scrollTop += drift;
    }
    anchorDocTop = newDocTop;
  };

  const syncDimensions = (scale: number) => {
    compensateAnchor();
    measureBase();
    updateDimensions(scale);
    requestAnimationFrame(recordAnchor);
  };

  const applyZoom = (zoomValue: number) => {
    if (!scalerRef || !stageRef || !shadowHostEl) return;
    const scale = zoomValue / 100;
    shadowHostEl.style.overflowX = scale > 1 ? "auto" : "hidden";
    scalerRef.style.willChange = "transform";
    scalerRef.style.transform = `scale(${scale})`;
    const startTime = performance.now();
    const step = () => {
      updateDimensions(scale);
      if (performance.now() - startTime < 350) {
        requestAnimationFrame(step);
      } else if (scalerRef) {
        scalerRef.style.willChange = "";
        recordAnchor();
      }
    };
    requestAnimationFrame(step);
  };

  const updatePanCursor = () => {
    if (!shadowHostEl) return;
    const canPan = panMode() && zoom() > 100;
    if (isPanning) {
      shadowHostEl.style.cursor = "grabbing";
    } else if (canPan) {
      shadowHostEl.style.cursor = "grab";
    } else {
      shadowHostEl.style.cursor = "default";
    }
    shadowHostEl.style.userSelect = canPan ? "none" : "";
    shadowHostEl.style.webkitUserSelect = canPan ? "none" : "";
    shadowHostEl.style.touchAction = canPan ? "none" : "pan-x pan-y";
  };

  const stripEventHandlers = (el: Element) => {
    const attrs = Array.from(el.attributes);
    for (const attr of attrs) {
      if (attr.name.toLowerCase().startsWith("on")) {
        el.removeAttribute(attr.name);
      }
    }
  };

  const handleLinkClick = async (href: string) => {
    if (!href) return;
    const displayHref =
      href.length > 60 ? `${href.substring(0, 60)}...` : href;
    const isConfirmed = await confirm(
      `You are about to open an external link.\n\nDestination:\n${displayHref}\n\nDo you trust this sender?`,
      {
        title: "Security Warning",
        okLabel: "Open Link",
        cancelLabel: "Cancel",
      }
    );
    if (isConfirmed) await open(href);
  };

  // ── image pipeline ───────────────────────────────────────────
  type ProxyMap = Record<string, string>;
  const proxyBatch = (urls: string[]): Promise<ProxyMap> => {
    const api = EmailApi as any;
    if (typeof api.proxyRemoteImages === "function") {
      return api.proxyRemoteImages(urls) as Promise<ProxyMap>;
    }
    return Promise.all(
      urls.map((u) =>
        EmailApi.proxyRemoteImage(u).then((r): [string, string] => [u, r])
      )
    ).then((pairs) => {
      const map: ProxyMap = {};
      pairs.forEach(([k, v]) => {
        map[k] = v;
      });
      return map;
    });
  };

  /** All pending placeholder URLs in DOM order (Lever 5 priority). */
  const collectPlaceholderUrls = (): string[] => {
    if (!paperRef) return [];
    const urls: string[] = [];
    paperRef.querySelectorAll(PLACEHOLDER_SELECTOR).forEach((el) => {
      const u =
        el.getAttribute("data-src") || el.getAttribute("data-background");
      if (u) urls.push(u);
    });
    return urls;
  };

  /** Lever 4: placeholders already inside viewport+margin. Reads batched. */
  const collectViewportPlaceholders = (): {
    imgs: HTMLImageElement[];
    bgs: HTMLElement[];
  } => {
    const imgs: HTMLImageElement[] = [];
    const bgs: HTMLElement[] = [];
    if (!paperRef) return { imgs, bgs };
    const margin = 600;
    const candidates: { el: Element; rect: DOMRect }[] = [];
    // Batch ALL layout reads first; no writes interleaved (TS perf guide).
    paperRef
      .querySelectorAll(PLACEHOLDER_SELECTOR)
      .forEach((el) => candidates.push({ el, rect: el.getBoundingClientRect() }));
    const vh = window.innerHeight;
    candidates.forEach(({ el, rect }) => {
      if (rect.top > vh + margin || rect.bottom < -margin) return;
      if (el.tagName === "IMG") imgs.push(el as HTMLImageElement);
      else bgs.push(el as HTMLElement);
    });
    return { imgs, bgs };
  };

  /** Shared apply-path for first batch and observer batches. */
  const loadAndApply = (
    imgBatch: HTMLImageElement[],
    bgBatch: HTMLElement[]
  ) => {
    if (imgBatch.length === 0 && bgBatch.length === 0) return;
    const urls: string[] = [];
    imgBatch.forEach((img) => {
      const u = img.getAttribute("data-src");
      if (u) urls.push(u);
      img.setAttribute("data-retrying", "true");
    });
    bgBatch.forEach((el) => {
      const u = el.getAttribute("data-background");
      if (u) urls.push(u);
      el.setAttribute("data-bg-pending", "true");
    });
    const uniqueUrls = Array.from(new Set(urls));

    setLoadingImages(true);
    proxyBatch(uniqueUrls)
      .then((results) => {
        let newFailures = 0;
        imgBatch.forEach((img) => {
          const u = img.getAttribute("data-src");
          img.removeAttribute("data-retrying");
          if (!u) return;
          const raw = results[u];
          if (!raw || raw.startsWith(FALLBACK_PIXEL)) {
            img.setAttribute("data-load-failed", "true");
            newFailures++;
          } else {
            img.setAttribute("src", proxyResultToSrc(raw));
            img.removeAttribute("data-src");
            img.removeAttribute("data-load-failed");
          }
        });
        bgBatch.forEach((el) => {
          const u = el.getAttribute("data-background");
          el.removeAttribute("data-bg-pending");
          if (!u) return;
          const raw = results[u];
          if (!raw || raw.startsWith(FALLBACK_PIXEL)) {
            el.setAttribute("data-bg-failed", "true");
          } else {
            const src = proxyResultToSrc(raw);
            const existingStyle = el.getAttribute("style") || "";
            el.setAttribute(
              "style",
              `${existingStyle}; background-image: url('${src}') !important;`
            );
            el.setAttribute("background", src);
            el.removeAttribute("data-background");
          }
        });
        if (newFailures > 0) setFailedImageCount((c) => c + newFailures);
      })
      .catch(() => {
        imgBatch.forEach((img) => {
          img.removeAttribute("data-retrying");
          img.setAttribute("data-load-failed", "true");
        });
        bgBatch.forEach((el) => {
          el.removeAttribute("data-bg-pending");
          el.setAttribute("data-bg-failed", "true");
        });
        setFailedImageCount((c) => c + imgBatch.length);
      })
      .finally(() => setLoadingImages(false));
  };

  const handleLazyIntersection = (entries: IntersectionObserverEntry[]) => {
    if (!imagesLoaded()) return;
    const imgBatch: HTMLImageElement[] = [];
    const bgBatch: HTMLElement[] = [];
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      const el = entry.target as HTMLElement;
      lazyObserver?.unobserve(el);
      if (
        el.tagName === "IMG" &&
        el.hasAttribute("data-src") &&
        !el.hasAttribute("data-load-failed") &&
        !el.hasAttribute("data-retrying")
      ) {
        imgBatch.push(el as HTMLImageElement);
      } else if (
        el.hasAttribute("data-background") &&
        !el.hasAttribute("data-bg-failed") &&
        !el.hasAttribute("data-bg-pending")
      ) {
        bgBatch.push(el);
      }
    }
    loadAndApply(imgBatch, bgBatch);
  };

  const observePlaceholders = () => {
    if (!paperRef || !lazyObserver || !imagesLoaded()) return;
    paperRef
      .querySelectorAll<HTMLImageElement>(
        "img[data-src]:not([data-load-failed])"
      )
      .forEach((el) => lazyObserver?.observe(el));
    paperRef
      .querySelectorAll<HTMLElement>("[data-background]:not([data-bg-failed])")
      .forEach((el) => lazyObserver?.observe(el));
  };

  const retrySingleImage = (img: HTMLImageElement, url: string) => {
    img.setAttribute("data-retrying", "true");
    EmailApi.proxyRemoteImage(url)
      .then((raw) => {
        img.removeAttribute("data-retrying");
        const success = !raw.startsWith(FALLBACK_PIXEL);
        if (success) {
          img.setAttribute("src", proxyResultToSrc(raw));
          img.removeAttribute("data-src");
          img.removeAttribute("data-load-failed");
          setFailedImageCount((c) => Math.max(0, c - 1));
          syncDimensions(zoom() / 100);
        } else {
          img.setAttribute("data-load-failed", "true");
        }
      })
      .catch(() => {
        img.removeAttribute("data-retrying");
        img.setAttribute("data-load-failed", "true");
      });
  };

  const retryFailedImages = () => {
    if (!paperRef) return;
    paperRef
      .querySelectorAll<HTMLImageElement>(
        "img[data-load-failed]:not([data-retrying])"
      )
      .forEach((img) => {
        const url = img.getAttribute("data-src");
        if (url) retrySingleImage(img, url);
      });
  };

  /** Opt-in entry point: prewarm + immediate first batch + arm observer. */
  const triggerLoadRemoteImages = () => {
    if (imagesLoaded() || loadingImages()) return;
    setImagesLoaded(true);

    // Lever 1: background prewarm of the WHOLE email (fire-and-forget).
    const allUrls = collectPlaceholderUrls();
    if (allUrls.length > 0) {
      EmailApi.prewarmImageCache(allUrls).catch((e) => {
        if (import.meta.env.DEV)
          console.error("[Littmaily] prewarm failed", e);
      });
    }

    // Lever 4: visible images start NOW, without waiting for the observer.
    const first = collectViewportPlaceholders();
    loadAndApply(first.imgs, first.bgs);

    observePlaceholders();
  };

  const resetZoom = () => {
    setZoom(100);
    applyZoom(100);
  };

  // ── Shadow DOM factory ───────────────────────────────────────
  const shadowHostRef = (el: HTMLDivElement) => {
    cleanupFns.forEach((fn) => fn());
    cleanupFns.length = 0;
    lazyObserver = null;

    shadowHostEl = el;
    const shadow = el.attachShadow({ mode: "open" });
    shadowRootRef = shadow;

    const styleEl = document.createElement("style");
    styleEl.textContent = buildShadowStyles();
    shadow.appendChild(styleEl);

    stageRef = document.createElement("div");
    stageRef.id = "stage";
    scalerRef = document.createElement("div");
    scalerRef.id = "scaler";
    paperRef = document.createElement("div");
    paperRef.id = "paper";
    scalerRef.appendChild(paperRef);
    stageRef.appendChild(scalerRef);
    shadow.appendChild(stageRef);

    lazyObserver = new IntersectionObserver(handleLazyIntersection, {
      rootMargin: "600px 0px",
      threshold: 0,
    });
    cleanupFns.push(() => lazyObserver?.disconnect());

    const html = renderedHtml();
    if (html && paperRef) {
      paperRef.innerHTML = html;
      injectedHtml = html;
      observePlaceholders();
    }

    anchorEl = null;
    measureBase();
    applyZoom(zoom());
    requestAnimationFrame(recordAnchor);

    resizeObserver = new ResizeObserver(() => {
      syncDimensions(zoom() / 100);
    });
    resizeObserver.observe(paperRef);
    cleanupFns.push(() => resizeObserver?.disconnect());

    watchdog = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        const added = mutation.addedNodes;
        for (let i = 0; i < added.length; i++) {
          const node = added[i];
          if (!(node instanceof Element)) continue;
          const tag = node.tagName.toUpperCase();
          if (tag === "SCRIPT" || tag === "NOSCRIPT") {
            node.remove();
            continue;
          }
          node.querySelectorAll("script, noscript").forEach((s) => s.remove());
          stripEventHandlers(node);
          node
            .querySelectorAll("*")
            .forEach((child) => stripEventHandlers(child));
        }
      }
    });
    watchdog.observe(shadow, { childList: true, subtree: true });
    cleanupFns.push(() => watchdog?.disconnect());

    const onPaperClickCapture = (e: Event) => {
      const img = e.target as HTMLElement;
      if (
        img &&
        img.tagName === "IMG" &&
        img.hasAttribute("data-load-failed") &&
        !img.hasAttribute("data-retrying")
      ) {
        const url = img.getAttribute("data-src");
        if (!url) return;
        e.preventDefault();
        e.stopPropagation();
        retrySingleImage(img as HTMLImageElement, url);
      }
    };
    paperRef.addEventListener("click", onPaperClickCapture, true);
    cleanupFns.push(() =>
      paperRef?.removeEventListener("click", onPaperClickCapture, true)
    );

    const onPaperClick = (e: Event) => {
      let target = e.target as Element | null;
      while (target && target.tagName !== "A") {
        target = target.parentElement;
      }
      const anchor = target as HTMLAnchorElement | null;
      if (!anchor || !anchor.href) return;
      e.preventDefault();
      const href = anchor.href;
      if (href.startsWith("mailto:")) {
        const addr = href.slice("mailto:".length).split("?")[0];
        if (addr.includes("@")) {
          openCompose({ type: "new", to: [addr] });
        }
        return;
      }
      handleLinkClick(href);
    };
    paperRef.addEventListener("click", onPaperClick);
    cleanupFns.push(() => paperRef?.removeEventListener("click", onPaperClick));

    const onPointerDown = (e: PointerEvent) => {
      if (!panMode() || zoom() <= 100) return;
      if (e.button !== 0) return;
      const composedTarget = e.composedPath()[0] as HTMLElement;
      if (
        composedTarget?.closest(
          'input, textarea, select, [contenteditable="true"]'
        )
      )
        return;
      isPanning = true;
      hasDragged = false;
      panStartX = e.pageX;
      panStartY = e.pageY;
      const scrollEl = getScrollContainer?.();
      panScrollLeft = shadowHostEl?.scrollLeft ?? 0;
      panScrollTop = scrollEl?.scrollTop ?? 0;
      e.preventDefault();
      updatePanCursor();
      try {
        shadowHostEl?.setPointerCapture(e.pointerId);
      } catch (_) {
        /* noop */
      }
    };

    const onPointerMove = (e: PointerEvent) => {
      if (!isPanning) return;
      e.preventDefault();
      const walkX = e.pageX - panStartX;
      const walkY = e.pageY - panStartY;
      if (!hasDragged && (Math.abs(walkX) > 3 || Math.abs(walkY) > 3)) {
        hasDragged = true;
      }
      if (hasDragged && shadowHostEl) {
        shadowHostEl.scrollLeft = panScrollLeft - walkX;
        const scrollEl = getScrollContainer?.();
        if (scrollEl) {
          scrollEl.scrollTop = panScrollTop - walkY;
        }
      }
    };

    const endPan = (e: PointerEvent) => {
      if (!isPanning) return;
      isPanning = false;
      updatePanCursor();
      try {
        shadowHostEl?.releasePointerCapture(e.pointerId);
      } catch (_) {
        /* noop */
      }
      if (!hasDragged) {
        const target = e.composedPath()[0] as HTMLElement;
        target?.dispatchEvent(
          new MouseEvent("click", { bubbles: true, cancelable: true })
        );
      }
    };

    shadowHostEl.addEventListener("pointerdown", onPointerDown);
    shadowHostEl.addEventListener("pointermove", onPointerMove);
    shadowHostEl.addEventListener("pointerup", endPan);
    shadowHostEl.addEventListener("pointercancel", endPan);
    cleanupFns.push(() => {
      shadowHostEl?.removeEventListener("pointerdown", onPointerDown);
      shadowHostEl?.removeEventListener("pointermove", onPointerMove);
      shadowHostEl?.removeEventListener("pointerup", endPan);
      shadowHostEl?.removeEventListener("pointercancel", endPan);
    });
  };

  // ── effects ──────────────────────────────────────────────────
  createEffect(() => {
    const html = renderedHtml();
    if (html === injectedHtml) return;
    injectedHtml = html;
    if (paperRef) {
      lazyObserver?.disconnect();
      paperRef.innerHTML = html ?? "";
      anchorEl = null;
      syncDimensions(zoom() / 100);
      observePlaceholders();
    }
  });

  createEffect(() => {
    const z = zoom();
    if (scalerRef) {
      applyZoom(z);
    }
  });

  createEffect(() => {
    panMode();
    updatePanCursor();
  });

  // ── lifecycle ────────────────────────────────────────────────
  const cleanupReopen = appEvents.on("email:reopen", () => refetch());

  onCleanup(() => {
    cleanupReopen();
    cleanupFns.forEach((fn) => fn());
    cleanupFns.length = 0;
  });

  return {
    state,
    emailBody,
    refetch,
    imagesLoaded,
    loadingImages,
    failedImageCount,
    renderedHtml,
    shadowHostRef,
    triggerLoadRemoteImages,
    retryFailedImages,
    zoom,
    setZoom,
    resetZoom,
    panMode,
    setPanMode,
  };
}
