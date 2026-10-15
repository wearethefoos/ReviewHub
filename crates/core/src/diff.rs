//! Unified diff parsing.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum LineType {
    Add,
    Del,
    Ctx,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: LineType,
    pub text: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

static HUNK_HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@").unwrap());

/// Parses the hunks of a single-file patch, as in the `patch` field of GitHub's files API.
pub fn parse_patch(patch: Option<&str>) -> Vec<Hunk> {
    let Some(patch) = patch.filter(|p| !p.is_empty()) else { return Vec::new() };
    let mut rows: Vec<&str> = patch.split('\n').collect();
    if rows.last() == Some(&"") {
        rows.pop();
    }

    let mut hunks: Vec<Hunk> = Vec::new();
    let (mut old_no, mut new_no) = (0u32, 0u32);
    for raw in rows {
        if let Some(caps) = HUNK_HEADER.captures(raw) {
            old_no = caps[1].parse().unwrap_or(0);
            new_no = caps[2].parse().unwrap_or(0);
            hunks.push(Hunk { header: raw.to_string(), lines: Vec::new() });
            continue;
        }
        let Some(hunk) = hunks.last_mut() else { continue };
        if raw.starts_with('\\') {
            continue; // "\ No newline at end of file"
        }
        let text = raw.get(1..).unwrap_or("").to_string();
        let line = match raw.chars().next() {
            Some('+') => {
                new_no += 1;
                DiffLine { kind: LineType::Add, text, old_line: None, new_line: Some(new_no - 1) }
            }
            Some('-') => {
                old_no += 1;
                DiffLine { kind: LineType::Del, text, old_line: Some(old_no - 1), new_line: None }
            }
            Some(' ') | None => {
                old_no += 1;
                new_no += 1;
                DiffLine { kind: LineType::Ctx, text, old_line: Some(old_no - 1), new_line: Some(new_no - 1) }
            }
            _ => continue,
        };
        hunk.lines.push(line);
    }
    hunks
}

/// Line numbers (new side) that the patch adds.
pub fn added_lines(hunks: &[Hunk]) -> Vec<u32> {
    hunks.iter().flat_map(|h| &h.lines).filter(|l| l.kind == LineType::Add).filter_map(|l| l.new_line).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_line_numbers() {
        let hunks = parse_patch(Some("@@ -10,3 +10,4 @@ fn()\n a\n-b\n+B\n+C\n d"));
        let lines = &hunks[0].lines;
        assert_eq!(lines.len(), 5);
        assert_eq!((lines[1].kind, lines[1].old_line, lines[1].new_line), (LineType::Del, Some(11), None));
        assert_eq!((lines[3].kind, lines[3].new_line), (LineType::Add, Some(12)));
        assert_eq!((lines[4].old_line, lines[4].new_line), (Some(12), Some(13)));
    }

    #[test]
    fn handles_multiple_hunks_and_markers() {
        let hunks = parse_patch(Some("@@ -1 +1 @@\n-x\n+y\n\\ No newline at end of file\n@@ -20,1 +20,1 @@\n z\n"));
        assert_eq!(hunks.iter().map(|h| h.lines.len()).collect::<Vec<_>>(), vec![2, 1]);
        assert_eq!(added_lines(&hunks), vec![1]);
        assert!(parse_patch(None).is_empty());
    }
}

/// Splits a multi-file unified diff (such as a suggested fix) into per-file hunks.
pub fn split_unified_diff(diff: &str) -> Vec<(String, Vec<Hunk>)> {
    let mut files: Vec<(String, Vec<&str>)> = Vec::new();
    for line in diff.split('\n') {
        if let Some(path) = line.strip_prefix("+++ ") {
            files.push((path.trim().trim_start_matches("b/").to_string(), Vec::new()));
        } else if line.starts_with("--- ") || line.starts_with("diff --git") || line.starts_with("index ") {
            continue;
        } else if let Some((_, body)) = files.last_mut() {
            body.push(line);
        }
    }
    files.into_iter().map(|(path, body)| (path, parse_patch(Some(&body.join("\n"))))).collect()
}

#[cfg(test)]
mod split_tests {
    use super::*;

    #[test]
    fn splits_multi_file_diffs() {
        let diff = "--- a/x.ts\n+++ b/x.ts\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/y.ts b/y.ts\n--- a/y.ts\n+++ b/y.ts\n@@ -5 +5 @@\n-c\n+d";
        let files = split_unified_diff(diff);
        assert_eq!(files.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), vec!["x.ts", "y.ts"]);
        assert_eq!(files[1].1[0].lines[1].new_line, Some(5));
    }
}
