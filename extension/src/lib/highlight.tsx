// Syntax highlighting for diffs and Markdown code blocks, with highlight.js
// grammars (through lowlight) and GitHub's colours (the --syntax-* tokens in
// globals.css). Highlighting only adds spans with class names to the text; it
// never renders HTML from the input.
import type { Element, ElementContent, Root, RootContent } from "hast";
import { toJsxRuntime } from "hast-util-to-jsx-runtime";
import { common, createLowlight } from "lowlight";
import type { ReactNode } from "react";
import { Fragment, jsx, jsxs } from "react/jsx-runtime";
import type { DiffHunk } from "./types";

export const lowlight = createLowlight(common);
export const languages = common;

/** Larger diffs are shown without highlighting, to keep the page responsive. */
const MAX_LINES = 5000;

const FILE_NAMES: Record<string, string> = { dockerfile: "dockerfile", makefile: "makefile", gemfile: "ruby", rakefile: "ruby" };

/** The highlight.js language for a file path, if it has one. */
export function languageFor(path: string): string | undefined {
  const name = path.split("/").pop()?.toLowerCase() ?? "";
  if (FILE_NAMES[name]) return FILE_NAMES[name];
  const ext = name.includes(".") ? name.split(".").pop()! : "";
  return ext && lowlight.registered(ext) ? ext : undefined;
}

/** Splits highlighted output into one node list per source line, reopening spans that cross line breaks. */
function splitLines(root: Root): ElementContent[][] {
  const lines: ElementContent[][] = [[]];
  function walk(nodes: (RootContent | ElementContent)[], wrap: (children: ElementContent[]) => ElementContent[]) {
    for (const node of nodes) {
      if (node.type === "text") {
        node.value.split("\n").forEach((part, i) => {
          if (i > 0) lines.push([]);
          if (part) lines[lines.length - 1]!.push(...wrap([{ type: "text", value: part }]));
        });
      } else if (node.type === "element") {
        const el = node as Element;
        walk(el.children, (children) => wrap([{ ...el, children }]));
      }
    }
  }
  walk(root.children, (children) => children);
  return lines;
}

function render(nodes: ElementContent[]): ReactNode {
  return toJsxRuntime({ type: "root", children: nodes }, { Fragment, jsx, jsxs });
}

/** Highlights consecutive source lines together (so multi-line strings and comments are right), returning one node per line. */
export function highlightLines(lines: string[], language: string | undefined): ReactNode[] | null {
  if (!language || lines.length === 0) return null;
  try {
    const split = splitLines(lowlight.highlight(language, lines.join("\n")));
    return lines.map((_, i) => render(split[i] ?? []));
  } catch {
    return null;
  }
}

/**
 * Highlighted text for every line of a file's hunks, as `[hunk][line]`. Each
 * side of a hunk is highlighted as a whole: unchanged and added lines as the
 * new version, unchanged and deleted lines as the old one.
 */
export function highlightHunks(hunks: DiffHunk[], path: string): (ReactNode | null)[][] | null {
  const language = languageFor(path);
  const total = hunks.reduce((n, h) => n + h.lines.length, 0);
  if (!language || total > MAX_LINES) return null;
  return hunks.map((hunk) => {
    const newSide = hunk.lines.filter((l) => l.kind !== "del");
    const oldSide = hunk.lines.filter((l) => l.kind !== "add");
    const newHtml = highlightLines(newSide.map((l) => l.text), language);
    const oldHtml = highlightLines(oldSide.map((l) => l.text), language);
    let n = 0;
    let o = 0;
    return hunk.lines.map((l) => {
      if (l.kind === "del") return oldHtml?.[o++] ?? null;
      if (l.kind === "ctx") o++;
      return newHtml?.[n++] ?? null;
    });
  });
}
