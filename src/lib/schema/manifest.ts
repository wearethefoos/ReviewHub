import { z } from "zod";

/**
 * The review manifest is written by the agent that authored a pull request.
 * It captures the context a human reviewer needs but cannot get from the diff:
 * what was asked, where the agent went its own way, how it validated the work,
 * and what is still left to do.
 */

export const FileRef = z.object({
  path: z.string().min(1).describe("Repository-relative file path."),
  lines: z
    .tuple([z.number().int().positive(), z.number().int().positive()])
    .optional()
    .describe("Inclusive [start, end] line range in the new version of the file."),
});
export type FileRef = z.infer<typeof FileRef>;

const id = z.string().min(1).describe("Stable identifier, unique within its list.");

export const Deviation = z.object({
  id,
  title: z.string().min(1),
  requested: z.string().describe("What the prompt asked for."),
  delivered: z.string().describe("What was delivered instead."),
  reason: z.string().describe("Why the agent deviated."),
  severity: z.enum(["minor", "major"]).default("minor"),
  files: z.array(FileRef).default([]),
});
export type Deviation = z.infer<typeof Deviation>;

export const Alternative = z.object({
  title: z.string().min(1),
  description: z.string(),
  tradeoffs: z.string().optional(),
  prompt: z
    .string()
    .optional()
    .describe("A ready-to-use prompt that asks an agent to switch to this alternative."),
});
export type Alternative = z.infer<typeof Alternative>;

export const Decision = z.object({
  id,
  title: z.string().min(1),
  context: z.string().describe("The question that needed answering and why the prompt did not settle it."),
  chosen: z.string().describe("What the agent decided."),
  rationale: z.string(),
  impact: z
    .enum(["low", "medium", "high"])
    .default("low")
    .describe("How costly it is to get this wrong or change it later."),
  alternatives: z.array(Alternative).default([]),
  files: z.array(FileRef).default([]),
});
export type Decision = z.infer<typeof Decision>;

export const ValidationKind = z.enum([
  "unit-test",
  "integration-test",
  "e2e-test",
  "typecheck",
  "lint",
  "build",
  "manual",
  "benchmark",
  "other",
]);

export const Validation = z.object({
  id,
  kind: ValidationKind,
  description: z.string(),
  command: z.string().optional(),
  result: z.enum(["passed", "failed", "partial", "skipped"]),
  evidence: z.string().optional().describe("Short excerpt of output proving the result."),
  files: z.array(FileRef).default([]),
});
export type Validation = z.infer<typeof Validation>;

export const Owner = z.enum(["human", "agent", "ci", "ops"]);

export const Gap = z.object({
  id,
  title: z.string().min(1),
  why: z.string().describe("Why the agent could not validate this."),
  howToValidate: z.string().describe("Concrete steps for whoever closes the gap."),
  owner: Owner.default("human"),
  effort: z.enum(["small", "medium", "large"]).optional(),
  files: z.array(FileRef).default([]),
});
export type Gap = z.infer<typeof Gap>;

export const RolloutStep = z.object({
  id,
  title: z.string().min(1),
  description: z.string().default(""),
  owner: Owner.default("human"),
  when: z.enum(["before-merge", "after-merge", "after-deploy"]).default("before-merge"),
  blocking: z.boolean().default(false),
  prompt: z.string().optional().describe("Prompt to hand this step to an agent."),
});
export type RolloutStep = z.infer<typeof RolloutStep>;

export const FileRole = z.enum([
  "core",
  "supporting",
  "test",
  "config",
  "docs",
  "generated",
  "mechanical",
]);
export type FileRole = z.infer<typeof FileRole>;

export const FileNote = z.object({
  path: z.string().min(1),
  role: FileRole,
  note: z.string().optional(),
  reviewHint: z
    .enum(["must-review", "skim", "skip"])
    .optional()
    .describe("The agent's own suggestion. ReviewHub treats 'skip' as a claim, not a verdict."),
});
export type FileNote = z.infer<typeof FileNote>;

export const ReviewManifest = z.object({
  version: z.literal("1"),
  agent: z.object({
    name: z.string().min(1),
    model: z.string().optional(),
    sessionUrl: z.string().url().optional(),
  }),
  prompt: z.object({
    original: z.string().min(1).describe("The user's request, verbatim."),
    followUps: z.array(z.string()).default([]).describe("Later user messages that changed the scope."),
    source: z.string().url().optional().describe("Link to the issue or conversation the prompt came from."),
  }),
  summary: z.string().describe("What was delivered, in a few sentences."),
  deviations: z.array(Deviation).default([]),
  decisions: z.array(Decision).default([]),
  validation: z.array(Validation).default([]),
  gaps: z.array(Gap).default([]),
  rollout: z.array(RolloutStep).default([]),
  files: z.array(FileNote).default([]),
});
export type ReviewManifest = z.infer<typeof ReviewManifest>;
