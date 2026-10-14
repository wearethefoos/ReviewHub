import { CheckCircleFillIcon, CircleIcon, XCircleFillIcon } from "@primer/octicons-react";
import type { ReportKind } from "@/lib/types";
import type { PullRequestBundle, ReportEnvelope } from "@/lib/types";
import { Code } from "./ui";

const TITLES: Record<ReportKind, string> = {
  manifest: "Review manifest",
  security: "Security report",
  metrics: "Coverage & quality",
};

function Source({ envelope }: { envelope: ReportEnvelope<unknown> }) {
  const s = envelope.source;
  if (s.type === "fixture") return <>demo data</>;
  if (s.type === "file") return <Code>{s.path}</Code>;
  return (
    <a href={s.url} className="hover:underline">
      comment by {s.author}
    </a>
  );
}

/** Sidebar: which reports were found, where they came from, and whether they validated. */
export function ReportSources({ reports }: { reports: PullRequestBundle["reports"] }) {
  return (
    <div>
      <h3 className="mb-2 text-xs font-semibold text-fg-muted">Reports</h3>
      <ul className="space-y-3 text-sm">
        {(Object.keys(TITLES) as ReportKind[]).map((kind) => {
          const envelope = reports[kind] as ReportEnvelope<unknown> | undefined;
          return (
            <li key={kind} className="flex gap-2">
              {!envelope ? (
                <CircleIcon size={16} className="mt-0.5 shrink-0 text-fg-muted" />
              ) : envelope.error ? (
                <XCircleFillIcon size={16} className="mt-0.5 shrink-0 text-danger" />
              ) : (
                <CheckCircleFillIcon size={16} className="mt-0.5 shrink-0 text-success" />
              )}
              <div className="min-w-0">
                <p className="font-medium">{TITLES[kind]}</p>
                <p className="text-xs text-fg-muted">{envelope ? <Source envelope={envelope} /> : "not published"}</p>
                {envelope?.error && (
                  <details className="mt-1">
                    <summary className="text-xs text-danger">Invalid report</summary>
                    <pre className="mt-1 whitespace-pre-wrap font-mono text-xs text-fg-muted">{envelope.error}</pre>
                  </details>
                )}
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
