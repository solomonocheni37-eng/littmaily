/**
 * Initial step for capturing the user's email address and triggering provider auto-discovery.
 */
import { Loader2, Server } from "lucide-solid";

interface Props {
  email: () => string;
  setEmail: (val: string) => void;
  loading: () => boolean;
  handleDiscover: () => void;
  close: () => void;
}

export default function EmailStep(props: Props) {
  return (
    <div class="space-y-4">
      <div>
        <label class="block text-sm font-medium mb-1.5 text-surface-700 dark:text-surface-200">
          Email Address
        </label>
        <input
          type="email"
          value={props.email()}
          onInput={(e) => props.setEmail(e.currentTarget.value)}
          class="w-full px-4 py-2.5 rounded-lg bg-surface-50 dark:bg-surface-800 border border-surface-300 dark:border-surface-700 text-surface-900 dark:text-surface-50 focus:ring-2 focus:ring-brand-500 outline-none transition-colors"
          placeholder="you@example.com"
          onKeyDown={(e) => {
            if (e.key === "Enter") props.handleDiscover();
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
          onClick={props.handleDiscover}
          disabled={props.loading() || !props.email().includes("@")}
          class="flex-1 py-2.5 bg-brand-500 hover:bg-brand-600 text-white rounded-lg font-medium flex items-center justify-center gap-2 disabled:opacity-50 transition-colors shadow-sm"
        >
          {props.loading() ? (
            <Loader2 class="animate-spin" size={18} />
          ) : (
            <Server size={18} />
          )}{" "}
          Continue
        </button>
      </div>
    </div>
  );
}
