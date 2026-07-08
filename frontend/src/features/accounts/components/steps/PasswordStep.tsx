/**
 * Collects IMAP/SMTP credentials. Displays a warning about App Passwords if the
 * provider is known to enforce 2FA (e.g., Google, Microsoft).
 */
import { Loader2, Server } from "lucide-solid";
import type { ProviderOAuthConfig } from "../../utils/providerDiscovery";

interface Props {
  email: () => string;
  password: () => string;
  setPassword: (val: string) => void;
  providerConfig: () => ProviderOAuthConfig | null;
  loading: () => boolean;
  handlePasswordAdd: () => void;
  close: () => void;
}

export default function PasswordStep(props: Props) {
  return (
    <div class="space-y-4">
      <div class="p-3 bg-brand-50 dark:bg-brand-900/20 text-brand-700 dark:text-brand-400 text-sm rounded-lg flex items-center gap-2 border border-brand-200 dark:border-brand-800">
        <Server size={16} /> Auto-discovered settings for{" "}
        <b>{props.email().split("@")[1]}</b>
      </div>
      {props.providerConfig() && (
        <div class="p-3 bg-amber-50 dark:bg-amber-900/20 text-amber-800 dark:text-amber-300 text-xs rounded-lg border border-amber-200 dark:border-amber-800">
          <b>⚠️ Note:</b> {props.providerConfig()?.name} requires an "App
          Password" if you have 2FA enabled.
        </div>
      )}
      <div>
        <label class="block text-sm font-medium mb-1.5 text-surface-700 dark:text-surface-200">
          Password / App Password
        </label>
        <input
          type="password"
          value={props.password()}
          onInput={(e) =>
            props.setPassword(e.currentTarget.value.replace(/\s/g, ""))
          }
          class="w-full px-4 py-2.5 rounded-lg bg-surface-50 dark:bg-surface-800 border border-surface-300 dark:border-surface-700 text-surface-900 dark:text-surface-50 focus:ring-2 focus:ring-brand-500 outline-none transition-colors"
          onKeyDown={(e) => {
            if (e.key === "Enter") props.handlePasswordAdd();
          }}
        />
      </div>
      <div class="flex justify-between items-center gap-4 pt-2">
        <button
          onClick={props.close}
          class="px-4 py-2.5 text-surface-600 dark:text-surface-300 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg font-medium transition-colors"
        >
          Cancel
        </button>
        <button
          onClick={props.handlePasswordAdd}
          disabled={props.loading() || !props.password()}
          class="flex-1 py-2.5 bg-brand-500 hover:bg-brand-600 text-white rounded-lg font-medium flex items-center justify-center gap-2 disabled:opacity-50 transition-colors shadow-sm"
        >
          {props.loading() ? (
            <Loader2 class="animate-spin" size={18} />
          ) : (
            "Sign In & Sync"
          )}
        </button>
      </div>
    </div>
  );
}
