import { z } from "zod";

/**
 * Written by a separate security agent that did not author the change. Each
 * finding carries something a reviewer can act on directly: a patch to apply,
 * a prompt to hand to an agent, or both.
 */

export const Severity = z.enum(["critical", "high", "medium", "low", "info"]);
export type Severity = z.infer<typeof Severity>;

export const SecurityFinding = z.object({
  id: z.string().min(1),
  title: z.string().min(1),
  severity: Severity,
  category: z.string().optional().describe("For example a CWE id such as 'CWE-89'."),
  description: z.string(),
  file: z.string().min(1),
  lines: z.tuple([z.number().int().positive(), z.number().int().positive()]).optional(),
  confidence: z.enum(["low", "medium", "high"]).default("medium"),
  suggestion: z.object({
    explanation: z.string(),
    diff: z.string().optional().describe("Unified diff that fixes the finding, applicable with `git apply`."),
    prompt: z.string().optional().describe("Prompt that asks an agent to implement the fix or refactor."),
  }),
});
export type SecurityFinding = z.infer<typeof SecurityFinding>;

export const SecurityReport = z.object({
  version: z.literal("1"),
  scanner: z.object({ name: z.string().min(1), model: z.string().optional() }),
  commit: z.string().optional().describe("Head commit SHA the scan ran against."),
  summary: z.string().default(""),
  findings: z.array(SecurityFinding).default([]),
});
export type SecurityReport = z.infer<typeof SecurityReport>;
