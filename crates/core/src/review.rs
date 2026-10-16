//! Pull request review comments: where GitHub accepts a comment on a diff, and
//! draft comments made from ReviewHub's findings, for the reviewer to edit and
//! submit as part of a GitHub review.

use serde::{Deserialize, Serialize};

use crate::diff::{DiffLine, Hunk, LineType, split_unified_diff};
use crate::focus::{Annotation, AnnotationKind};
use crate::model::SecurityFinding;

/// Which version of the file a comment is on: `RIGHT` for added and unchanged lines, `LEFT` for deleted ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Left,
    Right,
}

/// Where a review comment goes, in the terms of GitHub's review API. GitHub only
/// accepts lines inside the pull request's diff, and a multi-line comment must
/// start and end in the same hunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct CommentTarget {
    pub line: u32,
    pub side: Side,
    /// First line of a multi-line comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u32>,
    /// Side of `start_line` when it differs from `side` (a range from a deleted line to an added one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_side: Option<Side>,
}

/// A review comment for the reviewer to edit before submitting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DraftComment {
    pub path: String,
    pub target: CommentTarget,
    pub body: String,
}

/// The target for a comment on one line of the diff.
pub fn line_target(line: &DiffLine) -> CommentTarget {
    match line.new_line {
        Some(n) => CommentTarget { line: n, side: Side::Right, start_line: None, start_side: None },
        None => {
            CommentTarget { line: line.old_line.unwrap_or(1), side: Side::Left, start_line: None, start_side: None }
        }
    }
}

/// The target for the rows `from..=to` of a hunk (in either order), as a reviewer selects them on the diff.
/// `None` when an index is out of range.
pub fn selection_target(hunk: &Hunk, from: usize, to: usize) -> Option<CommentTarget> {
    let (first, last) = (from.min(to), from.max(to));
    let end = line_target(hunk.lines.get(last)?);
    if first == last {
        return Some(end);
    }
    let start = line_target(hunk.lines.get(first)?);
    Some(CommentTarget {
        start_line: Some(start.line),
        start_side: (start.side != end.side).then_some(start.side),
        ..end
    })
}

/// The target covering as much of the new-file lines `start..=end` as the diff
/// shows: ending at the last visible line in range, and starting at the first
/// visible line in range within the same hunk. `None` when no line in range is
/// in the diff.
pub fn range_target(hunks: &[Hunk], start: u32, end: u32) -> Option<CommentTarget> {
    let in_range = |l: &DiffLine| l.new_line.filter(|n| (start..=end).contains(n));
    let hunk = hunks.iter().rev().find(|h| h.lines.iter().any(|l| in_range(l).is_some()))?;
    let visible: Vec<u32> = hunk.lines.iter().filter_map(in_range).collect();
    let (first, last) = (*visible.first()?, *visible.last()?);
    Some(CommentTarget {
        line: last,
        side: Side::Right,
        start_line: (first != last).then_some(first),
        start_side: None,
    })
}

/// A fix that GitHub can show as a suggested change: replace the target lines with `replacement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedChange {
    pub target: CommentTarget,
    pub replacement: String,
}

/// Turns a fix (a unified diff against the pull request's head) into a GitHub
/// suggestion on `path`, when it is one hunk whose replaced lines are all
/// visible in one hunk of the pull request's diff.
pub fn suggested_change(pr_hunks: &[Hunk], path: &str, fix_diff: &str) -> Option<SuggestedChange> {
    let files = split_unified_diff(fix_diff);
    let [(fix_path, fix_hunks)] = files.as_slice() else { return None };
    let [fix] = fix_hunks.as_slice() else { return None };
    if fix_path != path {
        return None;
    }

    // Drop surrounding context, keeping one line to anchor a pure insertion.
    let lines = &fix.lines;
    let changed = |l: &&DiffLine| l.kind != LineType::Ctx;
    let first = lines.iter().position(|l| changed(&l))?;
    let last = lines.iter().rposition(|l| changed(&l))?;
    let (mut from, mut to) = (first, last);
    if !lines[from..=to].iter().any(|l| l.kind == LineType::Del) {
        if from > 0 {
            from -= 1;
        } else if to + 1 < lines.len() {
            to += 1;
        } else {
            return None;
        }
    }
    let kept = &lines[from..=to];

    // The fix's old side is the head version of the file, which is the pull request's new side.
    let old: Vec<u32> = kept.iter().filter_map(|l| l.old_line).collect();
    let (start, end) = (*old.first()?, *old.last()?);
    let hunk = pr_hunks.iter().find(|h| h.lines.iter().any(|l| l.new_line == Some(start)))?;
    let visible: Vec<u32> = hunk.lines.iter().filter_map(|l| l.new_line).collect();
    if !(start..=end).all(|n| visible.contains(&n)) {
        return None;
    }
    let replacement =
        kept.iter().filter(|l| l.kind != LineType::Del).map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
    Some(SuggestedChange {
        target: CommentTarget {
            line: end,
            side: Side::Right,
            start_line: (start != end).then_some(start),
            start_side: None,
        },
        replacement,
    })
}

