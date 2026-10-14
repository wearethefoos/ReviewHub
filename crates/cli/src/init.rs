//! `reviewhub init`: installs ReviewHub into a repository, for Claude Code
//! sessions that can't install plugins (cloud sessions, including ones started
//! from the mobile app). Writes the plugin's skills and subagent under
//! `.claude/`, a launcher pinned to this CLI's version, hooks in
//! `.claude/settings.json`, an opt-in note in `CLAUDE.md`, and the briefing
//! workflow. The plugin's files are compiled in, so there is one source.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Names change from the plugin's namespaced ones (`reviewhub:review`) to
/// prefixed ones, since repository skills have no namespace.
const RENAMES: &[(&str, &str)] = &[
    ("reviewhub:review-manifest", "reviewhub-manifest"),
    ("reviewhub:security-review", "reviewhub-security-review"),
    ("`review-manifest`", "`reviewhub-manifest`"),
    ("`security-reviewer`", "`reviewhub-security-reviewer`"),
    ("`security-review`", "`reviewhub-security-review`"),
    ("by the security-review skill", "by the reviewhub-security-review skill"),
    ("\nname: review-manifest\n", "\nname: reviewhub-manifest\n"),
    ("\nname: security-reviewer\n", "\nname: reviewhub-security-reviewer\n"),
    ("\nname: security-review\n", "\nname: reviewhub-security-review\n"),
    ("\nname: review\n", "\nname: reviewhub-review\n"),
];

fn rename(text: &str) -> String {
    RENAMES.iter().fold(text.to_string(), |text, (from, to)| text.replace(from, to))
}

const MARKER_START: &str = "<!-- reviewhub:start -->";
const MARKER_END: &str = "<!-- reviewhub:end -->";
const HOOK_DIR: &str = ".claude/reviewhub/";

struct Generated {
    path: &'static str,
    content: String,
    executable: bool,
}

fn generated(with_workflow: bool) -> Vec<Generated> {
    let file = |path, content: &str, executable| Generated { path, content: content.to_string(), executable };
    let mut files = vec![
        file(
            ".claude/skills/reviewhub-review/SKILL.md",
            &rename(include_str!("../../../plugin/skills/review/SKILL.md")),
            false,
        ),
        file(
            ".claude/skills/reviewhub-manifest/SKILL.md",
            &rename(include_str!("../../../plugin/skills/review-manifest/SKILL.md")),
            false,
        ),
        file(
            ".claude/skills/reviewhub-security-review/SKILL.md",
            &rename(include_str!("../../../plugin/skills/security-review/SKILL.md")),
            false,
        ),
        file(
            ".claude/agents/reviewhub-security-reviewer.md",
            &rename(include_str!("../../../plugin/agents/security-reviewer.md")),
            false,
        ),
        file(".claude/reviewhub/bin/reviewhub", include_str!("../../../plugin/bin/reviewhub"), true),
        file(".claude/reviewhub/bin/VERSION", &format!("{VERSION}\n"), false),
        file(".claude/reviewhub/session-start.sh", include_str!("../templates/session-start.sh"), true),
        file(".claude/reviewhub/pr-created.sh", &rename(include_str!("../../../plugin/scripts/pr-created.sh")), true),
    ];
    if with_workflow {
        files.push(file(
            ".github/workflows/reviewhub.yml",
            &include_str!("../templates/reviewhub.yml").replace("{{VERSION}}", VERSION),
            false,
        ));
    }
    files
}

fn claude_md_block() -> String {
    format!(
        "{MARKER_START}\n## ReviewHub\n\n\
         Pull requests in this repository are reviewed with ReviewHub. Publish a review manifest with every pull \
         request you open or update (reviewhub-manifest skill). The `reviewhub` CLI (v{VERSION}) is on PATH in \
         Claude Code sessions; elsewhere run `.claude/reviewhub/bin/reviewhub`. To update these files, run \
         `reviewhub init` with a newer CLI.\n{MARKER_END}\n"
    )
}

/// Adds or replaces the ReviewHub section of CLAUDE.md, leaving the rest alone.
fn update_claude_md(existing: &str) -> String {
    let block = claude_md_block();
    if let (Some(start), Some(end)) = (existing.find(MARKER_START), existing.find(MARKER_END))
        && start < end
    {
        let after = &existing[end + MARKER_END.len()..];
        return format!("{}{}{}", &existing[..start], block.trim_end_matches('\n'), after);
    }
    if existing.trim().is_empty() {
        return block;
    }
    format!("{}\n\n{block}", existing.trim_end())
}

fn hook(command: &str, timeout: u32) -> Value {
    json!({ "type": "command", "command": command, "timeout": timeout })
}

