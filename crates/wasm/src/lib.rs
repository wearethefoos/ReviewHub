//! WebAssembly bindings of `reviewhub-core` for the browser extension.
//!
//! Values cross the boundary as JSON strings; `extension/src/lib/core.ts` wraps
//! these functions with the TypeScript types generated from the Rust types.

use reviewhub_core::focus::{FocusInput, analyze};
use reviewhub_core::github::{self, RawPullRequest};
use reviewhub_core::model::PullRequestBundle;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

fn parse<T: serde::de::DeserializeOwned>(json: &str, what: &str) -> Result<T, JsError> {
    serde_json::from_str(json).map_err(|e| JsError::new(&format!("invalid {what}: {e}")))
}

/// Raw GitHub responses (`RawPullRequest`) → `{ bundle, focus }`.
#[wasm_bindgen]
pub fn review(raw: &str) -> Result<String, JsError> {
    let raw: RawPullRequest = parse(raw, "GitHub responses")?;
    Ok(review_bundle(github::assemble(&raw)))
}

/// A saved `PullRequestBundle` (e.g. the demo) → `{ bundle, focus }`.
#[wasm_bindgen(js_name = reviewBundle)]
pub fn review_saved_bundle(bundle: &str) -> Result<String, JsError> {
    Ok(review_bundle(parse(bundle, "bundle")?))
}

fn review_bundle(bundle: PullRequestBundle) -> String {
    let focus = analyze(&FocusInput {
        files: &bundle.files,
        manifest: bundle.manifest(),
        security: bundle.security(),
        metrics: bundle.metrics(),
    });
    json!({ "bundle": bundle, "focus": focus }).to_string()
}

/// Owner, repo and head commit to read report files from: `[owner, repo, sha]`.
#[wasm_bindgen(js_name = headRepo)]
pub fn head_repo(pull: &str, owner: &str, repo: &str) -> Result<Vec<String>, JsError> {
    let pull: Value = parse(pull, "pull request")?;
    let (o, r, sha) = github::head_repo(&pull, owner, repo);
    Ok(vec![o, r, sha])
}

/// Paths of the report files to try on the head commit.
#[wasm_bindgen(js_name = reportFilePaths)]
pub fn report_file_paths() -> Vec<String> {
    github::REPORT_FILE_PATHS.iter().map(|p| p.to_string()).collect()
}

/// A multi-file unified diff → `[[path, hunks], ...]`.
#[wasm_bindgen(js_name = splitDiff)]
pub fn split_diff(diff: &str) -> String {
    json!(reviewhub_core::diff::split_unified_diff(diff)).to_string()
}
