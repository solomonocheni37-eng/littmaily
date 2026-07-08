/**
 * Collects OAuth2 Client ID and Secret. We require users to provide their own
 * credentials because shipping a hardcoded client secret in an open-source desktop app
 * violates most provider terms of service and poses a severe security risk.
 */
import { Loader2, Globe } from "lucide-solid";
import type { ProviderOAuthConfig } from "../../utils/providerDiscovery";

interface Props {
  providerConfig: () => ProviderOAuthConfig | null;
  clientId: () => string;
  setClientId: (val: string) => void;
  clientSecret: () => string;
  setClientSecret: (val: string) => void;
  loading: () => boolean;
  handleOAuthLogin: () => void;
  close: () => void;
}

export default function OAuthStep(props: Props) {
  return (
    <div class="space-y-4">
      <div class="p-3 bg-blue-50 dark:bg-blue-900/20 text-blue-800 dark:text-blue-300 text-xs rounded-lg border border-blue-200 dark:border-blue-800">
        <b>ℹ️ {props.providerConfig()?.name} OAuth:</b> You must register an
        OAuth2 App in your developer console to get a Client ID and Secret.
      </div>
      <div>
        <label class="block text-sm font-medium mb-1.5 text-surface-700 dark:text-surface-200">
          Client ID
        </label>
        <input
          type="text"
          value={props.clientId()}
          onInput={(e) => props.setClientId(e.currentTarget.value)}
          class="w-full px-4 py-2.5 rounded-lg bg-surface-50 dark:bg-surface-800 border border-surface-300 dark:border-surface-700 text-surface-900 dark:text-surface-50 focus:ring-2 focus:ring-brand-500 outline-none transition-colors"
        />
      </div>
      <div>
        <label class="block text-sm font-medium mb-1.5 text-surface-700 dark:text-surface-200">
          Client Secret
        </label>
        <input
          type="password"
          value={props.clientSecret()}
          onInput={(e) => props.setClientSecret(e.currentTarget.value)}
          class="w-full px-4 py-2.5 rounded-lg bg-surface-50 dark:bg-surface-800 border border-surface-300 dark:border-surface-700 text-surface-900 dark:text-surface-50 focus:ring-2 focus:ring-brand-500 outline-none transition-colors"
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
          onClick={props.handleOAuthLogin}
          disabled={
            props.loading() || !props.clientId() || !props.clientSecret()
          }
          class="flex-1 py-2.5 bg-brand-500 hover:bg-brand-600 text-white rounded-lg font-medium flex items-center justify-center gap-2 disabled:opacity-50 transition-colors shadow-sm"
        >
          {props.loading() ? (
            <Loader2 class="animate-spin" size={18} />
          ) : (
            <>
              <Globe size={18} /> Sign in with Browser
            </>
          )}
        </button>
      </div>
    </div>
  );
}
