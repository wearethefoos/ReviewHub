//! I/O for `reviewhub metrics`: lcov paths and the changed files' sources from git.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use reviewhub_core::complexity::{self, ChangedSource};
use reviewhub_core::model::Quality;

/// Rewrites relative `SF:` paths so they are relative to `root`. Coverage tools
/// run in a project subdirectory write paths relative to it; such a path is
/// resolved against the directories between `root` and the lcov file, then
/// against the root's direct subdirectories.
pub fn resolve_lcov_paths(text: &str, root: &Path, lcov_file: &Path) -> String {
    let lcov_dir = lcov_file.parent().map(|d| if d.is_absolute() { d.to_path_buf() } else { root.join(d) });
    // Directories between the lcov file and the root, then the root's subdirectories (for lcov files written
    // elsewhere, such as a CI temp directory).
    let mut candidates: Vec<PathBuf> = lcov_dir
        .as_deref()
        .map(|d| d.ancestors().take_while(|a| a.starts_with(root)).map(Path::to_path_buf).collect())
        .unwrap_or_default();
    let mut subdirs: Vec<PathBuf> = std::fs::read_dir(root)
        .map(|entries| entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    subdirs.sort();
    candidates.extend(subdirs);
    text.lines()
        .map(|line| {
            let Some(path) = line.strip_prefix("SF:") else { return line.to_string() };
            let path = path.trim().trim_start_matches("./");
            if Path::new(path).is_absolute() || root.join(path).exists() {
                return line.to_string();
            }
            match candidates.iter().find(|dir| dir.join(path).exists()) {
                Some(dir) => format!("SF:{}", dir.join(path).strip_prefix(root).unwrap_or(&dir.join(path)).display()),
                None => line.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn git(root: &Path, args: &[&str]) -> Result<Option<String>> {
    let out = Command::new("git").current_dir(root).args(args).output().context("running git")?;
    Ok(out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// Complexity of the files changed between `base` (its merge base with HEAD) and HEAD.
pub fn changed_quality(root: &Path, base: &str) -> Result<Quality> {
    let Some(merge_base) = git(root, &["merge-base", base, "HEAD"])? else {
        bail!("can't find a merge base of {base} and HEAD (in CI, check out with fetch-depth: 0)");
    };
    let merge_base = merge_base.trim();
    let names = git(root, &["diff", "--name-only", "--no-renames", merge_base, "HEAD"])?.unwrap_or_default();
    let sources: Vec<(String, Option<String>, Option<String>)> = names
        .lines()
        .filter(|p| !p.is_empty())
        .map(|p| {
            let base = git(root, &["show", &format!("{merge_base}:{p}")]).ok().flatten();
            let head = git(root, &["show", &format!("HEAD:{p}")]).ok().flatten();
            Ok((p.to_string(), base, head))
        })
        .collect::<Result<_>>()?;
    let changed: Vec<ChangedSource> = sources
        .iter()
        .map(|(path, base, head)| ChangedSource { path, base: base.as_deref(), head: head.as_deref() })
        .collect();
    Ok(complexity::quality(&changed))
}
