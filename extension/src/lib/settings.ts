// Extension settings, kept in chrome.storage.local (never synced, never sent anywhere but api.github.com).

export interface Settings {
  /** GitHub token. Optional for public repositories, at a much lower rate limit. */
  token: string;
  /** Extra logins whose report comments are trusted. */
  trustedAuthors: string[];
}

const DEFAULTS: Settings = { token: "", trustedAuthors: [] };

export async function loadSettings(): Promise<Settings> {
  if (typeof chrome === "undefined" || !chrome.storage) return DEFAULTS;
  const stored = await chrome.storage.local.get(Object.keys(DEFAULTS));
  return { ...DEFAULTS, ...stored } as Settings;
}

export async function saveSettings(settings: Partial<Settings>): Promise<void> {
  await chrome.storage.local.set(settings);
}
