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

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: &str = include_str!("../../../fixtures/demo/bundle.json");

    /// The demo bundle as the raw GitHub responses `review` takes.
    fn raw_demo() -> Value {
        let demo: Value = serde_json::from_str(DEMO).unwrap();
        let pr = &demo["pr"];
        json!({
            "owner": "o", "repo": "r", "number": 7,
            "pull": {
                "title": pr["title"], "state": "open", "user": { "login": "agent" },
                "head": { "sha": pr["headSha"], "ref": "feature", "repo": { "name": "r", "owner": { "login": "fork" } } },
            },
            "files": demo["files"].as_array().unwrap().iter().map(|f| json!({
                "filename": f["path"], "status": f["status"], "patch": f["patch"],
            })).collect::<Vec<_>>(),
            "comments": [], "reportFiles": {}, "trustedAuthors": [],
        })
    }

    // Native tests only reach the success paths: `JsError` needs a JavaScript host.

    #[test]
    fn reviews_raw_responses_and_saved_bundles() {
        let out: Value = serde_json::from_str(&review(&raw_demo().to_string()).unwrap()).unwrap();
        assert_eq!(out["bundle"]["pr"]["owner"], "o");
        assert_eq!(out["focus"]["files"].as_array().unwrap().len(), out["bundle"]["files"].as_array().unwrap().len());

        let out: Value = serde_json::from_str(&review_saved_bundle(DEMO).unwrap()).unwrap();
        assert!(out["bundle"]["reports"]["manifest"].is_object());
        assert!(out["focus"]["files"][0]["tier"].is_string());
    }

    #[test]
    fn finds_report_files_on_the_head_repository() {
        let pull = raw_demo()["pull"].to_string();
        let [owner, repo, sha] = <[String; 3]>::try_from(head_repo(&pull, "o", "r").unwrap()).unwrap();
        assert_eq!((owner.as_str(), repo.as_str()), ("fork", "r"));
        assert_eq!(sha, serde_json::from_str::<Value>(DEMO).unwrap()["pr"]["headSha"]);
        assert!(report_file_paths().contains(&".reviewhub/manifest.json".to_string()));
    }

    #[test]
    fn drafts_comments_on_the_diff() {
        let diff = "diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1,2 +1,3 @@\n const a = 1;\n-const b = 2;\n+const b = 3;\n+const c = 4;\n";
        let files: Value = serde_json::from_str(&split_diff(diff)).unwrap();
        assert_eq!(files[0][0], "a.ts");
        let hunks = &files[0][1];

        let added = &hunks[0]["lines"][3];
        let target: Value = serde_json::from_str(&line_target(&added.to_string()).unwrap()).unwrap();
        assert_eq!((target["line"].as_u64(), target["side"].as_str()), (Some(3), Some("RIGHT")));
        let target: Value = serde_json::from_str(&selection_target(&hunks[0].to_string(), 2, 3).unwrap()).unwrap();
        assert_eq!((target["startLine"].as_u64(), target["line"].as_u64()), (Some(2), Some(3)));

        let finding = json!({
            "id": "x", "severity": "high", "category": "injection", "title": "Bad", "description": "Bad thing.",
            "file": "a.ts", "lines": [3, 3], "suggestion": { "explanation": "Do the safe thing." },
        });
        let draft: Value =
            serde_json::from_str(&finding_comment(&finding.to_string(), &hunks.to_string()).unwrap()).unwrap();
        assert_eq!((draft["path"].as_str(), draft["target"]["line"].as_u64()), (Some("a.ts"), Some(3)));

        let annotation = json!({ "kind": "gap", "start": 2, "end": 3, "title": "Untested" });
        let draft: Value =
            serde_json::from_str(&annotation_comment("a.ts", &annotation.to_string(), &hunks.to_string()).unwrap())
                .unwrap();
        assert!(draft["body"].as_str().unwrap().contains("Untested"), "{draft}");
    }
}
