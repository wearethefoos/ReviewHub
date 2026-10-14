import { addedLines, parsePatch, type DiffHunk } from "../diff/parse";
import type { FileNote, FileRef, FileRole, MetricsReport, ReviewManifest, SecurityReport, Severity } from "../schema";
import type { ChangedFile } from "../types";
import { detectRole, isCodeFile, sensitiveArea } from "./classify";

export type Tier = "focus" | "skim" | "skip";

export type SignalKind =
  | "security"
  | "decision"
  | "deviation"
  | "gap"
  | "validation"
  | "coverage"
  | "complexity"
  | "sensitive"
  | "size"
  | "role"
  | "agent-hint";

export interface Signal {
  kind: SignalKind;
  label: string;
  /** Contribution to the score. Zero for purely informational signals. */
  weight: number;
  /** Anchor of the related item on another tab, e.g. "decision-d1". */
  ref?: string;
}

export type AnnotationKind = "security" | "decision" | "deviation" | "gap" | "uncovered";

export interface LineAnnotation {
  kind: AnnotationKind;
  start: number;
  end: number;
  title: string;
  detail?: string;
  severity?: Severity;
  ref?: string;
}

export interface FileFocus {
  file: ChangedFile;
  hunks: DiffHunk[];
  role: FileRole;
  roleSource: "agent" | "detected";
  note?: string;
  score: number;
  tier: Tier;
  signals: Signal[];
  annotations: LineAnnotation[];
  uncoveredChangedLines: number;
}

export interface TierTotals {
  files: number;
  lines: number;
}

export interface FocusAnalysis {
  files: FileFocus[];
  totals: Record<Tier | "all", TierTotals>;
  /** Paths the reports reference that are not part of the diff. Usually a sign of a stale report. */
  danglingRefs: string[];
}

export interface FocusInput {
  files: ChangedFile[];
  manifest?: ReviewManifest;
  security?: SecurityReport;
  metrics?: MetricsReport;
}

export const TIER_THRESHOLDS = { focus: 40, skim: 12 } as const;

const ROLE_BASE: Record<FileRole, number> = {
  core: 10,
  supporting: 6,
  config: 6,
  test: 10,
  mechanical: 1,
  docs: 1,
  generated: 0,
};

const SECURITY_WEIGHT: Record<Severity, number> = { critical: 60, high: 45, medium: 25, low: 10, info: 2 };
const DECISION_WEIGHT = { high: 30, medium: 15, low: 5 } as const;
const DEVIATION_WEIGHT = { major: 30, minor: 12 } as const;
const GAP_WEIGHT = 18;
const FAILED_VALIDATION_WEIGHT = 25;

/** Signal kinds that point at real risk, as opposed to the file's shape. */
const RISK_KINDS = new Set<SignalKind>(["security", "decision", "deviation", "gap", "validation", "coverage", "complexity", "sensitive"]);

export function tierFor(score: number): Tier {
  if (score >= TIER_THRESHOLDS.focus) return "focus";
  if (score >= TIER_THRESHOLDS.skim) return "skim";
  return "skip";
}

function refsFor(refs: FileRef[], path: string): FileRef[] {
  return refs.filter((r) => r.path === path);
}

function collapseRanges(lines: number[]): [number, number][] {
  const sorted = [...new Set(lines)].sort((a, b) => a - b);
  const ranges: [number, number][] = [];
  for (const n of sorted) {
    const last = ranges.at(-1);
    if (last && n === last[1] + 1) last[1] = n;
    else ranges.push([n, n]);
  }
  return ranges;
}

