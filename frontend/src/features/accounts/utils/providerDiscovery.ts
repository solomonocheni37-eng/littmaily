export interface ProviderOAuthConfig {
  name: string;
  auth_url: string;
  token_url: string;
  scopes: string[];
  extra: [string, string][];
}

/**
 * Maps email domains to their OAuth2 endpoints and required scopes.
 * Returns `null` for unknown domains, signaling the UI to fall back to manual IMAP/SMTP entry.
 * Note: Google strictly requires `access_type: offline` and `prompt: consent` to guarantee
 * a refresh token is issued on the first login.
 */
export const getProviderConfig = (
  email: string
): ProviderOAuthConfig | null => {
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
      ],
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
      extra: [],
    };
  }

  if (domain === "yahoo.com") {
    return {
      name: "Yahoo",
      auth_url: "https://api.login.yahoo.com/oauth2/request_auth",
      token_url: "https://api.login.yahoo.com/oauth2/get_token",
      scopes: ["mail-w", "sdct-w"],
      extra: [["language", "en-us"]],
    };
  }

  return null;
};
