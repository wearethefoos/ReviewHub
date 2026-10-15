import {
  AlertIcon,
  BeakerIcon,
  ChevronRightIcon,
  FileMovedIcon,
  GitBranchIcon,
  LightBulbIcon,
  ShieldIcon,
} from "@primer/octicons-react";
import { Link } from "@/components/Link";
import { lineTarget } from "@/lib/core";
import { sameTarget, useDrafts } from "@/lib/drafts";
import type { ReactNode } from "react";
import type { DiffHunk, DiffLine } from "@/lib/types";
import type { AnnotationKind, FileFocus, LineAnnotation } from "@/lib/types";
import { fileAnchor, tabForRef, tabHref } from "@/lib/router";
import type { PullRequestInfo } from "@/lib/types";
import { InlineMarkdown } from "./Markdown";
import { AddLineComment, CommentComposer, CommentOnAnnotation, DraftCard } from "./ReviewComments";
import { cx, Label, SeverityLabel, TierLabel } from "./ui";

const KIND_META: Record<AnnotationKind, { label: string; icon: ReactNode; bar: string; card: string }> = {
  security: { label: "Security", icon: <ShieldIcon size={14} />, bar: "bg-danger", card: "border-l-danger" },
  deviation: { label: "Deviation", icon: <GitBranchIcon size={14} />, bar: "bg-severe", card: "border-l-severe" },
  decision: { label: "Agent decision", icon: <LightBulbIcon size={14} />, bar: "bg-done", card: "border-l-done" },
  gap: { label: "Not validated", icon: <AlertIcon size={14} />, bar: "bg-attention", card: "border-l-attention" },
  uncovered: { label: "No test coverage", icon: <BeakerIcon size={14} />, bar: "bg-neutral-emphasis", card: "border-l-neutral-emphasis" },
};

const KIND_PRIORITY: AnnotationKind[] = ["security", "deviation", "decision", "gap", "uncovered"];

function strongestKind(annotations: LineAnnotation[]): AnnotationKind | null {
  for (const kind of KIND_PRIORITY) if (annotations.some((a) => a.kind === kind)) return kind;
  return null;
}

function AnnotationCard({ annotation, pr, path }: { annotation: LineAnnotation; pr: PullRequestInfo; path?: string }) {
  const meta = KIND_META[annotation.kind];
  const range = annotation.start === annotation.end ? `line ${annotation.start}` : `lines ${annotation.start}–${annotation.end}`;
  return (
    <div className={cx("my-2 ml-2 mr-4 rounded-md border border-border border-l-4 bg-canvas px-3 py-2 font-sans text-sm", meta.card)}>
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-fg-muted">{meta.icon}</span>
        <span className="text-xs font-semibold uppercase tracking-wide text-fg-muted">{meta.label}</span>
        {annotation.severity && <SeverityLabel severity={annotation.severity} />}
        <span className="text-xs text-fg-muted">{range}</span>
        <span className="ml-auto flex items-center gap-3">
          {path && <CommentOnAnnotation path={path} annotation={annotation} />}
          {annotation.ref && (
            <Link href={tabHref(pr, tabForRef(annotation.ref), annotation.ref)} className="text-xs hover:underline">
              Details
            </Link>
          )}
        </span>
      </div>
      <p className="mt-1 font-semibold">{annotation.title}</p>
      {annotation.detail && <p className="mt-0.5 text-fg-muted">{annotation.detail}</p>}
    </div>
  );
}

