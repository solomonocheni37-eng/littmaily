/**
 * Prompts the user to choose between OAuth2 (browser-based) or App Password authentication.
 */
import { Globe, Key } from "lucide-solid";
import type { ProviderOAuthConfig } from "../../utils/providerDiscovery";

interface Props {
  email: () => string;
  providerConfig: () => ProviderOAuthConfig | null;
  setStep: (step: "oauth" | "password") => void;
  close: () => void;
}

export default function MethodStep(props: Props) {
  return (
    <div class="space-y-4">
      <h3 class="font-medium text-surface-800 dark:text-surface-100">
        How would you like to sign in to {props.providerConfig()?.name}?
      </h3>
      <button
        onClick={() => props.setStep("oauth")}
        class="w-full p-4 border border-surface-200 dark:border-surface-700 rounded-lg hover:bg-surface-50 dark:hover:bg-surface-800 text-left transition-colors"
      >
        <div class="font-medium text-brand-600 dark:text-brand-400 flex items-center gap-2">
          <Globe size={16} /> Browser Login
        </div>
        <div class="text-xs text-surface-500 dark:text-surface-400 mt-1">
          More secure. Requires a Client ID & Secret.
        </div>
      </button>
      <button
        onClick={() => props.setStep("password")}
        class="w-full p-4 border border-surface-200 dark:border-surface-700 rounded-lg hover:bg-surface-50 dark:hover:bg-surface-800 text-left transition-colors"
      >
        <div class="font-medium text-surface-900 dark:text-surface-50 flex items-center gap-2">
          <Key size={16} /> App Password
        </div>
        <div class="text-xs text-surface-500 dark:text-surface-400 mt-1">
          Use a generated app password from your provider.
        </div>
      </button>
      <div class="flex justify-between items-center gap-4 pt-2">
        <button
          onClick={props.close}
          class="px-4 py-2.5 text-surface-600 dark:text-surface-300 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg font-medium transition-colors"
        >
          Cancel
        </button>
        <div class="flex-1" />
      </div>
    </div>
  );
}