/// A draft comment for a security finding, on its lines in `pr_hunks` (the
/// finding's file). With a fix GitHub can show as a suggestion, the comment
/// carries it as a suggested change. `None` when the lines aren't in the diff.
pub fn finding_comment(finding: &SecurityFinding, pr_hunks: &[Hunk]) -> Option<DraftComment> {
    let suggestion = finding.suggestion.diff.as_deref().and_then(|d| suggested_change(pr_hunks, &finding.file, d));
    let target = match (&suggestion, finding.lines) {
        (Some(s), _) => s.target.clone(),
        (None, Some(lines)) => range_target(pr_hunks, lines.0, lines.1)?,
        (None, None) => return None,
    };

    let category = finding.category.as_deref().map(|c| format!(" ({})", quiet(c))).unwrap_or_default();
    let mut body = format!(
        "**{} security finding: {}**{category}\n\n{}\n\n**Suggested fix:** {}",
        finding.severity.as_str(),
        quiet(&finding.title),
        quiet(&finding.description),
        quiet(&finding.suggestion.explanation),
    );
    if let Some(s) = &suggestion {
        let fence = fence_for(&s.replacement);
        body.push_str(&format!("\n\n{fence}suggestion\n{}\n{fence}", s.replacement));
    } else if let Some(diff) = &finding.suggestion.diff {
        let fence = fence_for(diff);
        body.push_str(&format!("\n\n{fence}diff\n{}\n{fence}", diff.trim_end()));
    }
    Some(DraftComment { path: finding.file.clone(), target, body })
}

/// A draft comment for an annotation shown on the diff (a deviation, decision, gap or uncovered lines).
pub fn annotation_comment(path: &str, annotation: &Annotation, pr_hunks: &[Hunk]) -> Option<DraftComment> {
    let target = range_target(pr_hunks, annotation.start, annotation.end)?;
    let mut body = format!("**{}:** {}", annotation.kind.label(), quiet(&annotation.title));
    if let Some(detail) = &annotation.detail {
        body.push_str(&format!("\n\n{}", quiet(detail)));
    }
    Some(DraftComment { path: path.to_string(), target, body })
}

impl AnnotationKind {
    pub fn label(self) -> &'static str {
        match self {
            AnnotationKind::Security => "Security",
            AnnotationKind::Deviation => "Deviation",
            AnnotationKind::Decision => "Agent decision",
            AnnotationKind::Gap => "Not validated",
            AnnotationKind::Uncovered => "No test coverage",
        }
    }
}

/// Report text is written by agents; posted under the reviewer's name, it must
/// not notify people. Breaks @mentions with a zero-width space.
fn quiet(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '@' && chars.peek().is_some_and(|n| n.is_ascii_alphanumeric()) {
            out.push('\u{200B}');
        }
    }
    out
}

/// A code fence longer than any backtick run in `text`.
fn fence_for(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

/// The verdict of a reviewer who is also the pull request's author. GitHub doesn't let authors approve or request
/// changes on their own pull requests, so these are submitted as comments that say what the author decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AuthorVerdict {
    /// The author wants the agent to make changes before merging.
    RequestChanges,
    /// Approved, and merged right away.
    ApproveAndMerge,
    /// Approved, but it can't or shouldn't be merged yet, for `reason`.
    ApproveNotYet { reason: String },
}

