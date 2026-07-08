/**
 * Encapsulates the state and logic for the multi-step account creation flow.
 * Handles provider discovery, credential validation, and final account registration.
 *
 * Intercepts OS-level deep links (`littmaily://`) to complete OAuth2 flows without
 * relying on localhost TCP listeners, which are frequently blocked by enterprise firewalls
 * or conflict with other local development servers.
 */
import { createSignal, createMemo } from "solid-js";
import { AccountsApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import { getProviderConfig } from "../utils/providerDiscovery";
import { listen } from "@tauri-apps/api/event";

export type Step = "email" | "method" | "password" | "oauth";

export function useAccountCreation() {
  const { setAccounts, setShowAddAccount, selectAccount } = useAppContext();

  const [step, setStep] = createSignal<Step>("email");
  const [email, setEmail] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [clientId, setClientId] = createSignal("");
  const [clientSecret, setClientSecret] = createSignal("");
  const [error, setError] = createSignal("");
  const [loading, setLoading] = createSignal(false);

  const providerConfig = createMemo(() => getProviderConfig(email()));
  const close = () => setShowAddAccount(false);

  const goBack = () => {
    setError("");
    if (step() === "method") setStep("email");
    else if (step() === "password" || step() === "oauth") setStep("method");
  };

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
      // Fallback to "custom" if domain extraction fails, ensuring the DB always has a provider string.
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
      close();
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

      // 1. Start OAuth (Passes the custom redirect URI as the 7th argument)
      await AccountsApi.startOAuth2(
        clientId(),
        clientSecret(),
        config.auth_url,
        config.token_url,
        config.scopes,
        config.extra,
        customRedirectUri
      );

      // 2. Listen for the OS to redirect back to our app via the deep link
      const unlisten = await listen<{ code: string; state: string }>(
        "oauth:deep-link-callback",
        async (event) => {
          // Unlisten immediately to prevent memory leaks and double-firing
          // if the OS broadcasts the deep link event multiple times.
          unlisten();

          try {
            // 3. Complete OAuth (Passes deep link code/state as 5th and 6th arguments)
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
            close();
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

  return {
    step,
    setStep,
    goBack,
    email,
    setEmail,
    password,
    setPassword,
    clientId,
    setClientId,
    clientSecret,
    setClientSecret,
    error,
    loading,
    providerConfig,
    handleDiscover,
    handlePasswordAdd,
    handleOAuthLogin,
    close,
  };
}
