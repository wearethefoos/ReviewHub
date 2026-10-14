//! Unified diff parsing. Port of `src/lib/diff/parse.ts`.

use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineType {
    Add,
    Del,
    Ctx,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineType,
    pub text: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

#[derive(Debug, Clone)]
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
