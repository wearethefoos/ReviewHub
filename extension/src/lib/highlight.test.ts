import { createElement, Fragment } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { highlightHunks, highlightLines, languageFor } from "./highlight";
import type { DiffHunk } from "./types";

const html = (node: unknown) => renderToStaticMarkup(createElement(Fragment, null, node as never));

describe("highlighting", () => {
  it("picks languages from paths", () => {
    expect(languageFor("src/a.ts")).toBe("ts");
    expect(languageFor("crates/core/src/lib.rs")).toBe("rs");
    expect(languageFor("Dockerfile")).toBe("dockerfile");
    expect(languageFor("notes.unknownext")).toBeUndefined();
  });

  it("keeps multi-line comments highlighted on every line, and escapes text", () => {
    const lines = highlightLines(["/* start", "<b>middle</b>", "end */ const x = 1;"], "ts")!.map(html);
    expect(lines[1]).toBe('<span class="hljs-comment">&lt;b&gt;middle&lt;/b&gt;</span>');
    expect(lines[2]).toContain('<span class="hljs-keyword">const</span>');
  });

  it("highlights each side of a hunk as a whole", () => {
    const hunk: DiffHunk = {
      header: "@@",
      lines: [
        { kind: "ctx", text: "const a = `x", oldLine: 1, newLine: 1 },
        { kind: "del", text: "old`;", oldLine: 2, newLine: null },
        { kind: "add", text: "new`;", oldLine: null, newLine: 2 },
      ],
    };
    const [rows] = highlightHunks([hunk], "a.ts")!;
    expect(html(rows![1])).toContain('<span class="hljs-string">old`</span>');
    expect(html(rows![2])).toContain('<span class="hljs-string">new`</span>');
  });
});