/// The body of an author's review: what they decided, then their own comment. `line_comments` is how many comments
/// on the diff go with it. Starts with a hidden marker (`<!-- reviewhub-author-review: … -->`) for tools.
/// Anyone who can comment can write the same marker and text: tools must only act on it when the comment's or
/// review's `user.login` is the pull request's author.
pub fn author_review_body(verdict: &AuthorVerdict, body: &str, line_comments: usize) -> String {
    const NOTE: &str = "GitHub doesn't let authors approve or request changes on their own pull requests, so this is a \
                        comment.";
    let on_diff = match line_comments {
        0 => String::new(),
        1 => " and in the comment on the diff".to_string(),
        n => format!(" and in the {n} comments on the diff"),
    };
    let (marker, decision) = match verdict {
        AuthorVerdict::RequestChanges => (
            "changes-requested",
            format!(
                "**Changes requested** by the author. To the agent working on this pull request: make the changes \
                 asked for here{on_diff}, then push an update. {NOTE}"
            ),
        ),
        AuthorVerdict::ApproveAndMerge => ("approved", format!("**Approved and merged** by the author. {NOTE}")),
        AuthorVerdict::ApproveNotYet { reason } => (
            "approved-not-merged",
            format!("**Approved** by the author, but not to be merged yet: {}\n\n{NOTE}", reason.trim()),
        ),
    };
    let mut out = format!("<!-- reviewhub-author-review: {marker} -->\n{decision}");
    if !body.trim().is_empty() {
        out.push_str("\n\n");
        out.push_str(body.trim());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parse_patch;
    use crate::model::{Confidence, Severity, Suggestion};

    // Head lines 10-14 visible: 10 ctx, 11 added, 12 added, 13 ctx; head line 14 is outside the diff.
    fn pr_hunks() -> Vec<Hunk> {
        parse_patch(Some("@@ -10,2 +10,4 @@\n a\n+query(`${id}`)\n+log(id)\n b"))
    }

    fn finding(lines: Option<(u32, u32)>, diff: Option<&str>) -> SecurityFinding {
        SecurityFinding {
            id: "f".into(),
            title: "SQL injection, ask @alice".into(),
            severity: Severity::High,
            category: Some("CWE-89".into()),
            description: "Interpolated id.".into(),
            file: "src/a.ts".into(),
            lines: lines.map(|(a, b)| crate::model::LineRange(a, b)),
            confidence: Confidence::High,
            suggestion: Suggestion { explanation: "Bind it.".into(), diff: diff.map(String::from), prompt: None },
        }
    }

    #[test]
    fn targets_lines_inside_the_diff() {
        let hunks = pr_hunks();
        assert_eq!(
            range_target(&hunks, 11, 12),
            Some(CommentTarget { line: 12, side: Side::Right, start_line: Some(11), start_side: None })
        );
        assert_eq!(
            range_target(&hunks, 12, 20),
            Some(CommentTarget { line: 13, side: Side::Right, start_line: Some(12), start_side: None })
        );
        assert_eq!(range_target(&hunks, 11, 11).unwrap().start_line, None);
        assert_eq!(range_target(&hunks, 30, 40), None);
        let deleted = parse_patch(Some("@@ -5,1 +5,0 @@\n-gone"));
        assert_eq!(
            line_target(&deleted[0].lines[0]),
            CommentTarget { line: 5, side: Side::Left, start_line: None, start_side: None }
        );
    }

    #[test]
    fn targets_selected_rows() {
        // old 5 deleted, new 5-6 added, then line 6/7 unchanged.
        let hunk = &parse_patch(Some("@@ -5,2 +5,3 @@\n-old\n+new\n+more\n same"))[0];
        assert_eq!(
            selection_target(hunk, 1, 1),
            Some(CommentTarget { line: 5, side: Side::Right, start_line: None, start_side: None })
        );
        assert_eq!(
            selection_target(hunk, 3, 1),
            Some(CommentTarget { line: 7, side: Side::Right, start_line: Some(5), start_side: None })
        );
        assert_eq!(
            selection_target(hunk, 0, 2),
            Some(CommentTarget { line: 6, side: Side::Right, start_line: Some(5), start_side: Some(Side::Left) })
        );
        assert_eq!(selection_target(hunk, 0, 9), None);
    }

    #[test]
    fn makes_suggestions_from_single_hunk_fixes() {
        let fix = "--- a/src/a.ts\n+++ b/src/a.ts\n@@ -10,3 +10,3 @@\n a\n-query(`${id}`)\n+query('?', [id])\n log(id)";
        let s = suggested_change(&pr_hunks(), "src/a.ts", fix).unwrap();
        assert_eq!(s.target, CommentTarget { line: 11, side: Side::Right, start_line: None, start_side: None });
        assert_eq!(s.replacement, "query('?', [id])");

        // A pure insertion keeps the line before it as the anchor.
        let insert = "--- a/src/a.ts\n+++ b/src/a.ts\n@@ -12,1 +12,2 @@\n log(id)\n+audit(id)";
        let s = suggested_change(&pr_hunks(), "src/a.ts", insert).unwrap();
        assert_eq!((s.target.line, s.replacement.as_str()), (12, "log(id)\naudit(id)"));

        // Lines outside the diff, other files and multi-hunk fixes can't be suggestions.
        assert!(suggested_change(&pr_hunks(), "src/a.ts", "+++ b/src/a.ts\n@@ -14 +14 @@\n-x\n+y").is_none());
        assert!(suggested_change(&pr_hunks(), "src/b.ts", fix).is_none());
        assert!(suggested_change(&pr_hunks(), "src/a.ts", &format!("{fix}\n@@ -1 +1 @@\n-q\n+r")).is_none());
    }

    #[test]
    fn drafts_finding_comments() {
        let fix = "--- a/src/a.ts\n+++ b/src/a.ts\n@@ -11 +11 @@\n-query(`${id}`)\n+query('?', [id])";
        let c = finding_comment(&finding(Some((11, 11)), Some(fix)), &pr_hunks()).unwrap();
        assert_eq!(c.path, "src/a.ts");
        assert!(
            c.body.starts_with("**high security finding: SQL injection, ask @\u{200B}alice** (CWE-89)"),
            "{}",
            c.body
        );
        assert!(c.body.ends_with("```suggestion\nquery('?', [id])\n```"), "{}", c.body);

        // A fix that can't be a suggestion is shown as a diff, on the finding's lines.
        let elsewhere = "--- a/src/a.ts\n+++ b/src/a.ts\n@@ -40 +40 @@\n-x\n+y";
        let c = finding_comment(&finding(Some((12, 12)), Some(elsewhere)), &pr_hunks()).unwrap();
        assert_eq!(c.target.line, 12);
        assert!(c.body.contains("```diff\n"));
        assert!(finding_comment(&finding(Some((40, 41)), None), &pr_hunks()).is_none());
        assert!(finding_comment(&finding(None, None), &pr_hunks()).is_none());
    }

    #[test]
    fn fences_outgrow_backticks_in_the_code() {
        assert_eq!(fence_for("plain"), "```");
        assert_eq!(fence_for("a ``` b"), "````");
    }

    #[test]
    fn says_what_an_author_decided() {
        let changes = author_review_body(&AuthorVerdict::RequestChanges, " Rename the flag. ", 2);
        assert!(
            changes.starts_with(
                "<!-- reviewhub-author-review: changes-requested -->\n**Changes requested** by the author."
            )
        );
        assert!(
            changes.contains("make the changes asked for here and in the 2 comments on the diff, then push an update")
        );
        assert!(changes.ends_with("comment.\n\nRename the flag."), "{changes}");
        assert!(author_review_body(&AuthorVerdict::RequestChanges, "", 1).contains("and in the comment on the diff,"));
        assert!(author_review_body(&AuthorVerdict::RequestChanges, "x", 0).contains("asked for here, then"));

        let merged = author_review_body(&AuthorVerdict::ApproveAndMerge, "", 0);
        assert!(
            merged.starts_with("<!-- reviewhub-author-review: approved -->\n**Approved and merged** by the author.")
        );
        assert!(merged.ends_with("so this is a comment."), "{merged}");

        let held = AuthorVerdict::ApproveNotYet { reason: " Wait for the mobile release. ".into() };
        let held = author_review_body(&held, "Nice work.", 0);
        assert!(
            held.contains("**Approved** by the author, but not to be merged yet: Wait for the mobile release.\n\n")
        );
        assert!(held.ends_with("comment.\n\nNice work."), "{held}");
    }
}
