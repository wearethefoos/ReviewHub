import { parseReport, REPORT_KINDS, type ReportKind, type ReportOf } from "../schema";

/**
 * Agents publish reports as pull request comments that start with a marker
 * and contain the report as a fenced JSON block:
 *
 *     <!-- reviewhub:manifest -->
 *     ```json
 *     { "version": "1", ... }
 *     ```
 *
 * The JSON may be wrapped in a <details> block to keep the comment readable on GitHub.
 */

const MARKER = /<!--\s*reviewhub:(manifest|security|metrics)\s*-->/;
const JSON_FENCE = /```json\s*\n([\s\S]*?)\n```/;

export function markerFor(kind: ReportKind): string {
  return `<!-- reviewhub:${kind} -->`;
}

export function detectReportKind(body: string): ReportKind | null {
  const match = MARKER.exec(body);
  const kind = match?.[1] as ReportKind | undefined;
  return kind && REPORT_KINDS.includes(kind) ? kind : null;
}

export type ExtractResult<K extends ReportKind> = { kind: K; data?: ReportOf<K>; error?: string };

export function extractReport(body: string): ExtractResult<ReportKind> | null {
  const kind = detectReportKind(body);
  if (!kind) return null;
  const fence = JSON_FENCE.exec(body.slice(body.search(MARKER)));
  if (!fence?.[1]) return { kind, error: "Marker found but no ```json block follows it." };

  let json: unknown;
  try {
    json = JSON.parse(fence[1]);
  } catch (err) {
    return { kind, error: `Invalid JSON: ${(err as Error).message}` };
  }
  const parsed = parseReport(kind, json);
  return parsed.ok ? { kind, data: parsed.data } : { kind, error: parsed.error };
}

/** Renders a report as a comment body that `extractReport` can read back. */
export function renderReportComment(kind: ReportKind, report: unknown, summary?: string): string {
  const title = { manifest: "Review manifest", security: "Security review", metrics: "Coverage and quality metrics" }[kind];
  return [
    markerFor(kind),
    `**ReviewHub ${title.toLowerCase()}**${summary ? `: ${summary}` : ""}`,
    "",
    "<details><summary>Report data</summary>",
    "",
    "```json",
    JSON.stringify(report, null, 2),
    "```",
    "",
    "</details>",
  ].join("\n");
}
