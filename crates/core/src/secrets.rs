//! Detects credentials in report text. Reports are published as pull request
//! comments, so a token copied from a prompt, log or diff would leak.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

static PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        ("GitHub token", r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,})"),
        ("AWS access key", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
        ("Anthropic API key", r"\bsk-ant-[A-Za-z0-9_-]{20,}"),
        ("OpenAI API key", r"\bsk-(?:proj-)?[A-Za-z0-9_-]{32,}"),
        ("Google API key", r"\bAIza[0-9A-Za-z_-]{35}"),
        ("Slack token", r"\bxox[abposr]-[A-Za-z0-9-]{10,}"),
        ("Stripe secret key", r"\b[rs]k_live_[0-9A-Za-z]{16,}"),
        ("npm token", r"\bnpm_[A-Za-z0-9]{36}\b"),
        ("JSON Web Token", r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}"),
        ("private key", r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    ]
    .into_iter()
    .map(|(name, re)| (name, Regex::new(re).expect("valid pattern")))
    .collect()
});

/// A string in a report that looks like a credential. Never holds the value itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// JSON pointer to the string, e.g. `/prompt/original`.
    pub path: String,
    /// What it looks like, e.g. "GitHub token".
    pub kind: &'static str,
}

/// Returns every string in `json` that contains something credential-shaped.
pub fn find(json: &Value) -> Vec<Finding> {
    let mut out = Vec::new();
    walk(json, &mut String::new(), &mut out);
    out
}

fn walk(value: &Value, path: &mut String, out: &mut Vec<Finding>) {
    match value {
        Value::String(s) => {
            if let Some((kind, _)) = PATTERNS.iter().find(|(_, re)| re.is_match(s)) {
                out.push(Finding { path: path.clone(), kind });
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                let len = path.len();
                path.push_str(&format!("/{i}"));
                walk(item, path, out);
                path.truncate(len);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                let len = path.len();
                path.push('/');
                path.push_str(&key.replace('~', "~0").replace('/', "~1"));
                walk(item, path, out);
                path.truncate(len);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_tokens_without_echoing_them() {
        let token = format!("ghp_{}", "a1B2".repeat(9));
        let found = find(
            &json!({ "prompt": { "original": format!("use {token} to push"), "followUps": ["ok", format!("key AKIA{}", "ABCDEFGHIJKLMNOP")] } }),
        );
        assert_eq!(
            found,
            vec![
                Finding { path: "/prompt/followUps/1".into(), kind: "AWS access key" },
                Finding { path: "/prompt/original".into(), kind: "GitHub token" },
            ]
        );
        assert!(!format!("{found:?}").contains(&token));
    }

    #[test]
    fn ignores_ordinary_text() {
        let text = "Fix the ghp_ prefix check in token.rs, rename sk-learn usage, AKIA docs, and the eyJ example.";
        assert!(find(&json!({ "summary": text, "n": 3, "ok": true })).is_empty());
        assert!(find(&serde_json::from_str(include_str!("../../../fixtures/demo/bundle.json")).unwrap()).is_empty());
    }
}