function DiffRow({ line, kinds, path }: { line: DiffLine; kinds: AnnotationKind | null; path?: string }) {
  const bg = line.kind === "add" ? "bg-diff-add" : line.kind === "del" ? "bg-diff-del" : "";
  const numBg = line.kind === "add" ? "bg-diff-add-num" : line.kind === "del" ? "bg-diff-del-num" : "";
  const marker = line.kind === "add" ? "+" : line.kind === "del" ? "-" : " ";
  return (
    <tr className={cx("group/row", bg)}>
      <td className={cx("relative w-[1%] select-none px-2 text-right align-top text-fg-muted", numBg)}>
        {path && <AddLineComment path={path} target={lineTarget(line)} />}
        {line.oldLine ?? ""}
      </td>
      <td className={cx("relative w-[1%] select-none px-2 text-right align-top text-fg-muted", numBg)}>
        {kinds && <span className={cx("absolute inset-y-0 right-0 w-1", KIND_META[kinds].bar)} aria-label={KIND_META[kinds].label} />}
        {line.newLine ?? ""}
      </td>
      <td className="whitespace-pre-wrap break-all py-px pl-2 pr-4 align-top">
        <span className="select-none text-fg-muted">{marker}</span>
        {line.text}
      </td>
    </tr>
  );
}

function HunkRows({ hunk, file, pr, placed }: { hunk: DiffHunk; file: FileFocus; pr: PullRequestInfo; placed: Map<number, LineAnnotation[]> }) {
  const drafts = useDrafts();
  const path = file.file.path;
  const rows: ReactNode[] = [
    <tr key="h" className="bg-diff-hunk text-diff-hunk-fg">
      <td colSpan={2} className="select-none" />
      <td className="py-1 pl-2">{hunk.header}</td>
    </tr>,
  ];
  hunk.lines.forEach((line, i) => {
    const covering = line.newLine === null ? [] : file.annotations.filter((a) => line.newLine! >= a.start && line.newLine! <= a.end);
    rows.push(<DiffRow key={i} line={line} kinds={strongestKind(covering)} path={drafts ? path : undefined} />);
    const here = line.newLine !== null ? placed.get(line.newLine) : undefined;
    if (here && (line.kind !== "del")) {
      rows.push(
        <tr key={`a${i}`} className="bg-canvas-subtle">
          <td colSpan={3}>
            {here.map((a, j) => (
              <AnnotationCard key={j} annotation={a} pr={pr} path={path} />
            ))}
          </td>
        </tr>,
      );
    }
    if (drafts) {
      const target = lineTarget(line);
      const here = drafts.drafts.filter((d) => d.path === path && sameTarget(d.target, target));
      const composing = drafts.composing && !drafts.composing.id && drafts.composing.path === path && sameTarget(drafts.composing.target, target);
      if (here.length || composing) {
        rows.push(
          <tr key={`c${i}`} className="bg-canvas-subtle">
            <td colSpan={3}>
              {here.map((d) => (
                <DraftCard key={d.id} draft={d} />
              ))}
              {composing && <CommentComposer composing={drafts.composing!} />}
            </td>
          </tr>,
        );
      }
    }
  });
  return <>{rows}</>;
}

/**
 * Decides after which visible line each annotation is shown: its end line, or
 * the last visible line inside its range. Annotations with no visible line in
 * range are returned separately and shown above the diff.
 */
function placeAnnotations(file: FileFocus): { placed: Map<number, LineAnnotation[]>; outside: LineAnnotation[] } {
  const visible = new Set<number>();
  for (const hunk of file.hunks) for (const l of hunk.lines) if (l.newLine !== null) visible.add(l.newLine);
  const placed = new Map<number, LineAnnotation[]>();
  const outside: LineAnnotation[] = [];
  for (const a of file.annotations) {
    let at: number | null = null;
    for (let n = a.end; n >= a.start; n--) {
      if (visible.has(n)) {
        at = n;
        break;
      }
    }
    if (at === null) outside.push(a);
    else placed.set(at, [...(placed.get(at) ?? []), a]);
  }
  return { placed, outside };
}

const STATUS_LABEL: Record<string, string> = { added: "Added", removed: "Deleted", renamed: "Renamed", copied: "Copied" };

