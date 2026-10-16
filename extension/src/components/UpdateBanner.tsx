// Extension copies loaded unpacked from a release zip never update themselves
// (Chrome Web Store installs do). Tells their users when a newer release is out.
import { XIcon } from "@primer/octicons-react";
import { useEffect, useState } from "react";
import { updateNotice } from "@/lib/core";
import { isExtension } from "@/lib/settings";
import { InlineMarkdown } from "./Markdown";

const DAY = 24 * 60 * 60 * 1000;

interface Cached {
  tag: string;
  checked: number;
}

/** The latest release tag, checked on GitHub at most once a day. */
async function latestRelease(): Promise<string | null> {
  const { latestRelease: cached } = (await chrome.storage.local.get("latestRelease")) as { latestRelease?: Cached };
  if (cached && Date.now() - cached.checked < DAY) return cached.tag;
  try {
    const res = await fetch("https://api.github.com/repos/wearethefoos/ReviewHub/releases/latest", { headers: { Accept: "application/vnd.github+json" } });
    if (!res.ok) return cached?.tag ?? null;
    const tag = ((await res.json()) as { tag_name?: string }).tag_name;
    if (!tag) return cached?.tag ?? null;
    await chrome.storage.local.set({ latestRelease: { tag, checked: Date.now() } satisfies Cached });
    return tag;
  } catch {
    return cached?.tag ?? null;
  }
}

export function UpdateBanner() {
  const [notice, setNotice] = useState<{ text: string; tag: string } | null>(null);

  useEffect(() => {
    if (!isExtension()) return;
    let cancelled = false;
    (async () => {
      const self = await chrome.management.getSelf();
      if (self.installType !== "development") return;
      const tag = await latestRelease();
      if (!tag) return;
      const { dismissedUpdate } = (await chrome.storage.local.get("dismissedUpdate")) as { dismissedUpdate?: string };
      if (dismissedUpdate === tag) return;
      const text = await updateNotice("extension-unpacked", chrome.runtime.getManifest().version, tag);
      if (text && !cancelled) setNotice({ text, tag });
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  if (!notice) return null;
  return (
    <div className="border-b border-attention/40 bg-attention-subtle">
      <div className="mx-auto flex max-w-[1280px] items-start gap-3 px-4 py-2 text-sm md:px-6">
        <p className="min-w-0 flex-1">
          <InlineMarkdown>{notice.text}</InlineMarkdown>
        </p>
        <button
          type="button"
          aria-label="Dismiss until the next release"
          title="Dismiss until the next release"
          className="rounded p-1 text-fg-muted hover:bg-neutral-subtle"
          onClick={() => {
            void chrome.storage.local.set({ dismissedUpdate: notice.tag });
            setNotice(null);
          }}
        >
          <XIcon size={16} />
        </button>
      </div>
    </div>
  );
}
