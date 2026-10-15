// Settings. In the extension they live in chrome.storage.local. In the web app
// (GitHub Pages) the token stays in this tab's sessionStorage unless the user
// asks to remember it, because a github.io origin is shared with the owner's
// other Pages sites. Either way it is only ever sent to api.github.com.

export interface Settings {
  /** GitHub token. Optional for public repositories, at a much lower rate limit. */
  token: string;
  /** Extra logins whose report comments are trusted. */
  trustedAuthors: string[];
  /** Web app only: keep the token in localStorage across visits. */
  rememberToken: boolean;
}

const DEFAULTS: Settings = { token: "", trustedAuthors: [], rememberToken: false };
const WEB_KEY = "reviewhub:settings";

/** True inside the browser extension, false in the web app. */
export function isExtension(): boolean {
  return typeof chrome !== "undefined" && Boolean(chrome.runtime?.id) && Boolean(chrome.storage?.local);
}

function readWeb(storage: Storage): Partial<Settings> {
  try {
    return JSON.parse(storage.getItem(WEB_KEY) ?? "{}");
  } catch {
    return {};
  }
}

export async function loadSettings(): Promise<Settings> {
  if (isExtension()) {
    const stored = await chrome.storage.local.get(["token", "trustedAuthors"]);
    return { ...DEFAULTS, ...stored } as Settings;
  }
  if (typeof localStorage === "undefined") return DEFAULTS;
  const persisted = readWeb(localStorage);
  const session = readWeb(sessionStorage);
  return { ...DEFAULTS, ...persisted, ...session };
}

export async function saveSettings(settings: Partial<Settings>): Promise<void> {
  if (isExtension()) {
    const { token, trustedAuthors } = settings;
    await chrome.storage.local.set({ ...(token !== undefined && { token }), ...(trustedAuthors && { trustedAuthors }) });
    return;
  }
  const next = { ...(await loadSettings()), ...settings };
  try {
    sessionStorage.setItem(WEB_KEY, JSON.stringify(next));
    if (next.rememberToken) localStorage.setItem(WEB_KEY, JSON.stringify(next));
    else localStorage.setItem(WEB_KEY, JSON.stringify({ ...next, token: "" }));
  } catch {
    // Storage blocked (private mode): settings last for this page only.
  }
}

/** Opens the settings: the extension's options page, or the web app's settings page. */
export function openSettings(event?: { preventDefault(): void }) {
  if (isExtension()) {
    event?.preventDefault();
    chrome.runtime.openOptionsPage();
  }
  // In the web app the link's own href (options.html) navigates there.
}

export const EXTENSION_URL = "https://chromewebstore.google.com/detail/reviewhub/eoejgblmpnccaaohifkeajpblaidgnjo";
