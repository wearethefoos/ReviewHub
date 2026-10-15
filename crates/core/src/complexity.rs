//! A language-agnostic estimate of cyclomatic complexity: one plus the number
//! of decision points (branches, loops, case arms, catch clauses and boolean
//! operators), counted after removing comments and string literals. It is the
//! same measure for every language ReviewHub knows, so it is meant for
//! comparing a file before and after a change, not across languages.

use crate::model::{FileQuality, Quality};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    /// `//` and `/* */` comments, `"`, `'` and `` ` `` strings.
    CLike,
    /// Like `CLike`, but `'` starts a char literal only when it closes right away (lifetimes).
    Rust,
    /// `#` comments, `'`, `"` and triple-quoted strings.
    Python,
    /// `#` comments, `'` and `"` strings.
    Ruby,
}

fn family(path: &str) -> Option<Family> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "rs" => Family::Rust,
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" | "go" | "java" | "kt" | "kts" | "swift" | "c"
        | "h" | "cc" | "cpp" | "cxx" | "hpp" | "cs" | "php" | "scala" | "dart" => Family::CLike,
        "py" => Family::Python,
        "rb" => Family::Ruby,
        _ => return None,
    })
}

fn decision_words(family: Family) -> &'static [&'static str] {
    match family {
        Family::CLike => &["if", "for", "while", "case", "catch"],
        Family::Rust => &["if", "for", "while", "loop"],
        Family::Python => &["if", "elif", "for", "while", "except", "case", "and", "or"],
        Family::Ruby => &["if", "elsif", "unless", "while", "until", "for", "when", "rescue", "and", "or"],
    }
}

/// Replaces comments and string literals with spaces, keeping line structure.
fn strip(text: &str, family: Family) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let hash_comments = matches!(family, Family::Python | Family::Ruby);
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let line_comment = (!hash_comments && c == '/' && next == Some('/')) || (hash_comments && c == '#');
        if line_comment {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if !hash_comments && c == '/' && next == Some('*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                if chars[i] == '\n' {
                    out.push('\n');
                }
                i += 1;
            }
            i += 2;
            continue;
        }
        let quote = match c {
            '"' => true,
            '`' => family == Family::CLike,
            '\'' => match family {
                // A char literal ('x', '\n', '\u{..}') closes within a few characters; a lifetime doesn't.
                Family::Rust => chars[i + 1..]
                    .iter()
                    .take(10)
                    .position(|&q| q == '\'')
                    .is_some_and(|p| p == 1 || (chars.get(i + 1) == Some(&'\\'))),
                _ => true,
            },
            _ => false,
        };
        if quote {
            let triple = family == Family::Python && next == Some(c) && chars.get(i + 2) == Some(&c);
            let len = if triple { 3 } else { 1 };
            i += len;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == c && (!triple || (chars.get(i + 1) == Some(&c) && chars.get(i + 2) == Some(&c))) {
                    i += len;
                    break;
                }
                if chars[i] == '\n' {
                    out.push('\n');
                }
                i += 1;
            }
            out.push(' ');
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The complexity estimate of a source file, or `None` for files that aren't code ReviewHub knows.
pub fn complexity(path: &str, text: &str) -> Option<f64> {
    let family = family(path)?;
    let code = strip(text, family);
    let words = decision_words(family);
    let mut count = 0usize;
    for word in code.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if words.contains(&word) {
            count += 1;
        }
    }
    if matches!(family, Family::CLike | Family::Rust) {
        count += code.matches("&&").count() + code.matches("||").count();
    }
    if family == Family::Rust {
        // Match arms; closures `|x|` don't contain `=>`.
        count += code.matches("=>").count();
    }
    Some(1.0 + count as f64)
}

/// A changed file's contents before and after the change (`None` when it didn't exist).
pub struct ChangedSource<'a> {
    pub path: &'a str,
    pub base: Option<&'a str>,
    pub head: Option<&'a str>,
}

/// Complexity of each changed code file after the change, and how much the change added.
pub fn quality(files: &[ChangedSource]) -> Quality {
    let files = files
        .iter()
        .filter_map(|f| {
            let head = complexity(f.path, f.head?)?;
            let base = f.base.and_then(|b| complexity(f.path, b)).unwrap_or(0.0);
            Some(FileQuality {
                path: f.path.to_string(),
                complexity: Some(head),
                complexity_delta: Some(head - base),
                duplication: None,
                lint_issues: None,
            })
        })
        .collect();
    Quality { tool: Some("reviewhub complexity".into()), files }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_decisions_but_not_comments_or_strings() {
        let ts = r#"
            // if this were counted it would be wrong
            function f(a, b) {
              if (a && b) return "if for while";
              for (const x of a) { /* while */ }
              return a ? 1 : `case ${b}`;
            }"#;
        assert_eq!(complexity("src/f.ts", ts), Some(4.0)); // 1 + if + && + for
        assert_eq!(complexity("README.md", ts), None);
    }

    #[test]
    fn handles_rust_lifetimes_and_match_arms() {
        let rs = "fn f<'a>(x: &'a str) -> u8 { match x { \"a\" => 1, _ => 2 } }\nfn g(c: char) -> bool { c == '{' || c == '\\'' }";
        assert_eq!(complexity("lib.rs", rs), Some(4.0)); // 1 + two arms + ||
    }

    #[test]
    fn handles_python() {
        let py = "def f(x):\n    \"\"\"if docstring\"\"\"\n    if x and x > 1:  # or comment\n        return [y for y in x]\n    elif x: pass\n";
        assert_eq!(complexity("f.py", py), Some(5.0)); // 1 + if + and + for + elif
    }

    #[test]
    fn reports_complexity_and_delta() {
        let q = quality(&[
            ChangedSource { path: "a.ts", base: Some("if (x) {}"), head: Some("if (x) {} if (y) {}") },
            ChangedSource { path: "new.py", base: None, head: Some("if x: pass") },
            ChangedSource { path: "gone.ts", base: Some("if (x) {}"), head: None },
            ChangedSource { path: "data.json", base: None, head: Some("{}") },
        ]);
        let got: Vec<_> = q.files.iter().map(|f| (f.path.as_str(), f.complexity, f.complexity_delta)).collect();
        assert_eq!(got, vec![("a.ts", Some(3.0), Some(1.0)), ("new.py", Some(2.0), Some(2.0))]);
    }
}
