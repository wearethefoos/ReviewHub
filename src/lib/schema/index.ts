import { z } from "zod";
import { ReviewManifest } from "./manifest";
import { MetricsReport } from "./metrics";
import { SecurityReport } from "./security";

export * from "./manifest";
export * from "./metrics";
export * from "./security";

export const REPORT_KINDS = ["manifest", "security", "metrics"] as const;
export type ReportKind = (typeof REPORT_KINDS)[number];

export const reportSchemas = {
  manifest: ReviewManifest,
  security: SecurityReport,
  metrics: MetricsReport,
} as const satisfies Record<ReportKind, z.ZodType>;

export type ReportOf<K extends ReportKind> = z.infer<(typeof reportSchemas)[K]>;

export type ParseResult<K extends ReportKind> =
  | { ok: true; data: ReportOf<K> }
  | { ok: false; error: string };

export function parseReport<K extends ReportKind>(kind: K, input: unknown): ParseResult<K> {
  const result = reportSchemas[kind].safeParse(input);
  if (result.success) return { ok: true, data: result.data as ReportOf<K> };
  return { ok: false, error: z.prettifyError(result.error) };
}
