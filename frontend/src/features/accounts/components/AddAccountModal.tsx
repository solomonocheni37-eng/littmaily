/**
 * Orchestrates the multi-step account onboarding flow.
 * Intercepts OS-level deep links (`littmaily://`) to complete OAuth2 flows without
 * relying on localhost TCP listeners, which are frequently blocked by enterprise firewalls
 * or conflict with other local development servers.
 */
import { createSignal, Show, createMemo } from "solid-js";
import { Portal } from "solid-js/web";
import { AccountsApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import { listen } from "@tauri-apps/api/event"; // NEW: Import Tauri event listener
import {
  X,
  Loader2,
  Server,
  Mail,
  Globe,
  Key,
  ChevronLeft,
} from "lucide-solid";

type Step = "email" | "method" | "password" | "oauth";

const getProviderConfig = (email: string) => {
  const domain = email.split("@")[1]?.toLowerCase();
  if (!domain) return null;

  if (domain === "gmail.com" || domain === "googlemail.com") {
    return {
      name: "Google",
      auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
      token_url: "https://oauth2.googleapis.com/token",
      scopes: [
        "https://mail.google.com/",
        "https://www.googleapis.com/auth/userinfo.email",
      ],
      extra: [
        ["access_type", "offline"],
        ["prompt", "consent"],
      ] as [string, string][],
    };
  }

  if (
    domain === "outlook.com" ||
    domain === "hotmail.com" ||
    domain === "live.com"
  ) {
    return {
      name: "Microsoft",
      auth_url:
        "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
      token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
      scopes: [
        "https://outlook.office.com/IMAP.AccessAsUser.All",
        "https://outlook.office.com/SMTP.Send",
        "offline_access",
      ],
      extra: [] as [string, string][],
    };
  }

  if (domain === "yahoo.com") {
    return {
      name: "Yahoo",
      auth_url: "https://api.login.yahoo.com/oauth2/request_auth",
      token_url: "https://api.login.yahoo.com/oauth2/get_token",
      scopes: ["mail-w", "sdct-w"],
      extra: [["language", "en-us"]] as [string, string][],
    };
  }

  return null;
};

export default function AddAccountModal() {
  const { state, setAccounts, setShowAddAccount, selectAccount } =
    useAppContext();

  const [step, setStep] = createSignal<Step>("email");
  const [email, setEmail] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [clientId, setClientId] = createSignal("");
  const [clientSecret, setClientSecret] = createSignal("");
  const [error, setError] = createSignal("");
  const [loading, setLoading] = createSignal(false);

  const providerConfig = createMemo(() => getProviderConfig(email()));

  const handleDiscover = async () => {
    setLoading(true);
    setError("");
    try {
      await AccountsApi.discoverSettings(email());
      if (providerConfig()) {
        setStep("method");
      } else {
        setStep("password");
      }
    } catch (e: any) {
      setError(e.message || "Could not auto-discover settings.");
    } finally {
      setLoading(false);
    }
  };

  const handlePasswordAdd = async () => {
    setLoading(true);
    setError("");
    try {
      const config = await AccountsApi.discoverSettings(email());
      const domain = email().split("@")[1]?.toLowerCase() || "custom";

      const newAccount = await AccountsApi.add({
        email: email(),
        provider: domain,
        imapHost: config.imap.host,
        imapPort: config.imap.port,
        smtpHost: config.smtp.host,
        smtpPort: config.smtp.port,
        password: password(),
        authMethod: "password",
        oauthClientId: null,
        oauthClientSecret: null,
        oauthTokenUrl: null,
        syncWindow: "LAST_30_DAYS",
      });

      const accs = await AccountsApi.list();
      setAccounts(accs);
      selectAccount(newAccount.id);
      setShowAddAccount(false);
    } catch (e: any) {
      setError(e.message || "Failed to authenticate.");
    } finally {
      setLoading(false);
    }
  };

  const handleOAuthLogin = async () => {
    setLoading(true);
    setError("");
    try {
      const config = providerConfig();
      if (!config) throw new Error("Unsupported provider");

      // Use Custom URI Scheme for Enterprise compatibility
      const customRedirectUri = "littmaily://oauth/callback";

      await AccountsApi.startOAuth2(
        clientId(),
        clientSecret(),
        config.auth_url,
        config.token_url,
        config.scopes,
        config.extra,
        customRedirectUri
      );

      // Listen for the OS to redirect back to our app via the deep link
      const unlisten = await listen<{ code: string; state: string }>(
        "oauth:deep-link-callback",
        async (event) => {
          // Unlisten immediately to prevent memory leaks and double-firing
          // if the OS broadcasts the deep link event multiple times.
          unlisten();
          try {
            await AccountsApi.completeOAuth2(
              email(),
              clientId(),
              clientSecret(),
              config.token_url,
              event.payload.code,
              event.payload.state
            );

            const serverConfig = await AccountsApi.discoverSettings(email());

            const newAccount = await AccountsApi.add({
              email: email(),
              provider: config.name.toLowerCase(),
              imapHost: serverConfig.imap.host,
              imapPort: serverConfig.imap.port,
              smtpHost: serverConfig.smtp.host,
              smtpPort: serverConfig.smtp.port,
              password: null,
              authMethod: "oauth2",
              oauthClientId: clientId(),
              oauthClientSecret: clientSecret(),
              oauthTokenUrl: config.token_url,
              syncWindow: "LAST_30_DAYS",
            });

            const accs = await AccountsApi.list();
            setAccounts(accs);
            selectAccount(newAccount.id);
            setShowAddAccount(false);
          } catch (e: any) {
            setError(e.message || "OAuth token exchange failed.");
          } finally {
            setLoading(false);
          }
        }
      );
    } catch (e: any) {
      setError(e.message || "OAuth login failed.");
      setLoading(false);
    }
  };

  return (
    <Show when={state.showAddAccount}>
      <Portal>
        <div
          class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 p-4"
          onClick={() => setShowAddAccount(false)}
        >
          <div
            class="bg-white dark:bg-surface-900 rounded-2xl shadow-2xl w-full max-w-md p-6 border border-surface-200 dark:border-surface-800"
            onClick={(e) => e.stopPropagation()}
          >
            <div class="flex justify-between items-center mb-6">
              <div class="flex items-center gap-2">
                <Show when={step() !== "email"}>
                  <button
                    onClick={() => {
                      if (step() === "method") setStep("email");
                      else setStep("method");
                    }}
                    class="p-1.5 rounded-lg hover:bg-surface-100 dark:hover:bg-surface-800 text-surface-600 dark:text-surface-300 transition-colors"
                    title="Go back"
                  >
                    <ChevronLeft size={20} />
                  </button>
                </Show>
                <h2 class="text-xl font-bold flex items-center gap-2 text-surface-900 dark:text-surface-50">
                  <Mail size={20} class="text-brand-500" /> Add Account
                </h2>
              </div>
              <button
                onClick={() => setShowAddAccount(false)}
                class="p-1.5 rounded-lg hover:bg-surface-100 dark:hover:bg-surface-800 text-surface-500 hover:text-surface-900 dark:hover:text-white transition-colors"
                title="Cancel"
              >
                <X size={20} />
              </button>
            </div>

            <Show when={error()}>
              <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm rounded-lg border border-red-200 dark:border-red-800">
                {error()}
              </div>
            </Show>

            {/* Step 1: Email */}
            <Show when={step() === "email"}>
              <div class="space-y-4">
                <div>
                  <label class="block text-sm font-medium mb-1.5 text-surface-700 dark:text-surface-200">
                    Email Address
                  </label>
                  <input
                    type="email"
                    value={email()}
                    onInput={(e) => setEmail(e.currentTarget.value)}
                    class="w-full px-4 py-2.5 rounded-lg bg-surface-50 dark:bg-surface-800 border border-surface-300 dark:border-surface-700 text-surface-900 dark:text-surface-50 focus:ring-2 focus:ring-brand-500 outline-none transition-colors"
                    placeholder="you@example.com"
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleDiscover();
                    }}
                  />
                </div>
                <div class="flex justify-between items-center gap-4 pt-2">
                  <button
                    onClick={() => setShowAddAccount(false)}
                    class="px-4 py-2.5 text-surface-600 dark:text-surface-300 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg font-medium transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleDiscover}
                    disabled={loading() || !email().includes("@")}
                    class="flex-1 py-2.5 bg-brand-500 hover:bg-brand-600 text-white rounded-lg font-medium flex items-center justify-center gap-2 disabled:opacity-50 transition-colors shadow-sm"
                  >
                    {loading() ? (
                      <Loader2 class="animate-spin" size={18} />
                    ) : (
                      <Server size={18} />
                    )}{" "}
                    Continue
                  </button>
                </div>
              </div>
            </Show>

            {/* Step 2: Method Selection */}
            <Show when={step() === "method"}>
              <div class="space-y-4">
                <h3 class="font-medium text-surface-800 dark:text-surface-100">
                  How would you like to sign in to {providerConfig()?.name}?
                </h3>
                <button
                  onClick={() => setStep("oauth")}
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
                  onClick={() => setStep("password")}
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
                    onClick={() => setShowAddAccount(false)}
                    class="px-4 py-2.5 text-surface-600 dark:text-surface-300 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg font-medium transition-colors"
                  >
                    Cancel
                  </button>
                  <div class="flex-1" />
                </div>
              </div>
            </Show>

            {/* Step 3: Password */}
            <Show when={step() === "password"}>
              <div class="space-y-4">
                <div class="p-3 bg-brand-50 dark:bg-brand-900/20 text-brand-700 dark:text-brand-400 text-sm rounded-lg flex items-center gap-2 border border-brand-200 dark:border-brand-800">
                  <Server size={16} /> Auto-discovered settings for{" "}
                  <b>{email().split("@")[1]}</b>
                </div>
                <Show when={providerConfig()}>
                  <div class="p-3 bg-amber-50 dark:bg-amber-900/20 text-amber-800 dark:text-amber-300 text-xs rounded-lg border border-amber-200 dark:border-amber-800">
                    <b>⚠️ Note:</b> {providerConfig()?.name} requires an "App
                    Password" if you have 2FA enabled.
                  </div>
                </Show>
                <div>
                  <label class="block text-sm font-medium mb-1.5 text-surface-700 dark:text-surface-200">
                    Password / App Password
                  </label>
                  <input
                    type="password"
                    value={password()}
                    onInput={(e) =>
                      setPassword(e.currentTarget.value.replace(/\s/g, ""))
                    }
                    class="w-full px-4 py-2.5 rounded-lg bg-surface-50 dark:bg-surface-800 border border-surface-300 dark:border-surface-700 text-surface-900 dark:text-surface-50 focus:ring-2 focus:ring-brand-500 outline-none transition-colors"
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handlePasswordAdd();
                    }}
                  />
                </div>
                <div class="flex justify-between items-center gap-4 pt-2">
                  <button
                    onClick={() => setShowAddAccount(false)}
                    class="px-4 py-2.5 text-surface-600 dark:text-surface-300 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg font-medium transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handlePasswordAdd}
                    disabled={loading() || !password()}
                    class="flex-1 py-2.5 bg-brand-500 hover:bg-brand-600 text-white rounded-lg font-medium flex items-center justify-center gap-2 disabled:opacity-50 transition-colors shadow-sm"
                  >
                    {loading() ? (
                      <Loader2 class="animate-spin" size={18} />
                    ) : (
                      "Sign In & Sync"
                    )}
                  </button>
                </div>
              </div>
            </Show>

            {/* Step 4: OAuth */}
            <Show when={step() === "oauth"}>
              <div class="space-y-4">
                <div class="p-3 bg-blue-50 dark:bg-blue-900/20 text-blue-800 dark:text-blue-300 text-xs rounded-lg border border-blue-200 dark:border-blue-800">
                  <b>ℹ️ {providerConfig()?.name} OAuth:</b> You must register an
                  OAuth2 App in your developer console to get a Client ID and
                  Secret.
                </div>
                <div>
                  <label class="block text-sm font-medium mb-1.5 text-surface-700 dark:text-surface-200">
                    Client ID
                  </label>
                  <input
                    type="text"
                    value={clientId()}
                    onInput={(e) => setClientId(e.currentTarget.value)}
                    class="w-full px-4 py-2.5 rounded-lg bg-surface-50 dark:bg-surface-800 border border-surface-300 dark:border-surface-700 text-surface-900 dark:text-surface-50 focus:ring-2 focus:ring-brand-500 outline-none transition-colors"
                  />
                </div>
                <div>
                  <label class="block text-sm font-medium mb-1.5 text-surface-700 dark:text-surface-200">
                    Client Secret
                  </label>
                  <input
                    type="password"
                    value={clientSecret()}
                    onInput={(e) => setClientSecret(e.currentTarget.value)}
                    class="w-full px-4 py-2.5 rounded-lg bg-surface-50 dark:bg-surface-800 border border-surface-300 dark:border-surface-700 text-surface-900 dark:text-surface-50 focus:ring-2 focus:ring-brand-500 outline-none transition-colors"
                  />
                </div>
                <div class="flex justify-between items-center gap-4 pt-2">
                  <button
                    onClick={() => setShowAddAccount(false)}
                    class="px-4 py-2.5 text-surface-600 dark:text-surface-300 hover:bg-surface-100 dark:hover:bg-surface-800 rounded-lg font-medium transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleOAuthLogin}
                    disabled={loading() || !clientId() || !clientSecret()}
                    class="flex-1 py-2.5 bg-brand-500 hover:bg-brand-600 text-white rounded-lg font-medium flex items-center justify-center gap-2 disabled:opacity-50 transition-colors shadow-sm"
                  >
                    {loading() ? (
                      <Loader2 class="animate-spin" size={18} />
                    ) : (
                      <>
                        <Globe size={18} /> Sign in with Browser
                      </>
                    )}
                  </button>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </Portal>
    </Show>
  );
}
