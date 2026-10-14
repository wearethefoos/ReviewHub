import { describe, expect, it } from "vitest";
import { addedLines, parsePatch, splitUnifiedDiff } from "./parse";

describe("parsePatch", () => {
  it("tracks old and new line numbers across adds, deletes and context", () => {
    const hunks = parsePatch(["@@ -10,3 +10,4 @@ fn()", " a", "-b", "+B", "+C", " d"].join("\n"));
    expect(hunks).toHaveLength(1);
    expect(hunks[0]!.lines).toEqual([
      { type: "ctx", text: "a", oldLine: 10, newLine: 10 },
      { type: "del", text: "b", oldLine: 11, newLine: null },
      { type: "add", text: "B", oldLine: null, newLine: 11 },
      { type: "add", text: "C", oldLine: null, newLine: 12 },
      { type: "ctx", text: "d", oldLine: 12, newLine: 13 },
    ]);
  });

  it("handles multiple hunks, no-newline markers and a trailing newline", () => {
    const hunks = parsePatch(["@@ -1 +1 @@", "-x", "+y", "\\ No newline at end of file", "@@ -20,1 +20,1 @@", " z", ""].join("\n"));
    expect(hunks.map((h) => h.lines.length)).toEqual([2, 1]);
    expect(hunks[1]!.lines[0]).toMatchObject({ oldLine: 20, newLine: 20 });
  });

  it("returns nothing for missing patches", () => {
    expect(parsePatch(undefined)).toEqual([]);
    expect(parsePatch("")).toEqual([]);
  });

  it("lists added lines on the new side", () => {
    expect(addedLines(parsePatch("@@ -0,0 +1,3 @@\n+a\n+b\n+c"))).toEqual([1, 2, 3]);
  });
});

describe("splitUnifiedDiff", () => {
  it("splits a multi-file diff and strips the b/ prefix", () => {
    const diff = ["--- a/x.ts", "+++ b/x.ts", "@@ -1 +1 @@", "-a", "+b", "diff --git a/y.ts b/y.ts", "--- a/y.ts", "+++ b/y.ts", "@@ -5 +5 @@", "-c", "+d"].join("\n");
    const files = splitUnifiedDiff(diff);
    expect(files.map((f) => f.path)).toEqual(["x.ts", "y.ts"]);
    expect(files[1]!.hunks[0]!.lines[1]).toMatchObject({ type: "add", text: "d", newLine: 5 });
  });
});
