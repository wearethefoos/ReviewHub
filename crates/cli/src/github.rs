//! GitHub REST client: fetches the raw responses for a pull request (the
//! trust rules live in `reviewhub_core::github`) and publishes report comments.

use std::collections::HashMap;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use reviewhub_core::github::{self, RawPullRequest};
use reviewhub_core::model::PullRequestBundle;
use reviewhub_core::reports::{self, Kind, Report};
use serde::Deserialize;
use serde_json::{Value, json};

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
}

#[derive(Deserialize)]
struct Comment {
    id: u64,
    body: Option<String>,
    user: Option<User>,
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

    pub fn fetch_bundle(&self, owner: &str, repo: &str, number: u32) -> Result<PullRequestBundle> {
        let pull = self
            .request("GET", &format!("/repos/{owner}/{repo}/pulls/{number}"), None)?
            .ok_or_else(|| anyhow!("{owner}/{repo}#{number} not found (or the token can't see it)"))?;
        let files = self.get_pages(&format!("/repos/{owner}/{repo}/pulls/{number}/files"), MAX_FILE_PAGES)?;
        let comments = self.get_pages(&format!("/repos/{owner}/{repo}/issues/{number}/comments"), 100)?;

        let (head_owner, head_repo, head_sha) = github::head_repo(&pull, owner, repo);
        let mut report_files = HashMap::new();
        for path in github::REPORT_FILE_PATHS {
            let url = format!("/repos/{head_owner}/{head_repo}/contents/{path}?ref={head_sha}");
            if let Some(content) = self.request("GET", &url, None)? {
                report_files.insert(path.to_string(), content);
            }
        }

        let trusted_authors = std::env::var("REVIEWHUB_TRUSTED_AUTHORS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(String::from)
            .collect();
        Ok(github::assemble(&RawPullRequest {
            owner: owner.into(),
            repo: repo.into(),
            number,
            pull,
            files,
            comments,
            report_files,
            trusted_authors,
        }))
    }

    /// Creates the report comment, or updates this user's earlier comment of the same kind.
    pub fn publish(&self, kind: Kind, report: &Report, owner: &str, repo: &str, number: u32) -> Result<String> {
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