export function FileDiff({ file, pr, defaultOpen }: { file: FileFocus; pr: PullRequestInfo; defaultOpen: boolean }) {
  const { placed, outside } = placeAnnotations(file);
  const { path, previousPath, status, additions, deletions } = file.file;
  const reasons = file.signals.filter((s) => s.weight !== 0 || s.kind === "agent-hint" || s.kind === "coverage");

  return (
    <details id={fileAnchor(path)} open={defaultOpen} className="group rounded-md border border-border bg-canvas">
      <summary className="sticky top-0 z-10 flex flex-wrap items-center gap-2 rounded-t-md border-b border-border bg-canvas-subtle px-3 py-2 group-[:not([open])]:rounded-b-md group-[:not([open])]:border-b-0">
        <ChevronRightIcon size={16} className="chevron text-fg-muted transition-transform" />
        <TierLabel tier={file.tier} />
        <span className="font-mono text-xs font-semibold">
          {previousPath && previousPath !== path && (
            <span className="font-normal text-fg-muted">
              {previousPath} <FileMovedIcon size={12} />{" "}
            </span>
          )}
          {path}
        </span>
        {STATUS_LABEL[status] && <Label>{STATUS_LABEL[status]}</Label>}
        <span className="ml-auto flex items-center gap-3 text-xs">
          <span className="text-fg-muted" title={file.roleFromAgent ? "Role reported by the agent" : "Role detected from the path"}>
            {file.role}
          </span>
          <span className="font-mono">
            <span className="text-success">+{additions}</span> <span className="text-danger">−{deletions}</span>
          </span>
          <span className="rounded-md bg-neutral-subtle px-1.5 font-mono font-semibold" title="Focus score (0–100)">
            {file.score}
          </span>
        </span>
      </summary>

      {(reasons.length > 0 || file.note) && (
        <div className="border-b border-border px-4 py-2 text-xs text-fg-muted">
          {file.note && <p className="mb-1 text-fg">{file.note}</p>}
          <ul className="flex flex-wrap gap-x-4 gap-y-1">
            {reasons.map((s, i) => (
              <li key={i}>
                {s.ref ? (
                  <Link href={tabHref(pr, tabForRef(s.ref), s.ref)} className="hover:underline">
                    <InlineMarkdown>{s.label}</InlineMarkdown>
                  </Link>
                ) : (
                  <InlineMarkdown>{s.label}</InlineMarkdown>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}

      {outside.length > 0 && (
        <div className="border-b border-border bg-canvas-subtle py-1">
          {outside.map((a, i) => (
            <AnnotationCard key={i} annotation={a} pr={pr} path={path} />
          ))}
        </div>
      )}

      {file.hunks.length === 0 ? (
        <p className="px-4 py-6 text-center text-fg-muted">
          {status === "renamed" ? "File renamed without changes." : "No diff available (binary or too large)."}
        </p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full border-collapse font-mono text-xs leading-5">
            <tbody>
              {file.hunks.map((hunk, i) => (
                <HunkRows key={i} hunk={hunk} file={file} pr={pr} placed={placed} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </details>
  );
}

/** A read-only diff, used for suggested fixes. */
export function PatchView({ hunks }: { hunks: DiffHunk[] }) {
  return (
    <div className="overflow-x-auto rounded-md border border-border">
      <table className="w-full border-collapse font-mono text-xs leading-5">
        <tbody>
          {hunks.map((hunk, i) => (
            <HunkRowsPlain key={i} hunk={hunk} />
          ))}
        </tbody>
      </table>
    </div>
  );
}

function HunkRowsPlain({ hunk }: { hunk: DiffHunk }) {
  return (
    <>
      <tr className="bg-diff-hunk text-diff-hunk-fg">
        <td colSpan={2} />
        <td className="py-1 pl-2">{hunk.header}</td>
      </tr>
      {hunk.lines.map((line, i) => (
        <DiffRow key={i} line={line} kinds={null} />
      ))}
    </>
  );
}
