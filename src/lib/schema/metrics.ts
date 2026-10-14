import { z } from "zod";

/**
 * Coverage and code quality numbers, usually produced in CI. Use the
 * `reviewhub metrics` CLI to build this from an lcov report.
 */

const percent = z.number().min(0).max(100);

export const FileCoverage = z.object({
  path: z.string().min(1),
  lines: percent.describe("Line coverage of the whole file, in percent."),
  branches: percent.optional(),
  uncoveredLines: z
    .array(z.number().int().positive())
    .default([])
    .describe("Executable lines with zero hits in the new version of the file."),
});
export type FileCoverage = z.infer<typeof FileCoverage>;

export const FileQuality = z.object({
  path: z.string().min(1),
  complexity: z.number().nonnegative().optional().describe("Cyclomatic complexity of the file after the change."),
  complexityDelta: z.number().optional().describe("Change in complexity introduced by this pull request."),
  duplication: percent.optional(),
  lintIssues: z.number().int().nonnegative().optional(),
});
export type FileQuality = z.infer<typeof FileQuality>;

export const MetricsReport = z.object({
  version: z.literal("1"),
  commit: z.string().optional(),
  coverage: z
    .object({
      tool: z.string().optional(),
      lines: percent.optional().describe("Overall line coverage."),
      baseLines: percent.optional().describe("Overall line coverage on the base branch, if known."),
      files: z.array(FileCoverage).default([]),
    })
    .optional(),
  quality: z
    .object({
      tool: z.string().optional(),
      files: z.array(FileQuality).default([]),
    })
    .optional(),
});
export type MetricsReport = z.infer<typeof MetricsReport>;
