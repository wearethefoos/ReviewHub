//! Builds a [`PullRequestBundle`] from raw GitHub REST responses. No I/O here:
//! the CLI and the extension fetch the responses and hand them over, so both
//! apply the same trust rules.
//!
//! Requests a caller makes (all `GET`, paginate with `per_page=100`):
//! - `/repos/{owner}/{repo}/pulls/{number}`
//! - `/repos/{owner}/{repo}/pulls/{number}/files` (all pages, GitHub caps at 3000 files)
//! - `/repos/{owner}/{repo}/issues/{number}/comments` (all pages)
//! - for each of [`REPORT_FILE_PATHS`]: `/repos/{head owner}/{head repo}/contents/{path}?ref={head sha}`
//!   (see [`head_repo`]); skip the ones that return 404.

use std::collections::{HashMap, HashSet};

use base64::Engine;
use serde::Deserialize;
use serde_json::Value;

use crate::model::{
    Author, ChangedFile, PrState, PullRequestBundle, PullRequestInfo, ReportEnvelope, ReportSource, Reports,
};
use crate::reports::{self, Kind, Report};

/// Comment authors whose reports are trusted. Anyone else could post a fake
/// manifest that steers reviewers away from code. Bots are not trusted as a
/// class because some of them echo other people's comments.
const TRUSTED_ASSOCIATIONS: [&str; 3] = ["OWNER", "MEMBER", "COLLABORATOR"];
const TRUSTED_BOTS: [&str; 1] = ["github-actions[bot]"];

/// Report files read from the head commit when no report comment exists.
pub const REPORT_FILE_PATHS: [&str; 3] =
    [".reviewhub/manifest.json", ".reviewhub/security.json", ".reviewhub/metrics.json"];

/// The raw responses for one pull request.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPullRequest {
    pub owner: String,
    pub repo: String,
    pub number: u32,
    /// Response of `GET /repos/{owner}/{repo}/pulls/{number}`.
    pub pull: Value,
    /// All pages of `GET .../pulls/{number}/files`, concatenated.
    #[serde(default)]
    pub files: Vec<Value>,
    /// All pages of `GET .../issues/{number}/comments`, concatenated.
    #[serde(default)]
    pub comments: Vec<Value>,
    /// Contents API responses for the report files that exist, keyed by path.
    #[serde(default)]
    pub report_files: HashMap<String, Value>,
    /// Extra logins whose report comments are trusted (case-insensitive).
    #[serde(default)]
    pub trusted_authors: Vec<String>,
}

/// Owner, name and head commit of the repository the pull request's branch lives in.
pub fn head_repo(pull: &Value, owner: &str, repo: &str) -> (String, String, String) {
    let s = |v: &Value, default: &str| v.as_str().unwrap_or(default).to_string();
    (
        s(&pull["head"]["repo"]["owner"]["login"], owner),
        s(&pull["head"]["repo"]["name"], repo),
        s(&pull["head"]["sha"], ""),
    )
}

#[derive(Deserialize)]
struct User {
    login: String,
}

#[derive(Deserialize)]
struct Comment {
    html_url: String,
    body: Option<String>,
    user: Option<User>,
    #[serde(default)]
    author_association: String,
    #[serde(default)]
    updated_at: String,
}

pub fn assemble(raw: &RawPullRequest) -> PullRequestBundle {
    let pr = &raw.pull;
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let n = |v: &Value| v.as_u64().unwrap_or_default() as u32;
    let pr_author = s(&pr["user"]["login"]);
    let head_sha = s(&pr["head"]["sha"]);

    let files: Vec<ChangedFile> = raw
        .files
        .iter()
        .map(|f| ChangedFile {
            path: s(&f["filename"]),
            previous_path: f["previous_filename"].as_str().map(String::from),
            status: s(&f["status"]),
            additions: n(&f["additions"]),
            deletions: n(&f["deletions"]),
            patch: f["patch"].as_str().map(String::from),
        })
        // Report files are tooling output, not part of the change under review.
        .filter(|f| !f.path.starts_with(".reviewhub/"))
        .collect();

    let mut reports = Reports::default();
    let mut found: HashSet<Kind> = HashSet::new();
    let extra: HashSet<String> = raw.trusted_authors.iter().map(|a| a.trim().to_lowercase()).collect();

    // Newest comment of each kind wins.
    let mut comments: Vec<Comment> =
        raw.comments.iter().filter_map(|c| serde_json::from_value(c.clone()).ok()).collect();
    comments.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    for c in &comments {
        let Some(body) = &c.body else { continue };
        let login = c.user.as_ref().map(|u| u.login.to_lowercase()).unwrap_or_default();
        let trusted = TRUSTED_ASSOCIATIONS.contains(&c.author_association.as_str())
            || TRUSTED_BOTS.contains(&login.as_str())
            || login == pr_author.to_lowercase()
            || extra.contains(&login);
        if !trusted {
            continue;
        }
        let Some(extracted) = reports::extract(body) else { continue };
        if !found.insert(extracted.kind) {
            continue;
        }
        let source = ReportSource::Comment {
            url: c.html_url.clone(),
            author: c.user.as_ref().map(|u| u.login.clone()).unwrap_or_else(|| "unknown".into()),
            updated_at: c.updated_at.clone(),
        };
        store(&mut reports, extracted.kind, source, extracted.result);
    }

    // Fall back to report files committed on the head branch.
    for (kind, path) in Kind::ALL.into_iter().zip(REPORT_FILE_PATHS) {
        if found.contains(&kind) {
            continue;
        }
        let Some(content) = raw.report_files.get(path) else { continue };
        let encoded: String = content["content"].as_str().unwrap_or_default().split_whitespace().collect();
        let result = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| format!("Invalid base64 content: {e}"))
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).map_err(|e| format!("Invalid JSON: {e}")))
            .and_then(|json| reports::validate(kind, &json));
        store(&mut reports, kind, ReportSource::File { path: path.into(), git_ref: head_sha.clone() }, result);
    }

    PullRequestBundle {
        pr: PullRequestInfo {
            owner: raw.owner.clone(),
            repo: raw.repo.clone(),
            number: raw.number,
            title: s(&pr["title"]),
            body: s(&pr["body"]),
            state: if pr["merged_at"].is_string() {
                PrState::Merged
            } else if pr["state"] == "closed" {
                PrState::Closed
            } else {
                PrState::Open
            },
            draft: pr["draft"].as_bool().unwrap_or(false),
            author: Author {
                login: if pr_author.is_empty() { "unknown".into() } else { pr_author.clone() },
                avatar_url: s(&pr["user"]["avatar_url"]),
            },
            base_ref: s(&pr["base"]["ref"]),
            head_ref: s(&pr["head"]["ref"]),
            head_sha,
            html_url: s(&pr["html_url"]),
            created_at: s(&pr["created_at"]),
            additions: n(&pr["additions"]),
            deletions: n(&pr["deletions"]),
            changed_files: n(&pr["changed_files"]),
            commits: n(&pr["commits"]),
        },
        files,
        reports,
    }
}

