// Writing a GitHub review from ReviewHub (extension only): line comments on the
// diff, comments drafted from findings, and the review itself.
import { CheckIcon, CommentIcon, PencilIcon, PlusIcon, TrashIcon, TriangleDownIcon } from "@primer/octicons-react";
import { useEffect, useRef, useState } from "react";
import { annotationComment, authorReviewBody, findingComment } from "@/lib/core";
import { type Composing, type Draft, useDrafts } from "@/lib/drafts";
import { createClient, GitHubError, type MergeMethod, type ReviewEvent } from "@/lib/github";
import { useRoute } from "@/lib/router";
import { EXTENSION_URL, loadSettings, openSettings } from "@/lib/settings";
import type { AuthorVerdict, CommentTarget, LineAnnotation, PullRequestInfo, SecurityFinding } from "@/lib/types";
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

/** The "+" in a diff line's gutter: click to comment on the line, drag or shift-click to select several. */
export function AddLineComment({ onStart }: { onStart: (event: { shiftKey: boolean }) => void }) {
  return (
    <button
      type="button"
      title="Comment on this line (drag, or shift-click another line, to select several)"
      aria-label="Comment on this line"
      onMouseDown={(e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        onStart(e);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onStart(e);
        }
      }}
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

const EVENTS: { event: ReviewEvent; label: string; help: string; authorHelp?: string }[] = [
  { event: "COMMENT", label: "Comment", help: "Submit general feedback without explicit approval." },
  {
    event: "APPROVE",
    label: "Approve",
    help: "Give your approval to merge these changes.",
    authorHelp: "GitHub doesn't let you approve your own pull request, so your approval is posted as a comment. Merge it now, or say why it should wait.",
  },
  {
    event: "REQUEST_CHANGES",
    label: "Request changes",
    help: "Submit feedback that must be addressed before merging.",
    authorHelp: "GitHub doesn't let you request changes on your own pull request, so this is posted as a comment asking the agent to make them.",
  },
];

const METHODS: Record<MergeMethod, string> = { merge: "Create a merge commit", squash: "Squash and merge", rebase: "Rebase and merge" };

type Outcome = { status: "done"; url: string; merged: boolean };

/** Whether the token's user opened this pull request, and how its repository lets it be merged; null until known. */
function useAuthorship(open: boolean, pr: { owner: string; repo: string; number: number; author: { login: string } }) {
  const [authorship, setAuthorship] = useState<{ isAuthor: boolean; methods: MergeMethod[] } | null>(null);
  const { owner, repo, number } = pr;
  const author = pr.author.login.toLowerCase();
  useEffect(() => {
    if (!open || authorship) return;
    let cancelled = false;
    (async () => {
      const { token } = await loadSettings();
      if (!token) return;
      const client = createClient(token);
      const isAuthor = (await client.viewer()).toLowerCase() === author;
      const methods = isAuthor ? await client.mergeMethods({ owner, repo, number }) : [];
      if (!cancelled) setAuthorship({ isAuthor, methods });
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [open, authorship, owner, repo, number, author]);
  return authorship;
}

/** In the web app and on the demo, where reviews can't be submitted: the same button, saying where you can review. */
function ReviewElsewhere({ pr, demo }: { pr: PullRequestInfo; demo: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="relative">
      <button type="button" className={PRIMARY} onClick={() => setOpen(!open)} aria-expanded={open}>
        Review changes
        <TriangleDownIcon size={14} />
      </button>
      {open && (
        <Box className="absolute right-0 z-20 mt-2 w-[min(24rem,calc(100vw-2rem))] p-3 text-sm shadow-lg">
          {demo ? (
            <p>This is a demo pull request, so there&apos;s nothing to review on GitHub. Open one of your own pull requests to comment, approve or request changes.</p>
          ) : (
            <>
              <p>
                Reviewing from ReviewHub (line comments, Comment, Approve and Request changes, and merging your own pull requests) works in the Chrome extension,
                which keeps your GitHub token out of reach of other websites.
              </p>
              <div className="mt-3 flex flex-wrap justify-end gap-2">
                <a href={`${pr.htmlUrl}/files`} target="_blank" rel="noreferrer" className={BUTTON}>
                  Review on GitHub
                </a>
                <a href={EXTENSION_URL} target="_blank" rel="noreferrer" className={PRIMARY}>
                  Get the extension
                </a>
              </div>
            </>
          )}
        </Box>
      )}
    </div>
  );
}

/** The "Review changes" button and its panel, in the pull request header. */
export function ReviewPanel({ pr: info }: { pr: PullRequestInfo }) {
  const drafts = useDrafts();
  const route = useRoute();
  const [open, setOpen] = useState(false);
  const [event, setEvent] = useState<ReviewEvent>("COMMENT");
  const [mergeNow, setMergeNow] = useState(true);
  const [method, setMethod] = useState<MergeMethod | null>(null);
  const [reason, setReason] = useState("");
  const [state, setState] = useState<{ status: "idle" | "sending" } | { status: "error"; message: string; settings?: boolean } | Outcome>({
    status: "idle",
  });
  const authorship = useAuthorship(open, drafts?.pr ?? { owner: "", repo: "", number: 0, author: { login: "" } });
  if (!drafts) return <ReviewElsewhere pr={info} demo={route.demo} />;
  const { pr, current, drafts: all, body } = drafts;
  const outdated = all.length - current.length;
  const empty = !body.trim() && current.length === 0;
  const isAuthor = authorship?.isAuthor ?? false;
  // Only an open pull request that isn't a draft can be merged.
  const mergeable = pr.state === "open" && !pr.draft && (authorship?.methods.length ?? 0) > 0;
  const merging = isAuthor && event === "APPROVE" && mergeable && mergeNow;
  const chosenMethod = method ?? (authorship?.methods.includes("squash") ? "squash" : authorship?.methods[0]) ?? "merge";
  const missing =
    event !== "APPROVE" && empty
      ? "Add a comment or line comments first"
      : isAuthor && event === "APPROVE" && !merging && !reason.trim()
        ? "Say why it shouldn't be merged yet"
        : undefined;

  /** What GitHub gets: the reviewer's own verdict, or for the author (who can only comment) a comment saying theirs. */
  function submission(): { event: ReviewEvent; body: string } {
    if (!isAuthor || event === "COMMENT") return { event, body };
    const verdict: AuthorVerdict =
      event === "REQUEST_CHANGES" ? { kind: "request-changes" } : merging ? { kind: "approve-and-merge" } : { kind: "approve-not-yet", reason };
    return { event: "COMMENT", body: authorReviewBody(verdict, body, current.length) };
  }

  async function submit() {
    if (!drafts) return;
    setState({ status: "sending" });
    let merged = false;
    try {
      const { token } = await loadSettings();
      if (!token) {
        setState({ status: "error", message: "Reviewing needs a GitHub token with Pull requests: Read and write access. Add one in the settings.", settings: true });
        return;
      }
      const client = createClient(token);
      const ref = { owner: pr.owner, repo: pr.repo, number: pr.number };
      // Merge first: an "approved and merged" comment must not go up for a pull request GitHub then refuses to merge.
      if (merging) {
        await client.merge(ref, pr.headSha, chosenMethod);
        merged = true;
      }
      const url = await client.submitReview(ref, { commitId: pr.headSha, comments: current, ...submission() });
      drafts.clear();
      setState({ status: "done", url, merged });
    } catch (e) {
      const status = e instanceof GitHubError ? e.status : 0;
      const message = e instanceof Error ? e.message : String(e);
      setState({
        status: "error",
        message: merged ? `Merged, but your review comment wasn't posted: ${message}` : message,
        settings: [401, 403, 404].includes(status),
      });
    }
  }

  // For the author, approving means choosing between merging now and saying why it waits.
  const approval = isAuthor && event === "APPROVE" && (
                <fieldset className="ml-6 mt-2 space-y-1.5 border-l-2 border-border pl-3">
                  <label className="flex cursor-pointer gap-2 text-sm">
                    <input type="radio" name="author-approval" checked={mergeable && mergeNow} disabled={!mergeable} onChange={() => setMergeNow(true)} className="mt-1" />
                    <span>
                      <span className="font-medium">Merge it now</span>
                      <span className="block text-xs text-fg-muted">
                        {mergeable ? "It's approved: merge it, then post your approval." : "Only an open pull request that isn't a draft can be merged."}
                      </span>
                    </span>
                  </label>
                  {mergeable && mergeNow && authorship && authorship.methods.length > 1 && (
                    <select
                      aria-label="Merge method"
                      value={chosenMethod}
                      onChange={(e) => setMethod(e.target.value as MergeMethod)}
                      className="ml-6 rounded-md border border-border bg-canvas px-2 py-1 text-xs"
                    >
                      {authorship.methods.map((m) => (
                        <option key={m} value={m}>
                          {METHODS[m]}
                        </option>
                      ))}
                    </select>
                  )}
                  <label className="flex cursor-pointer gap-2 text-sm">
                    <input type="radio" name="author-approval" checked={!(mergeable && mergeNow)} onChange={() => setMergeNow(false)} className="mt-1" />
                    <span>
                      <span className="font-medium">Don&apos;t merge yet</span>
                      <span className="block text-xs text-fg-muted">Approve it, and say why it can&apos;t or shouldn&apos;t be merged yet.</span>
                    </span>
                  </label>
                  {!(mergeable && mergeNow) && (
                    <textarea
                      value={reason}
                      onChange={(e) => setReason(e.target.value)}
                      rows={2}
                      aria-label="Why it shouldn't be merged yet"
                      placeholder="Why it shouldn't be merged yet, e.g. waiting for the mobile release"
                      className="w-full rounded-md border border-border bg-canvas px-2 py-1.5 text-sm"
                    />
                  )}
                </fieldset>
  );

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
                <CheckIcon size={16} className="text-success" /> {state.merged ? "Merged, and your review posted" : "Review submitted"}
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
              {isAuthor && (
                <p className="mb-2 rounded-md border border-border bg-canvas-subtle px-2 py-1.5 text-xs text-fg-muted">
                  You opened this pull request. GitHub only lets authors comment, so ReviewHub posts your verdict as a comment that says what you decided.
                </p>
              )}
              <textarea
                value={body}
                onChange={(e) => drafts.setBody(e.target.value)}
                rows={5}
                placeholder="Leave a comment"
                className="w-full rounded-md border border-border bg-canvas px-2 py-1.5 text-sm"
              />
              <fieldset className="mt-2 space-y-1.5">
                {EVENTS.map((e) => (
                  <div key={e.event}>
                    <label className="flex cursor-pointer gap-2 text-sm">
                      <input type="radio" name="review-event" checked={event === e.event} onChange={() => setEvent(e.event)} className="mt-1" />
                      <span>
                        <span className="font-medium">{e.label}</span>
                        <span className="block text-xs text-fg-muted">{(isAuthor && e.authorHelp) || e.help}</span>
                      </span>
                    </label>
                    {e.event === "APPROVE" && approval}
                  </div>
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
                  disabled={state.status === "sending" || Boolean(missing)}
                  title={missing}
                  onClick={submit}
                >
                  {state.status === "sending" ? (merging ? "Merging…" : "Submitting…") : merging ? "Merge and submit" : "Submit review"}
                </button>
              </div>
            </>
          )}
        </Box>
      )}
    </div>
  );
}
