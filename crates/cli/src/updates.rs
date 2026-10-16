//! `reviewhub update-check`: whether a newer ReviewHub release exists, checked
//! at most once a day, and what to do about it for this kind of install. Used
//! by the Claude Code plugin's and `reviewhub init`'s session-start hooks.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use reviewhub_core::update::{self, Install};

use crate::github::GitHub;

const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// Off when the user turned off ReviewHub's check, or Claude Code's updates or non-essential traffic.
fn disabled(var: impl Fn(&str) -> Option<String>) -> bool {
    ["REVIEWHUB_NO_UPDATE_CHECK", "DISABLE_UPDATES", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"]
        .iter()
        .any(|name| var(name).is_some_and(|v| !v.is_empty() && v != "0"))
}

fn cache_file(var: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let dir = var("REVIEWHUB_CACHE_DIR")
        .or_else(|| var("CLAUDE_PLUGIN_DATA"))
        .map(PathBuf::from)
        .or_else(|| var("XDG_CACHE_HOME").map(|d| PathBuf::from(d).join("reviewhub")))
        .or_else(|| var("HOME").map(|h| PathBuf::from(h).join(".cache/reviewhub")))?;
    Some(dir.join("latest-release.json"))
}

/// The latest release tag: from the cache when it is under a day old, else from `fetch` (stored in the cache when
/// it answers). Offline, an older cached answer is better than none.
fn latest_release(cache: Option<&Path>, now: u64, fetch: impl FnOnce() -> Option<String>) -> Option<String> {
    let cached: Option<Value> =
        cache.and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok());
    let cached_tag = cached.as_ref().and_then(|c| c["tag"].as_str().map(String::from));
    let checked = cached.as_ref().and_then(|c| c["checked"].as_u64());
    if let (Some(tag), Some(checked)) = (&cached_tag, checked)
        && now.saturating_sub(checked) < CHECK_EVERY.as_secs()
    {
        return Some(tag.clone());
    }
    let Some(tag) = fetch() else { return cached_tag };
    if let Some(file) = cache {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file, json!({ "tag": tag, "checked": now }).to_string());
    }
    Some(tag)
}

/// What to print for a notice: the message itself, or with `hook`, Claude Code SessionStart hook output (shown to the
/// user, and given to Claude so it can mention it).
fn output(message: &str, hook: bool) -> String {
    if !hook {
        return message.to_string();
    }
    let context = format!("{message} Tell the user this in one short line at the start of your first reply.");
    json!({
        "systemMessage": message,
        "hookSpecificOutput": { "hookEventName": "SessionStart", "additionalContext": context }
    })
    .to_string()
}

/// Prints the update notice, if any. Never fails: an update check mustn't get in the way.
pub fn check(install: Install, hook: bool) {
    let var = |name: &str| std::env::var(name).ok();
    if disabled(var) {
        return;
    }
    // Launchers run the CLI of a pinned version; while a new release's binaries are still building they fall back
    // to an older CLI, so the pin (not this binary's version) is what's installed.
    let current = var("REVIEWHUB_PINNED_VERSION").unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // The releases endpoint is public: don't read or send the user's GitHub token for a background check.
    let fetch = || GitHub::with_timeout(None, Some(Duration::from_secs(3))).latest_reviewhub_release().ok();
    let Some(latest) = latest_release(cache_file(var).as_deref(), now, fetch) else { return };
    if let Some(message) = update::notice(install, &current, &latest) {
        println!("{}", output(&message, hook));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::HashMap;

    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |name| map.get(name).cloned()
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("reviewhub-updates-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn opts_out_on_any_of_the_switches() {
        assert!(!disabled(vars(&[])));
        assert!(!disabled(vars(&[("DISABLE_UPDATES", "0"), ("REVIEWHUB_NO_UPDATE_CHECK", "")])));
        for name in ["REVIEWHUB_NO_UPDATE_CHECK", "DISABLE_UPDATES", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"] {
            assert!(disabled(vars(&[(name, "1")])), "{name}");
        }
    }

    #[test]
    fn caches_in_the_first_directory_set() {
        let file = |pairs: &[(&str, &str)]| cache_file(vars(pairs)).map(|p| p.display().to_string());
        assert_eq!(file(&[]), None);
        assert_eq!(file(&[("HOME", "/h")]).as_deref(), Some("/h/.cache/reviewhub/latest-release.json"));
        assert_eq!(
            file(&[("HOME", "/h"), ("XDG_CACHE_HOME", "/x")]).as_deref(),
            Some("/x/reviewhub/latest-release.json")
        );
        assert_eq!(
            file(&[("XDG_CACHE_HOME", "/x"), ("CLAUDE_PLUGIN_DATA", "/p")]).as_deref(),
            Some("/p/latest-release.json")
        );
        assert_eq!(
            file(&[("CLAUDE_PLUGIN_DATA", "/p"), ("REVIEWHUB_CACHE_DIR", "/c")]).as_deref(),
            Some("/c/latest-release.json")
        );
    }

    #[test]
    fn asks_github_at_most_once_a_day() {
        let file = temp_dir("daily").join("nested/latest-release.json");
        let calls = Cell::new(0);
        let fetch = |tag: &str| {
            let tag = tag.to_string();
            let calls = &calls;
            move || {
                calls.set(calls.get() + 1);
                Some(tag)
            }
        };
        // Nothing cached: asks, and creates the cache's directory.
        assert_eq!(latest_release(Some(&file), 1_000, fetch("v0.6.0")).as_deref(), Some("v0.6.0"));
        assert_eq!(calls.get(), 1);
        // Within the day: the cached answer, without asking.
        assert_eq!(latest_release(Some(&file), 1_000 + 3_600, fetch("v0.7.0")).as_deref(), Some("v0.6.0"));
        assert_eq!(calls.get(), 1);
        // A day later: asks again and stores the new answer.
        let later = 1_000 + CHECK_EVERY.as_secs();
        assert_eq!(latest_release(Some(&file), later, fetch("v0.7.0")).as_deref(), Some("v0.7.0"));
        assert_eq!(calls.get(), 2);
        let stored: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(stored, json!({ "tag": "v0.7.0", "checked": later }));
        let _ = std::fs::remove_dir_all(temp_dir("daily"));
    }

    #[test]
    fn falls_back_to_a_stale_answer_offline() {
        let dir = temp_dir("offline");
        let file = dir.join("latest-release.json");
        assert_eq!(latest_release(Some(&file), 0, || None), None);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&file, r#"{"tag":"v0.5.0","checked":0}"#).unwrap();
        assert_eq!(latest_release(Some(&file), CHECK_EVERY.as_secs() * 5, || None).as_deref(), Some("v0.5.0"));
        // A broken cache counts as none.
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(latest_release(Some(&file), 0, || Some("v0.6.0".into())).as_deref(), Some("v0.6.0"));
        // No cache location at all: still answers.
        assert_eq!(latest_release(None, 0, || Some("v0.6.0".into())).as_deref(), Some("v0.6.0"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prints_plain_text_or_hook_output() {
        assert_eq!(output("ReviewHub 0.6.0 is available.", false), "ReviewHub 0.6.0 is available.");
        let hook: Value = serde_json::from_str(&output("ReviewHub 0.6.0 is available.", true)).unwrap();
        assert_eq!(hook["systemMessage"], "ReviewHub 0.6.0 is available.");
        assert_eq!(hook["hookSpecificOutput"]["hookEventName"], "SessionStart");
        let context = hook["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
        assert!(context.starts_with("ReviewHub 0.6.0 is available. Tell the user"), "{context}");
    }
}
