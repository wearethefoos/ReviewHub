import { describe, expect, it } from "vitest";
import type { z } from "zod";
import { ReviewManifest, SecurityReport } from "../schema";
import { demoBundle } from "../sources/demo";
import { addedPatch, changedFile } from "../sources/fixture-util";
import type { ChangedFile } from "../types";
import { detectRole, sensitiveArea } from "./classify";
import { analyzeFocus, tierFor } from "./score";

const code = (path: string, lines = 20): ChangedFile =>
  changedFile(path, "added", addedPatch(Array.from({ length: lines }, (_, i) => `line ${i + 1}`).join("\n")));

const manifest = (input: Partial<z.input<typeof ReviewManifest>>) =>
  ReviewManifest.parse({ version: "1", agent: { name: "test" }, prompt: { original: "do it" }, summary: "", ...input });

describe("detectRole", () => {
  it.each([
    ["package-lock.json", "generated"],
    ["src/__snapshots__/a.test.ts.snap", "generated"],
    ["src/a.test.ts", "test"],
    ["pkg/server_test.go", "test"],
    ["README.md", "docs"],
    [".github/workflows/ci.yml", "config"],
    ["src/server.ts", "core"],
  ])("%s is %s", (path, role) => {
    expect(detectRole(path)).toBe(role);
  });

  it("flags sensitive paths but not lookalikes", () => {
    expect(sensitiveArea("src/auth/session.ts")).toBe("authentication");
    expect(sensitiveArea("src/authors.ts")).toBeNull();
  });
});

describe("analyzeFocus", () => {
  it("ranks a critical security finding above everything else", () => {
    const security = SecurityReport.parse({
      version: "1",
      scanner: { name: "s" },
      findings: [{ id: "x", title: "SQLi", severity: "critical", description: "", file: "src/b.ts", lines: [2, 3], suggestion: { explanation: "" } }],
    });
    const result = analyzeFocus({ files: [code("src/a.ts", 200), code("src/b.ts", 5)], security });
    expect(result.files[0]!.file.path).toBe("src/b.ts");
    expect(result.files[0]!.tier).toBe("focus");
    expect(result.files[0]!.annotations).toEqual([expect.objectContaining({ kind: "security", start: 2, end: 3, severity: "critical" })]);
  });

  it("keeps generated files out of the way unless a security finding lands in them", () => {
    const result = analyzeFocus({ files: [code("package-lock.json", 2000)] });
    expect(result.files[0]!.tier).toBe("skip");
  });

  it("honours an agent's 'skip' hint only when nothing independent contradicts it", () => {
    const m = manifest({ files: [{ path: "src/pay/charge.ts", role: "mechanical", reviewHint: "skip" }, { path: "src/util.ts", role: "mechanical", reviewHint: "skip" }] });
    const result = analyzeFocus({ files: [code("src/pay/charge.ts"), code("src/util.ts")], manifest: m });
    const byPath = Object.fromEntries(result.files.map((f) => [f.file.path, f]));
    expect(byPath["src/util.ts"]!.tier).toBe("skip");
    // "charge" is a payments path, which the agent's hint cannot silence.
    expect(byPath["src/pay/charge.ts"]!.signals.some((s) => s.kind === "agent-hint" && s.label.includes("overridden"))).toBe(true);
  });

  it("raises 'must-review' files to focus", () => {
    const m = manifest({ files: [{ path: "src/x.ts", role: "core", reviewHint: "must-review" }] });
    expect(analyzeFocus({ files: [code("src/x.ts", 1)], manifest: m }).files[0]!.tier).toBe("focus");
  });

  it("annotates changed lines that coverage reports as missed", () => {
    const result = analyzeFocus({
      files: [code("src/x.ts", 10)],
      metrics: { version: "1", coverage: { files: [{ path: "src/x.ts", lines: 70, uncoveredLines: [4, 5, 6, 9, 50] }] } },
    });
    const file = result.files[0]!;
    expect(file.uncoveredChangedLines).toBe(4);
    expect(file.annotations.map((a) => [a.start, a.end])).toEqual([
      [4, 6],
      [9, 9],
    ]);
  });

  it("treats code files missing from a coverage report as untested, but not SQL", () => {
    const result = analyzeFocus({
      files: [code("src/x.ts"), code("db/migrate.sql")],
      metrics: { version: "1", coverage: { files: [] } },
    });
    const untested = result.files.filter((f) => f.signals.some((s) => s.label === "Not exercised by any test")).map((f) => f.file.path);
    expect(untested).toEqual(["src/x.ts"]);
  });

  it("reports references to files outside the diff", () => {
    const m = manifest({ gaps: [{ id: "g", title: "t", why: "", howToValidate: "", files: [{ path: "src/gone.ts" }] }] });
    expect(analyzeFocus({ files: [code("src/x.ts")], manifest: m }).danglingRefs).toEqual(["src/gone.ts"]);
  });

  it("sums totals per tier", () => {
    const result = analyzeFocus({ files: [code("src/a.ts", 10), code("package-lock.json", 30)] });
    expect(result.totals.all).toEqual({ files: 2, lines: 40 });
    expect(result.totals.skip.lines).toBe(30);
  });
});

describe("demo pull request", () => {
  it("validates and puts the risky files first and the noise last", () => {
    const bundle = demoBundle();
    const { manifest: m, security: s, metrics: x } = bundle.reports;
    const result = analyzeFocus({ files: bundle.files, manifest: m?.data, security: s?.data, metrics: x?.data });
    const tiers = Object.fromEntries(result.files.map((f) => [f.file.path, f.tier]));
    expect(tiers["src/payments/refunds.ts"]).toBe("focus");
    expect(tiers["src/api/routes/refunds.ts"]).toBe("focus");
    expect(tiers["src/payments/refunds.test.ts"]).toBe("skim");
    expect(tiers["package-lock.json"]).toBe("skip");
    expect(tiers["src/payments/webhooks.ts"]).toBe("skip");
    expect(result.danglingRefs).toEqual([]);
  });
});

describe("tierFor", () => {
  it("uses the thresholds", () => {
    expect([tierFor(0), tierFor(12), tierFor(40)]).toEqual(["skip", "skim", "focus"]);
  });
});
