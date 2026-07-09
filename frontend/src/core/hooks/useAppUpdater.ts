import { onMount } from "solid-js";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { toast } from "@/core/ui/toast";

export function useAppUpdater() {
  onMount(async () => {
    // Delay the check slightly so it doesn't block initial UI rendering
    await new Promise((r) => setTimeout(r, 3000));

    try {
      const update = await check();
      if (update?.available) {
        toast(
          `Version ${update.version} is available!`,
          "Install & Restart",
          async () => {
            try {
              await update.downloadAndInstall();
              await relaunch();
            } catch (e) {
              console.error("Update installation failed:", e);
              toast("Update failed. Please try again later.");
            }
          }
        );
      }
    } catch (error) {
      console.error("Failed to check for updates:", error);
    }
  });
}
