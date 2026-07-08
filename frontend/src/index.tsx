/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import App from "./App.tsx";

const root = document.getElementById("root");

// Renders a full-screen crash overlay to catch fatal errors that bypass SolidJS's
// internal ErrorBoundary (e.g., errors during initial module evaluation or global event handlers).
function showCrash(error: any) {
  document.body.innerHTML = "";
  const div = document.createElement("div");
  div.style.cssText =
    "position:fixed; inset:0; background:#111; color:#ff4444; padding:40px; font-family:monospace; z-index:99999; overflow:auto;";

  const h1 = document.createElement("h1");
  h1.innerText = " SOLIDJS CRASH DETECTED";
  div.appendChild(h1);

  const pre = document.createElement("pre");
  pre.style.cssText =
    "background:#222; padding:20px; color:#fff; white-space:pre-wrap; border:1px solid #ff4444; border-radius:8px;";

  let msg = "Unknown error";
  if (error) {
    if (error instanceof Error) msg = error.stack || error.message;
    else {
      try {
        msg = JSON.stringify(error, null, 2);
      } catch (_) {
        msg = String(error);
      }
    }
  }
  pre.innerText = msg;
  div.appendChild(pre);
  document.body.appendChild(div);
}

// Global error handler to catch unhandled exceptions.
window.onerror = function (msg, _url, _line, _col, error) {
  const errorMsg = typeof msg === "string" ? msg : error?.message || "";

  // Ignore benign ResizeObserver loop warnings. This is a known browser quirk
  // when using dynamic virtualizers (like @tanstack/solid-virtual) that resize
  // elements during layout passes. It is completely safe to ignore and prevents
  // false-positive crash screens.
  if (errorMsg.includes("ResizeObserver loop")) {
    return true;
  }

  showCrash(error || msg);
};

window.addEventListener("unhandledrejection", function (event) {
  showCrash(event.reason);
});

try {
  render(() => {
    try {
      return <App />;
    } catch (e) {
      showCrash(e);
      return null;
    }
  }, root!);
} catch (e) {
  showCrash(e);
}