function analyzeFile(file: ChangedFile, input: FocusInput, notes: Map<string, FileNote>): FileFocus {
  const { manifest, security, metrics } = input;
  const hunks = parsePatch(file.patch);
  const note = notes.get(file.path);
  const detected = detectRole(file.path);
  const role = note?.role ?? detected;
  const signals: Signal[] = [];
  const annotations: LineAnnotation[] = [];
  const churn = file.additions + file.deletions;

  signals.push({ kind: "role", label: `${role} file${note ? " (per agent)" : ""}`, weight: ROLE_BASE[role] });

  if (role !== "generated" && role !== "mechanical") {
    const sizeWeight = Math.min(15, Math.round(Math.sqrt(churn) * (role === "test" || role === "docs" ? 0.5 : 1)));
    if (sizeWeight > 0) signals.push({ kind: "size", label: `${churn} changed lines`, weight: sizeWeight });
  }

  const area = sensitiveArea(file.path);
  if (area && role !== "test" && role !== "docs") {
    signals.push({ kind: "sensitive", label: `Touches ${area}`, weight: 10 });
  }

  for (const finding of security?.findings ?? []) {
    if (finding.file !== file.path) continue;
    const ref = `security-${finding.id}`;
    signals.push({ kind: "security", label: `${finding.severity} security finding: ${finding.title}`, weight: SECURITY_WEIGHT[finding.severity], ref });
    if (finding.lines) {
      annotations.push({ kind: "security", start: finding.lines[0], end: finding.lines[1], title: finding.title, detail: finding.description, severity: finding.severity, ref });
    }
  }

  for (const decision of manifest?.decisions ?? []) {
    const refs = refsFor(decision.files, file.path);
    if (refs.length === 0) continue;
    const ref = `decision-${decision.id}`;
    signals.push({ kind: "decision", label: `${decision.impact}-impact decision: ${decision.title}`, weight: DECISION_WEIGHT[decision.impact], ref });
    for (const r of refs) {
      if (r.lines) annotations.push({ kind: "decision", start: r.lines[0], end: r.lines[1], title: decision.title, detail: decision.chosen, ref });
    }
  }

  for (const deviation of manifest?.deviations ?? []) {
    const refs = refsFor(deviation.files, file.path);
    if (refs.length === 0) continue;
    const ref = `deviation-${deviation.id}`;
    signals.push({ kind: "deviation", label: `${deviation.severity} deviation: ${deviation.title}`, weight: DEVIATION_WEIGHT[deviation.severity], ref });
    for (const r of refs) {
      if (r.lines) annotations.push({ kind: "deviation", start: r.lines[0], end: r.lines[1], title: deviation.title, detail: deviation.reason, ref });
    }
  }

  for (const gap of manifest?.gaps ?? []) {
    const refs = refsFor(gap.files, file.path);
    if (refs.length === 0) continue;
    const ref = `gap-${gap.id}`;
    signals.push({ kind: "gap", label: `Not validated: ${gap.title}`, weight: GAP_WEIGHT, ref });
    for (const r of refs) {
      if (r.lines) annotations.push({ kind: "gap", start: r.lines[0], end: r.lines[1], title: gap.title, detail: gap.why, ref });
    }
  }

  for (const validation of manifest?.validation ?? []) {
    if (validation.result === "passed" || validation.result === "skipped") continue;
    if (refsFor(validation.files, file.path).length === 0) continue;
    signals.push({ kind: "validation", label: `Validation ${validation.result}: ${validation.description}`, weight: FAILED_VALIDATION_WEIGHT, ref: `validation-${validation.id}` });
  }

  let uncoveredChangedLines = 0;
  const coverage = metrics?.coverage;
  if (coverage && (role === "core" || role === "supporting") && isCodeFile(file.path)) {
    const entry = coverage.files.find((f) => f.path === file.path);
    if (!entry) {
      if (file.status !== "removed" && file.additions > 0) {
        signals.push({ kind: "coverage", label: "Not exercised by any test", weight: 15 });
      }
    } else {
      const added = new Set(addedLines(hunks));
      const uncovered = entry.uncoveredLines.filter((n) => added.has(n));
      uncoveredChangedLines = uncovered.length;
      if (uncovered.length > 0) {
        signals.push({ kind: "coverage", label: `${uncovered.length} changed lines not covered by tests`, weight: Math.min(25, uncovered.length * 2) });
        for (const [start, end] of collapseRanges(uncovered)) {
          annotations.push({ kind: "uncovered", start, end, title: "Not covered by tests" });
        }
      } else {
        signals.push({ kind: "coverage", label: `All changed lines covered (${entry.lines.toFixed(0)}% of file)`, weight: 0 });
      }
    }
  }

  const quality = metrics?.quality?.files.find((f) => f.path === file.path);
  if (quality?.complexityDelta && quality.complexityDelta > 0) {
    signals.push({ kind: "complexity", label: `Complexity +${quality.complexityDelta}`, weight: Math.min(20, quality.complexityDelta * 2) });
  }
  if (quality?.lintIssues) {
    signals.push({ kind: "complexity", label: `${quality.lintIssues} lint issues`, weight: Math.min(10, quality.lintIssues * 2) });
  }

  let score = signals.reduce((sum, s) => sum + s.weight, 0);
  const riskSignals = signals.filter((s) => RISK_KINDS.has(s.kind) && s.weight > 0);
  const hasSecurity = signals.some((s) => s.kind === "security");

  // Generated files only surface when the security agent found something in them.
  if (role === "generated" && !hasSecurity) score = Math.min(score, 5);

  // The agent's own hints: "must-review" always surfaces, "skip" only holds if
  // nothing independent of the agent's judgement says otherwise.
  if (note?.reviewHint === "must-review" && score < TIER_THRESHOLDS.focus) {
    signals.push({ kind: "agent-hint", label: "Agent asked for a careful review", weight: TIER_THRESHOLDS.focus - score });
    score = TIER_THRESHOLDS.focus;
  } else if (note?.reviewHint === "skip") {
    const independent = riskSignals.filter((s) => s.kind !== "decision" && s.kind !== "deviation" && s.kind !== "gap");
    if (independent.length === 0) {
      const capped = Math.min(score, TIER_THRESHOLDS.skim - 1);
      signals.push({ kind: "agent-hint", label: "Agent marked this as safe to skip", weight: capped - score });
      score = capped;
    } else {
      signals.push({ kind: "agent-hint", label: `Agent suggested skipping, overridden by: ${independent.map((s) => s.kind).join(", ")}`, weight: 0 });
    }
  }

  score = Math.max(0, Math.min(100, Math.round(score)));
  annotations.sort((a, b) => a.start - b.start);
  signals.sort((a, b) => b.weight - a.weight);

  return {
    file,
    hunks,
    role,
    roleSource: note ? "agent" : "detected",
    note: note?.note,
    score,
    tier: tierFor(score),
    signals,
    annotations,
    uncoveredChangedLines,
  };
}

