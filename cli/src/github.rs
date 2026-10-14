//! Minimal GitHub REST client: loads a pull request with its ReviewHub reports
//! (same rules as `src/lib/sources/github.ts`) and publishes report comments.

use std::collections::HashSet;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::model::{Author, ChangedFile, PullRequestBundle, PullRequestInfo, ReportEnvelope, ReportSource, Reports};
use crate::reports::{self, Kind, Report};

/// Comment authors whose reports are trusted. Anyone else could post a fake
/// manifest that steers reviewers away from code.
const TRUSTED_ASSOCIATIONS: [&str; 3] = ["OWNER", "MEMBER", "COLLABORATOR"];
const TRUSTED_BOTS: [&str; 1] = ["github-actions[bot]"];
const MAX_FILE_PAGES: u32 = 30; // 3000 files, GitHub's own limit

pub struct GitHub {
    agent: ureq::Agent,
    api: String,
    token: Option<String>,
}

/// `GITHUB_TOKEN`, falling back to the GitHub CLI's login.
pub fn resolve_token() -> Option<String> {
    if let Ok(token) = std::env::var("GITHUB_TOKEN")
        && !token.is_empty()
    {
        return Some(token);
    }
    let output = Command::new("gh").args(["auth", "token"]).output().ok()?;
    let token = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (output.status.success() && !token.is_empty()).then_some(token)
}

#[derive(Deserialize)]
struct User {
    login: String,
    #[serde(default)]
    avatar_url: String,
}

#[derive(Deserialize)]
struct Comment {
    id: u64,
    html_url: String,
    body: Option<String>,
    user: Option<User>,
    author_association: String,
    updated_at: String,
}

impl GitHub {
    pub fn new(token: Option<String>) -> Self {
        let config = ureq::Agent::config_builder()
            .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
            .user_agent("reviewhub-cli")
            .http_status_as_error(false)
            .build();
        let api = std::env::var("GITHUB_API_URL").unwrap_or_else(|_| "https://api.github.com".into());
        GitHub { agent: config.new_agent(), api: api.trim_end_matches('/').to_string(), token }
    }

    fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<Option<Value>> {
        let url = format!("{}{path}", self.api);
        let mut builder = ureq::http::Request::builder()
            .method(method)
            .uri(&url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &self.token {
            builder = builder.header("Authorization", format!("Bearer {token}"));
        }
        let mut response = match body {
            Some(body) => self.agent.run(builder.header("Content-Type", "application/json").body(body.to_string())?),
            None => self.agent.run(builder.body(())?),
        }
        .with_context(|| format!("{method} {url}"))?;

        let status = response.status().as_u16();
        if status == 404 {
            return Ok(None);
        }
        let text = response.body_mut().read_to_string()?;
        match status {
            200..=299 => Ok(Some(serde_json::from_str(&text)?)),
            401 => bail!("GitHub rejected the token (401). Set GITHUB_TOKEN or run `gh auth login`."),
            403 | 429 => bail!(
                "GitHub refused {method} {path} ({status}): the token lacks access or the rate limit was hit.{}",
                if self.token.is_none() { " No token is set; set GITHUB_TOKEN or run `gh auth login`." } else { "" }
            ),
            _ => bail!("{method} {path} failed with {status}: {text}"),
        }
    }

    fn get(&self, path: &str) -> Result<Value> {
        self.request("GET", path, None)?.ok_or_else(|| anyhow!("Not found: {path}"))
    }

    fn get_pages<T: for<'de> Deserialize<'de>>(&self, path: &str, max_pages: u32) -> Result<Vec<T>> {
        let mut all = Vec::new();
        for page in 1..=max_pages {
            let items: Vec<T> = serde_json::from_value(self.get(&format!("{path}?per_page=100&page={page}"))?)?;
            let done = items.len() < 100;
            all.extend(items);
            if done {
                break;
            }
        }
        Ok(all)
    }

    pub fn fetch_bundle(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequestBundle> {
        let pr = self
            .request("GET", &format!("/repos/{owner}/{repo}/pulls/{number}"), None)?
            .ok_or_else(|| anyhow!("{owner}/{repo}#{number} not found (or the token can't see it)"))?;
        let files: Vec<Value> =
            self.get_pages(&format!("/repos/{owner}/{repo}/pulls/{number}/files"), MAX_FILE_PAGES)?;
        let comments: Vec<Comment> = self.get_pages(&format!("/repos/{owner}/{repo}/issues/{number}/comments"), 100)?;

        let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
        let n = |v: &Value| v.as_u64().unwrap_or_default();
        let pr_author = s(&pr["user"]["login"]);
        let head_sha = s(&pr["head"]["sha"]);

        let files: Vec<ChangedFile> = files
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
        let mut found: HashSet<&'static str> = HashSet::new();
        let extra_trusted: HashSet<String> = std::env::var("REVIEWHUB_TRUSTED_AUTHORS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        // Newest comment of each kind wins.
        let mut comments = comments;
        comments.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        for c in &comments {
            let Some(body) = &c.body else { continue };
            let login = c.user.as_ref().map(|u| u.login.to_lowercase()).unwrap_or_default();
            let trusted = TRUSTED_ASSOCIATIONS.contains(&c.author_association.as_str())
                || TRUSTED_BOTS.contains(&login.as_str())
                || login == pr_author.to_lowercase()
                || extra_trusted.contains(&login);
            if !trusted {
                continue;
            }
            let Some(extracted) = reports::extract(body) else { continue };
            if !found.insert(extracted.kind.as_str()) {
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
        let head_owner = pr["head"]["repo"]["owner"]["login"].as_str().unwrap_or(owner).to_string();
        let head_repo = pr["head"]["repo"]["name"].as_str().unwrap_or(repo).to_string();
        for kind in Kind::ALL {
            if found.contains(kind.as_str()) {
                continue;
            }
            let path = format!(".reviewhub/{}.json", kind.as_str());
            let Some(content) =
                self.request("GET", &format!("/repos/{head_owner}/{head_repo}/contents/{path}?ref={head_sha}"), None)?
            else {
                continue;
            };
            let encoded: String = s(&content["content"]).split_whitespace().collect();
            let result = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| e.to_string())
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).map_err(|e| format!("Invalid JSON: {e}")))
                .and_then(|json| reports::validate(kind, &json));
            store(&mut reports, kind, ReportSource::File { path, git_ref: head_sha.clone() }, result);
        }

        let user: Option<User> = serde_json::from_value(pr["user"].clone()).ok();
        Ok(PullRequestBundle {
            pr: PullRequestInfo {
                owner: owner.into(),
                repo: repo.into(),
                number,
                title: s(&pr["title"]),
                body: s(&pr["body"]),
                state: if pr["merged_at"].is_string() { "merged".into() } else { s(&pr["state"]) },
                draft: pr["draft"].as_bool().unwrap_or(false),
                author: Author {
                    login: user.as_ref().map(|u| u.login.clone()).unwrap_or_else(|| "unknown".into()),
                    avatar_url: user.map(|u| u.avatar_url).unwrap_or_default(),
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
        })
    }

    /// Creates the report comment, or updates this user's earlier comment of the same kind.
    pub fn publish(&self, kind: Kind, report: &Report, owner: &str, repo: &str, number: u64) -> Result<String> {
        if self.token.is_none() {
            bail!("publishing needs a token: set GITHUB_TOKEN or run `gh auth login`");
        }
        let me = self.get("/user").ok().and_then(|u| u["login"].as_str().map(String::from));
        let marker = kind.marker();
        let comments: Vec<Comment> = self.get_pages(&format!("/repos/{owner}/{repo}/issues/{number}/comments"), 100)?;
        let existing = comments.iter().find(|c| {
            c.body.as_deref().is_some_and(|b| b.contains(&marker))
                && me.as_deref().is_none_or(|me| c.user.as_ref().is_some_and(|u| u.login == me))
        });

        let body = json!({ "body": reports::render_comment(kind, report) });
        let result = match existing {
            Some(c) => self.request("PATCH", &format!("/repos/{owner}/{repo}/issues/comments/{}", c.id), Some(body))?,
            None => self.request("POST", &format!("/repos/{owner}/{repo}/issues/{number}/comments"), Some(body))?,
        };
        let url = result.and_then(|r| r["html_url"].as_str().map(String::from));
        url.ok_or_else(|| anyhow!("{owner}/{repo}#{number} not found"))
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