fn store(reports: &mut Reports, kind: Kind, source: ReportSource, result: Result<Report, String>) {
    macro_rules! envelope {
        ($variant:ident) => {
            match result {
                Ok(Report::$variant(data)) => ReportEnvelope { source, data: Some(data), error: None },
                Ok(_) => unreachable!("validate returns the requested kind"),
                Err(error) => ReportEnvelope { source, data: None, error: Some(error) },
            }
        };
    }
    match kind {
        Kind::Manifest => reports.manifest = Some(envelope!(Manifest)),
        Kind::Security => reports.security = Some(envelope!(Security)),
        Kind::Metrics => reports.metrics = Some(envelope!(Metrics)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn metrics_comment(lines: u32) -> String {
        let report =
            reports::validate(Kind::Metrics, &json!({ "version": "1", "coverage": { "lines": lines } })).unwrap();
        reports::render_comment(Kind::Metrics, &report)
    }

    fn comment(login: &str, association: &str, at: &str, body: String) -> Value {
        json!({ "html_url": format!("https://x/{login}"), "body": body, "user": { "login": login },
                "author_association": association, "updated_at": at })
    }

    fn raw(comments: Vec<Value>) -> RawPullRequest {
        RawPullRequest {
            owner: "o".into(),
            repo: "r".into(),
            number: 1,
            pull: json!({ "title": "t", "state": "open", "user": { "login": "claude[bot]" },
                          "head": { "ref": "h", "sha": "abc" }, "base": { "ref": "main" } }),
            files: vec![
                json!({ "filename": "src/a.ts", "status": "added", "additions": 1, "deletions": 0, "patch": "@@ -0,0 +1 @@\n+x" }),
                json!({ "filename": ".reviewhub/metrics.json", "status": "added", "additions": 1, "deletions": 0 }),
            ],
            comments,
            ..Default::default()
        }
    }

    fn coverage(bundle: &PullRequestBundle) -> Option<f64> {
        bundle.metrics()?.coverage.as_ref()?.lines
    }

    #[test]
    fn ignores_untrusted_comments_even_when_newer() {
        let bundle = assemble(&raw(vec![
            comment("mallory", "NONE", "2026-01-02", metrics_comment(99)),
            comment("alice", "MEMBER", "2026-01-01", metrics_comment(50)),
        ]));
        assert_eq!(coverage(&bundle), Some(50.0));
        assert!(
            matches!(&bundle.reports.metrics.as_ref().unwrap().source, ReportSource::Comment { author, .. } if author == "alice")
        );
    }

    #[test]
    fn trusts_the_pr_author_actions_bot_and_configured_logins() {
        for (login, extra) in
            [("claude[bot]", vec![]), ("github-actions[bot]", vec![]), ("Ci-Bot", vec!["ci-bot".to_string()])]
        {
            let mut r = raw(vec![comment(login, "NONE", "2026-01-01", metrics_comment(70))]);
            r.trusted_authors = extra;
            assert_eq!(coverage(&assemble(&r)), Some(70.0), "{login}");
        }
    }

    #[test]
    fn newest_trusted_comment_wins() {
        let bundle = assemble(&raw(vec![
            comment("alice", "OWNER", "2026-01-01", metrics_comment(10)),
            comment("bob", "COLLABORATOR", "2026-01-03", metrics_comment(30)),
        ]));
        assert_eq!(coverage(&bundle), Some(30.0));
    }

    #[test]
    fn falls_back_to_report_files_and_hides_them_from_the_diff() {
        let mut r = raw(vec![]);
        let content = base64::engine::general_purpose::STANDARD.encode(r#"{"version":"1","coverage":{"lines":42}}"#);
        r.report_files.insert(".reviewhub/metrics.json".into(), json!({ "content": content }));
        let bundle = assemble(&r);
        assert_eq!(coverage(&bundle), Some(42.0));
        assert_eq!(bundle.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), vec!["src/a.ts"]);
    }

    #[test]
    fn keeps_invalid_reports_with_their_error() {
        let bundle = assemble(&raw(vec![comment(
            "alice",
            "OWNER",
            "2026-01-01",
            "<!-- reviewhub:security -->\n```json\n{}\n```".into(),
        )]));
        let security = bundle.reports.security.unwrap();
        assert!(security.data.is_none());
        assert!(security.error.unwrap().contains("required"));
    }
}
