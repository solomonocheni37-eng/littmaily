import { Mail, ArrowRight } from "lucide-solid";
import { useAppContext } from "@/core/store/AppStore";

export function OnboardingHero() {
  const { setShowAddAccount } = useAppContext();
  return (
    <div class="flex-1 flex flex-col items-center justify-center bg-surface-50 dark:bg-surface-950 p-8 text-center">
      <div class="w-20 h-20 rounded-2xl bg-brand-500/10 flex items-center justify-center mb-6 shadow-glow">
        <Mail size={40} class="text-brand-500" />
      </div>
      <h1 class="text-3xl font-bold text-surface-900 dark:text-surface-50 mb-3">
        Welcome to Littmaily
      </h1>
      <p class="text-surface-500 dark:text-surface-400 max-w-md mb-8">
        Your privacy-focused, local-first email client. Connect your IMAP
        account to get started.
      </p>
      <button
        onClick={() => setShowAddAccount(true)}
        class="px-6 py-3 bg-brand-500 hover:bg-brand-600 text-white rounded-xl font-medium flex items-center gap-2 shadow-elevated transition-all active:scale-95"
      >
        Add Your First Account <ArrowRight size={18} />
      </button>
    </div>
  );
}
