// Writing a GitHub review from ReviewHub (extension only): line comments on the
// diff, comments drafted from findings, and the review itself.
import { CheckIcon, CommentIcon, PencilIcon, PlusIcon, TrashIcon, TriangleDownIcon } from "@primer/octicons-react";
import { useEffect, useRef, useState } from "react";
import { annotationComment, findingComment } from "@/lib/core";
import { type Composing, type Draft, useDrafts } from "@/lib/drafts";
import { createClient, GitHubError, type ReviewEvent } from "@/lib/github";
import { loadSettings, openSettings } from "@/lib/settings";
import type { CommentTarget, LineAnnotation, SecurityFinding } from "@/lib/types";
import { Markdown } from "./Markdown";
import { Box, Counter, Label } from "./ui";

const BUTTON = "inline-flex items-center gap-1.5 rounded-md border border-border bg-canvas-subtle px-2.5 py-1 text-xs font-medium text-fg hover:bg-neutral-subtle disabled:opacity-50";
const PRIMARY = "inline-flex items-center gap-1.5 rounded-md bg-success-emphasis px-3 py-1 text-xs font-medium text-white hover:opacity-90 disabled:opacity-50";

function describe(target: CommentTarget): string {
  const side = target.side === "LEFT" ? " (deleted)" : "";
  return target.startLine !== undefined ? `lines ${target.startLine}–${target.line}${side}` : `line ${target.line}${side}`;
}

export function CommentComposer({ composing }: { composing: Composing }) {
  const drafts = useDrafts();
  const [body, setBody] = useState(composing.body);
  const ref = useRef<HTMLTextAreaElement>(null);
  useEffect(() => ref.current?.focus(), []);
  if (!drafts) return null;
  return (
    <div className="my-2 ml-2 mr-4 rounded-md border border-border bg-canvas p-2 font-sans text-sm">
      <p className="mb-1 text-xs text-fg-muted">
        Comment on {describe(composing.target)} of <span className="font-mono">{composing.path}</span>
      </p>
      <textarea
        ref={ref}
        value={body}
        onChange={(e) => setBody(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && body.trim()) drafts.save({ ...composing, body });
          if (e.key === "Escape") drafts.compose(null);
        }}
        rows={Math.min(14, Math.max(4, body.split("\n").length + 1))}
        placeholder="Leave a comment (Markdown; a ```suggestion block proposes a change)"
        className="w-full rounded-md border border-border bg-canvas px-2 py-1.5 font-mono text-xs"
      />
      <div className="mt-2 flex justify-end gap-2">
        <button type="button" className={BUTTON} onClick={() => drafts.compose(null)}>
          Cancel
        </button>
        <button type="button" className={PRIMARY} disabled={!body.trim()} onClick={() => drafts.save({ ...composing, body })}>
          {composing.id ? "Update comment" : "Add review comment"}
        </button>
      </div>
    </div>
  );
}

export function DraftCard({ draft }: { draft: Draft }) {
  const drafts = useDrafts();
  if (!drafts) return null;
  const outdated = draft.commitId !== drafts.pr.headSha;
  if (drafts.composing?.id === draft.id) return <CommentComposer composing={drafts.composing} />;
  return (
    <div className="my-2 ml-2 mr-4 rounded-md border border-border bg-canvas px-3 py-2 font-sans text-sm">
      <div className="mb-1 flex items-center gap-2">
        <Label tone={outdated ? "attention" : "accent"}>{outdated ? "Outdated" : "Pending"}</Label>
        <span className="text-xs text-fg-muted">{outdated ? "written on an earlier commit; it won't be submitted" : describe(draft.target)}</span>
        <span className="ml-auto flex gap-1">
          {!outdated && (
            <button type="button" title="Edit" className="rounded p-1 text-fg-muted hover:bg-neutral-subtle" onClick={() => drafts.compose({ ...draft })}>
              <PencilIcon size={14} />
            </button>
          )}
          <button type="button" title="Delete" className="rounded p-1 text-fg-muted hover:bg-neutral-subtle" onClick={() => drafts.remove(draft.id)}>
            <TrashIcon size={14} />
          </button>
        </span>
      </div>
      <Markdown>{draft.body}</Markdown>
    </div>
  );
}

/** The "+" in a diff line's gutter. */
export function AddLineComment({ path, target }: { path: string; target: CommentTarget }) {
  const drafts = useDrafts();
  if (!drafts) return null;
  return (
    <button
      type="button"
      title="Comment on this line"
      aria-label="Comment on this line"
      onClick={() => drafts.compose({ path, target, body: "" })}
      className="absolute left-0 top-0 hidden size-5 items-center justify-center rounded bg-accent-emphasis text-white group-hover/row:flex focus:flex"
    >
      <PlusIcon size={12} />
    </button>
  );
}

/** "Comment" on an annotation card: a draft from the finding or annotation, to edit before adding. */
export function CommentOnAnnotation({ path, annotation }: { path: string; annotation: LineAnnotation }) {
  const drafts = useDrafts();
  if (!drafts) return null;
  const file = drafts.focus.files.find((f) => f.file.path === path);
  if (!file) return null;
  const finding = annotation.kind === "security" ? drafts.findings.find((f) => `security-${f.id}` === annotation.ref) : undefined;
  const draft = finding ? findingComment(finding, file.hunks) : annotationComment(path, annotation, file.hunks);
  if (!draft) return null;
  return (
    <button type="button" className="text-xs text-accent hover:underline" onClick={() => drafts.compose({ ...draft })}>
      Comment
    </button>
  );
}

