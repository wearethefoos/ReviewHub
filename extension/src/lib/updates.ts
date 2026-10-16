// Extension copies loaded unpacked from a release zip never update themselves
// (Chrome Web Store installs do). Works out what to tell their users.
import { updateNotice } from "./core";

const DAY = 24 * 60 * 60 * 1000;
const LATEST = "https://api.github.com/repos/wearethefoos/ReviewHub/releases/latest";

interface Cached {
  tag: string;
  checked: number;
}

/** What the update check needs from the browser; `chrome.*` in the extension, fakes in tests. */
export interface UpdateEnv {
  installType: () => Promise<string>;
  version: string;
  storage: { get: (key: string) => Promise<Record<string, unknown>>; set: (items: Record<string, unknown>) => Promise<void> };
  fetch: typeof fetch;
  now: () => number;
}

export function chromeUpdateEnv(): UpdateEnv {
  return {
    installType: async () => (await chrome.management.getSelf()).installType,
    version: chrome.runtime.getManifest().version,
    storage: chrome.storage.local,
    fetch: (...args) => fetch(...args),
    now: Date.now,
  };
}

/** The latest release tag, asked of GitHub (anonymously) at most once a day. Offline, an older answer will do. */
async function latestRelease(env: UpdateEnv): Promise<string | null> {
  const cached = (await env.storage.get("latestRelease")).latestRelease as Cached | undefined;
  if (cached && env.now() - cached.checked < DAY) return cached.tag;
  try {
    const res = await env.fetch(LATEST, { headers: { Accept: "application/vnd.github+json" } });
    const tag = res.ok ? ((await res.json()) as { tag_name?: unknown }).tag_name : undefined;
    if (typeof tag !== "string" || !tag) return cached?.tag ?? null;
    await env.storage.set({ latestRelease: { tag, checked: env.now() } satisfies Cached });
    return tag;
  } catch {
    return cached?.tag ?? null;
  }
}

/** The update notice (Markdown) for an unpacked copy with a newer release out that the user hasn't dismissed. */
export async function pendingUpdate(env: UpdateEnv): Promise<{ text: string; tag: string } | null> {
  if ((await env.installType()) !== "development") return null;
  const tag = await latestRelease(env);
  if (!tag) return null;
  if ((await env.storage.get("dismissedUpdate")).dismissedUpdate === tag) return null;
  const text = await updateNotice("extension-unpacked", env.version, tag);
  return text ? { text, tag } : null;
}

/** Hides the notice for `tag` until a newer release comes out. */
export function dismissUpdate(env: UpdateEnv, tag: string): Promise<void> {
  return env.storage.set({ dismissedUpdate: tag });
}
