// FILE: ./frontend/src/features/email/hooks/useEmailViewer.ts
import {
  createResource,
  createSignal,
  onMount,
  onCleanup,
  createEffect,
  createMemo,
} from "solid-js";
import { open } from "@tauri-apps/plugin-shell";
import { confirm } from "@tauri-apps/plugin-dialog";
import { EmailApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import { appEvents } from "@/core/events/eventBus";
import {
  sanitizeEmailDom,
  buildSrcdoc,
  loadRemoteImages,
  FALLBACK_PIXEL,
} from "../utils/emailSanitizer";

export function useEmailViewer() {
  const { state } = useAppContext();
  const [imagesLoaded, setImagesLoaded] = createSignal(false);
  const [loadingImages, setLoadingImages] = createSignal(false);
  const [failedImageCount, setFailedImageCount] = createSignal(0);
  const [renderedHtml, setRenderedHtml] = createSignal<string | null>(null);
  const [zoom, setZoom] = createSignal(100);
  const [panMode, setPanMode] = createSignal(false);
  const [iframeReady, setIframeReady] = createSignal(false);
  let iframeRef: HTMLIFrameElement | undefined;

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
      setIframeReady(false);
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

  const isDark = () => document.documentElement.classList.contains("dark");

  const srcdoc = createMemo(() => {
    const dark = isDark();
    return buildSrcdoc(dark);
  });

  createEffect(() => {
    isDark();
    setIframeReady(false);
  });

  // Push HTML to the iframe whenever renderedHtml changes.
  createEffect(() => {
    const html = renderedHtml();
    if (iframeReady() && iframeRef) {
      iframeRef.contentWindow?.postMessage(
        { type: "email-content", html: html ?? "" },
        "*"
      );
    }
  });

  createEffect(() => {
    const z = zoom();
    if (iframeReady() && iframeRef) {
      iframeRef.contentWindow?.postMessage({ type: "zoom-update", zoom: z }, "*");
    }
  });

  // Re-sync zoom when the list pane collapses/expands (the reading pane width
  // changes). Wait for the width transition (~200ms) to settle before re-applying.
  createEffect(() => {
    state.isListPaneCollapsed;
    const z = zoom();
    if (iframeReady() && iframeRef) {
      const t = setTimeout(() => {
        iframeRef?.contentWindow?.postMessage({ type: "zoom-update", zoom: z }, "*");
      }, 250);
      onCleanup(() => clearTimeout(t));
    }
  });

  createEffect(() => {
    const mode = panMode();
    if (iframeReady() && iframeRef) {
      iframeRef.contentWindow?.postMessage({ type: "mode-update", panMode: mode }, "*");
    }
  });

  const handleLinkClick = async (href: string) => {
    if (!href || href.startsWith("mailto:")) return;
    const displayHref = href.length > 60 ? `${href.substring(0, 60)}...` : href;
    const isConfirmed = await confirm(
      `You are about to open an external link.\nDestination:\n${displayHref}\nDo you trust this sender?`,
      { title: "Security Warning", okLabel: "Open Link", cancelLabel: "Cancel" }
    );
    if (isConfirmed) await open(href);
  };

  const triggerLoadRemoteImages = async () => {
    const html = emailBody()?.html_body;
    if (!html || loadingImages()) return;
    setLoadingImages(true);
    try {
      const { html: loaded, failedImages } = await loadRemoteImages(html);
      setRenderedHtml(loaded);
      setFailedImageCount(failedImages);
      setImagesLoaded(true);
    } catch (e) {
      if (import.meta.env.DEV)
        console.error("[Littmaily] loadRemoteImages failed", e);
    } finally {
      setLoadingImages(false);
    }
  };

  // Ask the iframe to retry every image still marked as failed. The iframe owns
  // the live DOM, so it only retries what actually failed — never images that
  // already loaded.
  const retryFailedImages = () => {
    if (iframeReady() && iframeRef) {
      iframeRef.contentWindow?.postMessage({ type: "retry-all-failed" }, "*");
    }
  };

  // `setZoom(100)` alone is a no-op when zoom is already 100 (SolidJS signal
  // equality), so we ALWAYS post directly to guarantee the reset lands.
  const resetZoom = () => {
    setZoom(100);
    if (iframeReady() && iframeRef) {
      iframeRef.contentWindow?.postMessage({ type: "zoom-update", zoom: 100 }, "*");
    }
  };

  onMount(() => {
    const cleanupReopen = appEvents.on("email:reopen", () => {
      refetch();
    });
    const handleMessage = (event: MessageEvent) => {
      if (event.data && event.data.type === "email-link-click") {
        handleLinkClick(event.data.href);
      } else if (event.data && event.data.type === "email-resize" && iframeRef) {
        // Apply height on the next frame and only when it actually changed, to
        // avoid layout thrashing during image loads / zoom animation.
        const nextHeight = `${event.data.height + 40}px`;
        requestAnimationFrame(() => {
          if (iframeRef && iframeRef.style.height !== nextHeight) {
            iframeRef.style.height = nextHeight;
          }
        });
      } else if (event.data && event.data.type === "iframe-ready") {
        setIframeReady(true);
        if (iframeRef) {
          iframeRef.contentWindow?.postMessage({ type: "zoom-update", zoom: zoom() }, "*");
          iframeRef.contentWindow?.postMessage({ type: "mode-update", panMode: panMode() }, "*");
        }
      } else if (event.data && event.data.type === "image-retry") {
        // A single failed image was clicked (or "retry all" fanned out). Proxy
        // just this URL and hand the result back to the iframe.
        const { url, requestId } = event.data;
        EmailApi.proxyRemoteImage(url)
          .then((dataUri) => {
            const success = !dataUri.startsWith(FALLBACK_PIXEL);
            if (success) setFailedImageCount((c) => Math.max(0, c - 1));
            iframeRef?.contentWindow?.postMessage(
              { type: "image-retry-result", requestId, dataUri, success },
              "*"
            );
          })
          .catch(() => {
            iframeRef?.contentWindow?.postMessage(
              { type: "image-retry-result", requestId, dataUri: null, success: false },
              "*"
            );
          });
      }
    };
    window.addEventListener("message", handleMessage);
    onCleanup(() => {
      window.removeEventListener("message", handleMessage);
      cleanupReopen();
    });
  });

  return {
    state,
    emailBody,
    refetch,
    imagesLoaded,
    loadingImages,
    failedImageCount,
    renderedHtml,
    iframeRef: (el: HTMLIFrameElement) => (iframeRef = el),
    srcdoc,
    triggerLoadRemoteImages,
    retryFailedImages,
    zoom,
    setZoom,
    resetZoom,
    panMode,
    setPanMode,
  };
}
