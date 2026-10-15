// The reviewer's pending GitHub review: line comments and the review itself,
// kept in this browser per pull request until submitted. Extension only: the
// web app has no write access to GitHub.
import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import type { CommentTarget, DraftComment, FocusAnalysis, PullRequestInfo, SecurityFinding } from "./types";

export interface Draft extends DraftComment {
  id: string;
  /** Head commit the comment was written against; GitHub places it on that commit's diff. */
  commitId: string;
}

/** A comment being written, before it is added to the review. */
export interface Composing {
  path: string;
  target: CommentTarget;
  body: string;
  /** Set when editing an existing draft. */
  id?: string;
}

interface Stored {
  drafts: Draft[];
  body: string;
}

interface Drafts {
  pr: PullRequestInfo;
  focus: FocusAnalysis;
  findings: SecurityFinding[];
  drafts: Draft[];
  /** Drafts written against the current head commit; older ones can't be submitted. */
  current: Draft[];
  body: string;
  setBody(body: string): void;
  composing: Composing | null;
  compose(c: Composing | null): void;
  save(c: Composing): void;
  add(d: DraftComment): void;
  remove(id: string): void;
  clear(): void;
}

const Context = createContext<Drafts | null>(null);

/** The pending review, or null where reviews can't be written (the web app). */
export function useDrafts(): Drafts | null {
  return useContext(Context);
}

function storageKey(pr: PullRequestInfo) {
  return `reviewhub:review:${pr.owner}/${pr.repo}#${pr.number}`;
}

function load(pr: PullRequestInfo): Stored {
  try {
    const stored = JSON.parse(localStorage.getItem(storageKey(pr)) ?? "null") as Stored | null;
    if (stored && Array.isArray(stored.drafts)) return { drafts: stored.drafts, body: stored.body ?? "" };
  } catch {
    // Unreadable or blocked storage: start empty.
  }
  return { drafts: [], body: "" };
}

export function DraftsProvider({
  pr,
  focus,
  findings,
  children,
}: {
  pr: PullRequestInfo;
  focus: FocusAnalysis;
  findings: SecurityFinding[];
  children: ReactNode;
}) {
  const [stored, setStored] = useState<Stored>(() => load(pr));
  const [composing, compose] = useState<Composing | null>(null);

  useEffect(() => {
    try {
      if (stored.drafts.length || stored.body) localStorage.setItem(storageKey(pr), JSON.stringify(stored));
      else localStorage.removeItem(storageKey(pr));
    } catch {
      // Storage blocked: the review lasts for this page only.
    }
  }, [pr, stored]);

  const add = useCallback(
    (d: DraftComment) =>
      setStored((s) => ({ ...s, drafts: [...s.drafts, { ...d, id: crypto.randomUUID(), commitId: pr.headSha }] })),
    [pr.headSha],
  );
  const save = useCallback(
    (c: Composing) => {
      setStored((s) =>
        c.id
          ? { ...s, drafts: s.drafts.map((d) => (d.id === c.id ? { ...d, body: c.body } : d)) }
          : { ...s, drafts: [...s.drafts, { path: c.path, target: c.target, body: c.body, id: crypto.randomUUID(), commitId: pr.headSha }] },
      );
      compose(null);
    },
    [pr.headSha],
  );

  const value = useMemo<Drafts>(
    () => ({
      pr,
      focus,
      findings,
      drafts: stored.drafts,
      current: stored.drafts.filter((d) => d.commitId === pr.headSha),
      body: stored.body,
      setBody: (body) => setStored((s) => ({ ...s, body })),
      composing,
      compose,
      save,
      add,
      remove: (id) => setStored((s) => ({ ...s, drafts: s.drafts.filter((d) => d.id !== id) })),
      clear: () => setStored({ drafts: [], body: "" }),
    }),
    [pr, focus, findings, stored, composing, save, add],
  );
  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function sameTarget(a: CommentTarget, b: CommentTarget): boolean {
  return a.line === b.line && a.side === b.side;
}
