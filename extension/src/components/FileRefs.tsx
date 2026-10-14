import { FileIcon } from "@primer/octicons-react";
import { Link } from "@/components/Link";
import { fileAnchor, tabHref } from "@/lib/router";
import type { FileRef } from "@/lib/types";
import type { PullRequestInfo } from "@/lib/types";

export function FileRefs({ refs, pr }: { refs: FileRef[]; pr: PullRequestInfo }) {
  if (refs.length === 0) return null;
  return (
    <ul className="flex flex-wrap gap-2">
      {refs.map((r, i) => (
        <li key={i}>
          <Link
            href={tabHref(pr, "changes", fileAnchor(r.path))}
            className="inline-flex items-center gap-1 rounded-md border border-border bg-canvas-subtle px-2 py-0.5 font-mono text-xs text-fg hover:border-accent hover:text-accent"
          >
            <FileIcon size={12} />
            {r.path}
            {r.lines && <span className="text-fg-muted">:{r.lines[0] === r.lines[1] ? r.lines[0] : `${r.lines[0]}-${r.lines[1]}`}</span>}
          </Link>
        </li>
      ))}
    </ul>
  );
}