/** On the Security tab: adds a finding to the review as a line comment, with its fix as a suggestion where possible. */
export function AddFindingToReview({ finding }: { finding: SecurityFinding }) {
  const drafts = useDrafts();
  if (!drafts) return null;
  const file = drafts.focus.files.find((f) => f.file.path === finding.file);
  const draft = file ? findingComment(finding, file.hunks) : null;
  const added = draft && drafts.current.some((d) => d.path === draft.path && d.body === draft.body);
  return (
    <button
      type="button"
      className={BUTTON}
      disabled={!draft || Boolean(added)}
      title={draft ? "Add a line comment with this finding to your review" : "The finding's lines aren't in this pull request's diff"}
      onClick={() => draft && drafts.add(draft)}
    >
      {added ? <CheckIcon size={14} className="text-success" /> : <CommentIcon size={14} className="text-fg-muted" />}
      {added ? "Added to your review" : "Add to review"}
    </button>
  );
}

const EVENTS: { event: ReviewEvent; label: string; help: string }[] = [
  { event: "COMMENT", label: "Comment", help: "Submit general feedback without explicit approval." },
  { event: "APPROVE", label: "Approve", help: "Give your approval to merge these changes." },
  { event: "REQUEST_CHANGES", label: "Request changes", help: "Submit feedback that must be addressed before merging." },
];

/** The "Review changes" button and its panel, in the pull request header. */
export function ReviewPanel() {
  const drafts = useDrafts();
  const [open, setOpen] = useState(false);
  const [event, setEvent] = useState<ReviewEvent>("COMMENT");
  const [state, setState] = useState<{ status: "idle" | "sending" } | { status: "error"; message: string; settings?: boolean } | { status: "done"; url: string }>({
    status: "idle",
  });
  if (!drafts) return null;
  const { pr, current, drafts: all, body } = drafts;
  const outdated = all.length - current.length;
  const empty = !body.trim() && current.length === 0;

  async function submit() {
    if (!drafts) return;
    setState({ status: "sending" });
    try {
      const { token } = await loadSettings();
      if (!token) {
        setState({ status: "error", message: "Reviewing needs a GitHub token with Pull requests: Read and write access. Add one in the settings.", settings: true });
        return;
      }
      const url = await createClient(token).submitReview(
        { owner: pr.owner, repo: pr.repo, number: pr.number },
        { commitId: pr.headSha, event, body, comments: current },
      );
      drafts.clear();
      setState({ status: "done", url });
    } catch (e) {
      const status = e instanceof GitHubError ? e.status : 0;
      setState({ status: "error", message: e instanceof Error ? e.message : String(e), settings: [401, 403, 404].includes(status) });
    }
  }

  return (
    <div className="relative">
      <button type="button" className={PRIMARY} onClick={() => setOpen(!open)} aria-expanded={open}>
        Review changes
        {current.length > 0 && <Counter>{current.length}</Counter>}
        <TriangleDownIcon size={14} />
      </button>
      {open && (
        <Box className="absolute right-0 z-20 mt-2 w-[min(28rem,calc(100vw-2rem))] p-3 shadow-lg">
          {state.status === "done" ? (
            <div className="text-sm">
              <p className="flex items-center gap-1.5 font-semibold">
                <CheckIcon size={16} className="text-success" /> Review submitted
              </p>
              {state.url && (
                <a href={state.url} target="_blank" rel="noreferrer" className="mt-1 block text-xs hover:underline">
                  View it on GitHub
                </a>
              )}
            </div>
          ) : (
            <>
              <p className="mb-2 text-sm font-semibold">Finish your review</p>
              <textarea
                value={body}
                onChange={(e) => drafts.setBody(e.target.value)}
                rows={5}
                placeholder="Leave a comment"
                className="w-full rounded-md border border-border bg-canvas px-2 py-1.5 text-sm"
              />
              <fieldset className="mt-2 space-y-1.5">
                {EVENTS.map((e) => (
                  <label key={e.event} className="flex cursor-pointer gap-2 text-sm">
                    <input type="radio" name="review-event" checked={event === e.event} onChange={() => setEvent(e.event)} className="mt-1" />
                    <span>
                      <span className="font-medium">{e.label}</span>
                      <span className="block text-xs text-fg-muted">{e.help}</span>
                    </span>
                  </label>
                ))}
              </fieldset>
              <p className="mt-2 text-xs text-fg-muted">
                {current.length === 0 ? "No line comments." : `${current.length} line ${current.length === 1 ? "comment" : "comments"} will be submitted.`}
                {outdated > 0 && ` ${outdated} written on an earlier commit won't be.`}
              </p>
              {state.status === "error" && (
                <p className="mt-2 rounded-md border border-danger/40 bg-danger-subtle px-2 py-1.5 text-xs text-danger">
                  {state.message}{" "}
                  {state.settings && (
                    <a href="options.html" onClick={openSettings} className="underline">
                      Settings
                    </a>
                  )}
                </p>
              )}
              <div className="mt-3 flex justify-end gap-2">
                <button type="button" className={BUTTON} onClick={() => setOpen(false)}>
                  Close
                </button>
                <button
                  type="button"
                  className={PRIMARY}
                  disabled={state.status === "sending" || (event !== "APPROVE" && empty)}
                  title={event !== "APPROVE" && empty ? "Add a comment or line comments first" : undefined}
                  onClick={submit}
                >
                  {state.status === "sending" ? "Submitting…" : "Submit review"}
                </button>
              </div>
            </>
          )}
        </Box>
      )}
    </div>
  );
}

