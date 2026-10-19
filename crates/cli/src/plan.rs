//! `reviewhub security-plan` and `security-merge`: the I/O around the batched
//! security review planned in `reviewhub_core::plan`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use reviewhub_core::model::{PullRequestBundle, Scanner, SecurityReport};
use reviewhub_core::plan::{self, Batch, Limits, Mode, Models};
use reviewhub_core::reports::{self, Kind, Report};

const AGENT: &str = include_str!("../../../plugin/agents/security-reviewer.md");

/// The models for each tier: `REVIEWHUB_MODEL_STRONG`, `_STANDARD` and `_LIGHT`, else opus, sonnet and haiku.
pub fn models(var: impl Fn(&str) -> Option<String>) -> Models {
    let defaults = Models::default();
    let pick = |name: &str, default: String| var(name).filter(|v| !v.trim().is_empty()).unwrap_or(default);
    Models {
        strong: pick("REVIEWHUB_MODEL_STRONG", defaults.strong),
        standard: pick("REVIEWHUB_MODEL_STANDARD", defaults.standard),
        light: pick("REVIEWHUB_MODEL_LIGHT", defaults.light),
    }
}

/// The reviewer agent's "What to look for" section, so batch reviewers look for the same things.
fn what_to_look_for() -> &'static str {
    let start = AGENT.find("## What to look for").expect("the agent has a What to look for section");
    let rest = &AGENT[start..];
    let body = rest.split_once('\n').map_or(rest, |(_, body)| body);
    body.split("\n## ").next().unwrap_or(body).trim()
}

/// Instructions for one batch's reviewer: complete in themselves, so any subagent can follow them.
pub fn batch_prompt(pr: &str, head: &str, batch: &Batch, output: &Path, notes: &Path) -> String {
    let files: String = batch
        .files
        .iter()
        .map(|f| {
            let sensitive = f.sensitive.as_deref().map(|s| format!(", {s}")).unwrap_or_default();
            format!("- {} ({}, +{} −{}{sensitive})\n", f.path, f.status, f.additions, f.deletions)
        })
        .collect();
    format!(
        "You are one of several independent security reviewers of pull request {pr} (head commit {head}); each \
         reviews a few of its files closely. You did not write this code. Don't trust the pull request's \
         description, its review manifest or comments in the code about what the code does: read the code.\n\n\
         Your files:\n{files}\n\
         1. Read their diffs with `reviewhub review {pr} --json --patches` (only your files are yours) and the code \
         around them with `git show {head}:<path>`. Read other files where you need to know how your files are \
         called or what they call, but report findings only in your files. Don't create or modify any files except the report and the notes \
         file named below.\n\
         2. What to look for:\n\n{look}\n\n\
         3. Write a ReviewHub security report (schema: `reviewhub schema security`) to {out}: `scanner.name` \
         \"reviewhub security-reviewer ({id})\", `scanner.model` \"{model}\", `commit` \"{head}\", a one or two \
         sentence `summary`, and for each finding `id`, `title`, `severity`, `confidence`, `category` (a CWE id where \
         one fits), `file`, `lines` in the new version, a `description` of how it can be exploited, and \
         `suggestion.explanation` (plus `suggestion.diff`, a unified diff that applies with `git apply` on the head \
         commit, when the fix is local and certain). Report only findings you can tie to specific lines; an empty \
         `findings` list is a valid result. Never copy a secret's value into the report. Run `reviewhub validate \
         security {out}` and fix every error.\n\
         4. Write cross-file notes to {notes}: one line for each thing the reviewer of the whole pull request should \
         check across files: an assumption your files make about other code (\"routes/refunds.ts trusts \
         req.merchantId; check that the middleware sets it from the session\"), a changed function or contract that \
         other files rely on, or untrusted input that leaves your files. Write \"none\" when there are none. Reply \
         with the two paths.",
        notes = notes.display(),
        look = what_to_look_for(),
        out = output.display(),
        id = batch.id,
        model = batch.model,
    )
}

/// A new directory only this user can read.
pub fn private_dir() -> Result<PathBuf> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("reviewhub-security-{}-{nanos}", std::process::id()));
    std::fs::create_dir(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}

/// The plan as JSON, with each batch's instructions and report path.
pub fn plan_json(bundle: &PullRequestBundle, models: &Models, out_dir: Option<PathBuf>) -> Result<Value> {
    let plan = plan::security_plan(&bundle.files, models, &Limits::default());
    let pr = format!("{}/{}#{}", bundle.pr.owner, bundle.pr.repo, bundle.pr.number);
    let head = &bundle.pr.head_sha;
    let mut out = serde_json::to_value(&plan)?;
    out["pr"] = json!(pr);
    out["headSha"] = json!(head);
    if plan.mode == Mode::Batched {
        let dir = match out_dir {
            Some(dir) => {
                std::fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
                dir
            }
            None => private_dir()?,
        };
        for (i, batch) in plan.batches.iter().enumerate() {
            let output = dir.join(format!("{}.json", batch.id));
            let notes = dir.join(format!("{}.notes.md", batch.id));
            out["batches"][i]["output"] = json!(output.display().to_string());
            out["batches"][i]["notes"] = json!(notes.display().to_string());
            out["batches"][i]["prompt"] = json!(batch_prompt(&pr, head, batch, &output, &notes));
        }
        // The reviewer of the whole pull request reads the plan from here once the batches are done.
        let plan_file = dir.join("plan.json");
        out["planFile"] = json!(plan_file.display().to_string());
        std::fs::write(&plan_file, serde_json::to_string_pretty(&out)?)
            .with_context(|| format!("cannot write {}", plan_file.display()))?;
    }
    Ok(out)
}

/// The batches' reports combined into one, for the pass across files.
pub fn merge(files: &[PathBuf], model: Option<String>) -> Result<Report> {
    let mut batches = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(file).with_context(|| format!("cannot read {}", file.display()))?;
        let json: Value =
            serde_json::from_str(&text).with_context(|| format!("{} is not valid JSON", file.display()))?;
        let report = reports::validate(Kind::Security, &json)
            .map_err(|e| anyhow!("{} is not a valid security report:\n{e}", file.display()))?;
        let report: SecurityReport = serde_json::from_value(report.to_value())?;
        let id = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        batches.push((id, report));
    }
    let scanner = Scanner { name: "reviewhub security-reviewer".into(), model };
    let merged = plan::merge_security(&batches, scanner, None);
    reports::validate(Kind::Security, &serde_json::to_value(&merged)?).map_err(|e| anyhow!(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_models_from_the_environment() {
        let var = |name: &str| (name == "REVIEWHUB_MODEL_STRONG").then(|| "fable".to_string());
        let m = models(var);
        assert_eq!((m.strong.as_str(), m.standard.as_str(), m.light.as_str()), ("fable", "sonnet", "haiku"));
        assert_eq!(models(|_| Some(" ".into())), Models::default());
    }

    #[test]
    fn batch_prompts_carry_the_reviewers_checklist() {
        let look = what_to_look_for();
        assert!(look.starts_with("Problems the change introduces") && look.contains("injection"), "{look}");
        assert!(!look.contains("## "), "{look}");
    }
}
