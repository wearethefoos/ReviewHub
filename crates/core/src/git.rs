//! Git plumbing the pre-push hook needs: what is being pushed, and to which GitHub repository.

use std::sync::LazyLock;

use regex::Regex;

/// A branch a `git push` updates, as git hands it to the pre-push hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushedBranch {
    pub branch: String,
    pub sha: String,
}

/// Parses the pre-push hook's input (`<local ref> <local sha> <remote ref> <remote sha>` per line),
/// keeping branch updates and dropping deletions and tags.
pub fn pushed_branches(input: &str) -> Vec<PushedBranch> {
    input
        .lines()
        .filter_map(|line| {
            let [_, local_sha, remote_ref, _] = line.split_whitespace().collect::<Vec<_>>()[..] else { return None };
            let branch = remote_ref.strip_prefix("refs/heads/")?;
            let deleted = local_sha.chars().all(|c| c == '0');
            (!deleted).then(|| PushedBranch { branch: branch.to_string(), sha: local_sha.to_string() })
        })
        .collect()
}

static GITHUB_REMOTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:https?://(?:[^@/]+@)?github\.com/|git@github\.com:|ssh://git@github\.com/)([\w.-]+)/([\w.-]+?)(?:\.git)?/?$")
        .unwrap()
});

/// `owner/repo` of a github.com remote URL (HTTPS or SSH), or `None` for other hosts.
pub fn github_repo(url: &str) -> Option<String> {
    let caps = GITHUB_REMOTE.captures(url.trim())?;
    Some(format!("{}/{}", &caps[1], &caps[2]))
}

/// A file name for per-branch state: branch names can contain slashes.
pub fn branch_file_name(branch: &str) -> String {
    let safe: String =
        branch.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '_' }).collect();
    format!("{safe}.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_branch_updates_only() {
        let zeros = "0".repeat(40);
        let input = format!(
            "refs/heads/fix abc123 refs/heads/claude/fix {zeros}\n\
             refs/tags/v1 def456 refs/tags/v1 {zeros}\n\
             (delete) {zeros} refs/heads/old fed987\n"
        );
        assert_eq!(pushed_branches(&input), vec![PushedBranch { branch: "claude/fix".into(), sha: "abc123".into() }]);
        assert!(pushed_branches("").is_empty());
    }

    #[test]
    fn recognises_github_remotes() {
        for url in [
            "https://github.com/wearethefoos/ReviewHub.git",
            "https://token@github.com/wearethefoos/ReviewHub",
            "git@github.com:wearethefoos/ReviewHub.git",
            "ssh://git@github.com/wearethefoos/ReviewHub.git",
        ] {
            assert_eq!(github_repo(url).as_deref(), Some("wearethefoos/ReviewHub"), "{url}");
        }
        assert_eq!(github_repo("https://gitlab.com/a/b.git"), None);
        assert_eq!(github_repo("http://127.0.0.1:8080/git/a/b"), None);
    }

    #[test]
    fn makes_safe_file_names() {
        assert_eq!(branch_file_name("claude/fix-it"), "claude_fix-it.json");
    }
}