const TIER_ORDER: Record<Tier, number> = { focus: 0, skim: 1, skip: 2 };

export function analyzeFocus(input: FocusInput): FocusAnalysis {
  const notes = new Map((input.manifest?.files ?? []).map((n) => [n.path, n]));
  const files = input.files
    .map((f) => analyzeFile(f, input, notes))
    .sort(
      (a, b) =>
        TIER_ORDER[a.tier] - TIER_ORDER[b.tier] ||
        b.score - a.score ||
        // Plain code-unit order, so the Rust CLI sorts identically.
        (a.file.path < b.file.path ? -1 : a.file.path > b.file.path ? 1 : 0),
    );

  const totals: FocusAnalysis["totals"] = {
    all: { files: 0, lines: 0 },
    focus: { files: 0, lines: 0 },
    skim: { files: 0, lines: 0 },
    skip: { files: 0, lines: 0 },
  };
  for (const f of files) {
    const lines = f.file.additions + f.file.deletions;
    for (const key of ["all", f.tier] as const) {
      totals[key].files += 1;
      totals[key].lines += lines;
    }
  }

  const changed = new Set(input.files.map((f) => f.path));
  const referenced = [
    ...(input.manifest?.decisions ?? []).flatMap((d) => d.files.map((r) => r.path)),
    ...(input.manifest?.deviations ?? []).flatMap((d) => d.files.map((r) => r.path)),
    ...(input.manifest?.gaps ?? []).flatMap((g) => g.files.map((r) => r.path)),
    ...(input.security?.findings ?? []).map((f) => f.file),
  ];
  const danglingRefs = [...new Set(referenced.filter((p) => !changed.has(p)))].sort();

  return { files, totals, danglingRefs };
}
