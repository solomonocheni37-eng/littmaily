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
} from "../utils/emailSanitizer";

export function useEmailViewer() {
  const { state } = useAppContext();
  const [imagesLoaded, setImagesLoaded] = createSignal(false);
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
      setRenderedHtml(null);
      setIframeReady(false);
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

  // Generate srcdoc shell (NO HTML injected directly to avoid template literal bugs)
  const srcdoc = createMemo(() => {
    const dark = isDark();
    return buildSrcdoc(dark);
  });

  createEffect(() => {
    isDark();
    setIframeReady(false);
  });

  // RESTORED: Push HTML via postMessage when renderedHtml changes
  createEffect(() => {
    const html = renderedHtml();
    if (iframeReady() && iframeRef) {
      if (html) {
        iframeRef.contentWindow?.postMessage(
          { type: "email-content", html },
          "*"
        );
      } else {
        iframeRef.contentWindow?.postMessage(
          { type: "email-content", html: "" },
          "*"
        );
      }
    }
  });

  createEffect(() => {
    const z = zoom();
    if (iframeReady() && iframeRef) {
      iframeRef.contentWindow?.postMessage(
        { type: "zoom-update", zoom: z },
        "*"
      );
    }
  });

  createEffect(() => {
    const mode = panMode();
    if (iframeReady() && iframeRef) {
      iframeRef.contentWindow?.postMessage(
        { type: "mode-update", panMode: mode },
        "*"
      );
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
    if (!html) return;
    const serializedHtml = await loadRemoteImages(html);
    setRenderedHtml(serializedHtml);
    setImagesLoaded(true);
  };

  onMount(() => {
    const cleanupReopen = appEvents.on("email:reopen", () => {
      refetch();
    });
    const handleMessage = (event: MessageEvent) => {
      if (event.data && event.data.type === "email-link-click") {
        handleLinkClick(event.data.href);
      } else if (
        event.data &&
        event.data.type === "email-resize" &&
        iframeRef
      ) {
        iframeRef.style.height = `${event.data.height + 40}px`;
      } else if (event.data && event.data.type === "iframe-ready") {
        setIframeReady(true);
        if (iframeRef) {
          iframeRef.contentWindow?.postMessage(
            { type: "zoom-update", zoom: zoom() },
            "*"
          );
          iframeRef.contentWindow?.postMessage(
            { type: "mode-update", panMode: panMode() },
            "*"
          );
        }
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
    renderedHtml,
    iframeRef: (el: HTMLIFrameElement) => (iframeRef = el),
    srcdoc,
    triggerLoadRemoteImages,
    zoom,
    setZoom,
    panMode,
    setPanMode,
  };
}
