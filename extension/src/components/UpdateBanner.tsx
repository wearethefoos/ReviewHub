// Tells users of an unpacked copy of the extension when a newer release is out
// (lib/updates.ts decides when).
import { XIcon } from "@primer/octicons-react";
import { useEffect, useState } from "react";
import { isExtension } from "@/lib/settings";
import { chromeUpdateEnv, dismissUpdate, pendingUpdate } from "@/lib/updates";
import { InlineMarkdown } from "./Markdown";

export function UpdateBanner() {
  const [notice, setNotice] = useState<{ text: string; tag: string } | null>(null);

  useEffect(() => {
    if (!isExtension()) return;
    let cancelled = false;
    pendingUpdate(chromeUpdateEnv())
      .then((pending) => {
        if (!cancelled) setNotice(pending);
      })
      .catch(() => {});
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
            void dismissUpdate(chromeUpdateEnv(), notice.tag);
            setNotice(null);
          }}
        >
          <XIcon size={16} />
        </button>
      </div>
    </div>
  );
}
