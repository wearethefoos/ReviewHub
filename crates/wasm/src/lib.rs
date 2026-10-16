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

/// The review comment target for one diff line (`DiffLine` → `CommentTarget`).
#[wasm_bindgen(js_name = lineTarget)]
pub fn line_target(line: &str) -> Result<String, JsError> {
    let line: reviewhub_core::diff::DiffLine = parse(line, "diff line")?;
    Ok(json!(reviewhub_core::review::line_target(&line)).to_string())
}

/// A draft review comment for a security finding on its file's hunks, or `null`.
#[wasm_bindgen(js_name = findingComment)]
pub fn finding_comment(finding: &str, hunks: &str) -> Result<String, JsError> {
    let finding: reviewhub_core::model::SecurityFinding = parse(finding, "security finding")?;
    let hunks: Vec<reviewhub_core::diff::Hunk> = parse(hunks, "hunks")?;
    Ok(json!(reviewhub_core::review::finding_comment(&finding, &hunks)).to_string())
}

/// A draft review comment for an annotation on a file's hunks, or `null`.
#[wasm_bindgen(js_name = annotationComment)]
pub fn annotation_comment(path: &str, annotation: &str, hunks: &str) -> Result<String, JsError> {
    let annotation: reviewhub_core::focus::Annotation = parse(annotation, "annotation")?;
    let hunks: Vec<reviewhub_core::diff::Hunk> = parse(hunks, "hunks")?;
    Ok(json!(reviewhub_core::review::annotation_comment(path, &annotation, &hunks)).to_string())
}

/// The review comment target for the selected rows `from..=to` of a hunk, or `null`.
#[wasm_bindgen(js_name = selectionTarget)]
pub fn selection_target(hunk: &str, from: usize, to: usize) -> Result<String, JsError> {
    let hunk: reviewhub_core::diff::Hunk = parse(hunk, "hunk")?;
    Ok(json!(reviewhub_core::review::selection_target(&hunk, from, to)).to_string())
}

/// What to tell a user of this kind of install (`plugin`, `repo`, `cli`, `extension-unpacked`) when `latest` is
/// newer than `current`, or `undefined`.
#[wasm_bindgen(js_name = updateNotice)]
pub fn update_notice(install: &str, current: &str, latest: &str) -> Option<String> {
    reviewhub_core::update::notice(reviewhub_core::update::Install::parse(install)?, current, latest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_notice_knows_the_install_kinds() {
        let notice = update_notice("extension-unpacked", "0.5.0", "v0.6.0").unwrap();
        assert!(
            notice.starts_with("ReviewHub 0.6.0 is available (you have 0.5.0).") && notice.contains("chromewebstore")
        );
        assert_eq!(update_notice("extension-unpacked", "0.6.0", "v0.6.0"), None);
        assert_eq!(update_notice("browser", "0.5.0", "v0.6.0"), None);
    }
}
