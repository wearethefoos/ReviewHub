"use client";

import { CheckIcon, CopyIcon } from "@primer/octicons-react";
import { useState } from "react";

export function CopyButton({ text, label = "Copy", className }: { text: string; label?: string; className?: string }) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard access can be denied (e.g. insecure context); nothing useful to do.
    }
  }

  return (
    <button
      type="button"
      onClick={copy}
      className={
        "inline-flex items-center gap-1.5 rounded-md border border-border bg-canvas-subtle px-2.5 py-1 text-xs font-medium text-fg hover:bg-neutral-subtle " +
        (className ?? "")
      }
    >
      {copied ? <CheckIcon size={14} className="text-success" /> : <CopyIcon size={14} className="text-fg-muted" />}
      {copied ? "Copied" : label}
    </button>
  );
}