/// Adds ReviewHub's hooks to `.claude/settings.json`, replacing earlier ones and keeping everything else.
fn update_settings(existing: &str) -> Result<String> {
    let mut settings: Value = if existing.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(existing).context(".claude/settings.json is not valid JSON")?
    };
    let Some(root) = settings.as_object_mut() else { bail!(".claude/settings.json must contain a JSON object") };
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    let Some(hooks) = hooks.as_object_mut() else { bail!("`hooks` in .claude/settings.json must be an object") };

    let ours = [
        ("SessionStart", json!({ "hooks": [hook("\"$CLAUDE_PROJECT_DIR\"/.claude/reviewhub/session-start.sh", 120)] })),
        (
            "PostToolUse",
            json!({
                "matcher": "Bash|mcp__github__create_pull_request",
                "hooks": [hook("\"$CLAUDE_PROJECT_DIR\"/.claude/reviewhub/pr-created.sh", 10)]
            }),
        ),
    ];
    for (event, entry) in ours {
        let list = hooks.entry(event).or_insert_with(|| json!([]));
        let Some(list) = list.as_array_mut() else {
            bail!("`hooks.{event}` in .claude/settings.json must be an array")
        };
        list.retain(|group| {
            !group["hooks"]
                .as_array()
                .is_some_and(|hs| hs.iter().any(|h| h["command"].as_str().is_some_and(|c| c.contains(HOOK_DIR))))
        });
        list.push(entry);
    }
    Ok(format!("{}\n", serde_json::to_string_pretty(&settings)?))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Created,
    Updated,
    Unchanged,
}

pub struct Outcome {
    pub changes: Vec<(String, Change)>,
}

impl Outcome {
    pub fn changed(&self) -> bool {
        self.changes.iter().any(|(_, c)| *c != Change::Unchanged)
    }
}

/// Installs (or with `check`, only compares) ReviewHub's files in `root`.
pub fn run(root: &Path, with_workflow: bool, check: bool) -> Result<Outcome> {
    if !root.is_dir() {
        bail!("{} is not a directory", root.display());
    }
    let mut planned: Vec<(PathBuf, String, bool)> =
        generated(with_workflow).into_iter().map(|g| (root.join(g.path), g.content, g.executable)).collect();

    let read = |p: &Path| fs::read_to_string(p).unwrap_or_default();
    let settings_path = root.join(".claude/settings.json");
    planned.push((settings_path.clone(), update_settings(&read(&settings_path))?, false));
    let claude_md = root.join("CLAUDE.md");
    planned.push((claude_md.clone(), update_claude_md(&read(&claude_md)), false));

    let mut changes = Vec::new();
    for (path, content, executable) in planned {
        let relative = path.strip_prefix(root).unwrap_or(&path).display().to_string();
        let change = match fs::read_to_string(&path) {
            Ok(current) if current == content => Change::Unchanged,
            Ok(_) => Change::Updated,
            Err(_) => Change::Created,
        };
        if !check && change != Change::Unchanged {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
            }
            fs::write(&path, &content).with_context(|| format!("writing {}", path.display()))?;
            #[cfg(unix)]
            if executable {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
            }
        }
        #[cfg(not(unix))]
        let _ = executable;
        changes.push((relative, change));
    }
    Ok(Outcome { changes })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("reviewhub-init-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn installs_and_is_idempotent() {
        let dir = tempdir("idempotent");
        let first = run(&dir, true, false).unwrap();
        assert!(first.changes.iter().all(|(_, c)| *c == Change::Created));
        assert!(!run(&dir, true, true).unwrap().changed(), "a second run changes nothing");
        assert_eq!(fs::read_to_string(dir.join(".claude/reviewhub/bin/VERSION")).unwrap().trim(), VERSION);
        let workflow = fs::read_to_string(dir.join(".github/workflows/reviewhub.yml")).unwrap();
        assert!(workflow.contains(&format!("REVIEWHUB_VERSION: \"{VERSION}\"")) && !workflow.contains("{{VERSION}}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.join(".claude/reviewhub/bin/reviewhub")).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111);
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn renames_every_reference_to_plugin_names() {
        for g in generated(true) {
            for stale in [
                "reviewhub:review",
                "reviewhub:security",
                "`review-manifest`",
                "`security-review`",
                "`security-reviewer`",
                "\nname: review\n",
            ] {
                assert!(!g.content.contains(stale), "{} still mentions {stale:?}", g.path);
            }
        }
        let agent = generated(false).into_iter().find(|g| g.path.ends_with("reviewhub-security-reviewer.md")).unwrap();
        assert!(agent.content.contains("\nname: reviewhub-security-reviewer\n"));
    }

    #[test]
    fn merges_settings_and_replaces_its_own_hooks() {
        let existing = r#"{ "model": "opus", "hooks": { "SessionStart": [
            { "hooks": [{ "type": "command", "command": "npm ci" }] },
            { "hooks": [{ "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/reviewhub/old.sh" }] }
        ] } }"#;
        let merged: Value = serde_json::from_str(&update_settings(existing).unwrap()).unwrap();
        assert_eq!(merged["model"], "opus");
        let start = merged["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(start.len(), 2, "keeps the project's hook, replaces the old ReviewHub one");
        assert_eq!(start[0]["hooks"][0]["command"], "npm ci");
        assert!(start[1]["hooks"][0]["command"].as_str().unwrap().ends_with("session-start.sh"));
        assert_eq!(merged["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);
        assert!(update_settings("[1]").is_err());
    }

    #[test]
    fn keeps_the_rest_of_claude_md() {
        let first = update_claude_md("# Project\n\nOur rules.\n");
        assert!(first.starts_with("# Project\n\nOur rules.\n\n<!-- reviewhub:start -->"));
        let again = update_claude_md(&format!("{first}\n## Later section\n"));
        assert_eq!(again.matches(MARKER_START).count(), 1);
        assert!(again.ends_with("## Later section\n"));
        assert_eq!(update_claude_md(""), claude_md_block());
    }
}
