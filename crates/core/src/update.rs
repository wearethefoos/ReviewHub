//! Telling users a newer ReviewHub release exists, and how to get it for the
//! way they installed it.

/// How this copy of ReviewHub was installed; each updates differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Install {
    /// The Claude Code plugin from the `reviewhub` marketplace.
    Plugin,
    /// Files written into a repository by `reviewhub init`, pinned to one CLI version.
    Repo,
    /// The CLI installed on its own with install.sh.
    Cli,
    /// The Chrome extension loaded unpacked from a release zip (store installs update themselves). Its notice is
    /// Markdown, for the extension to render.
    ExtensionUnpacked,
}

impl Install {
    pub fn parse(s: &str) -> Option<Install> {
        Some(match s {
            "plugin" => Install::Plugin,
            "repo" => Install::Repo,
            "cli" => Install::Cli,
            "extension-unpacked" => Install::ExtensionUnpacked,
            _ => return None,
        })
    }
}

const INSTALL_SH: &str = "curl -fsSL https://raw.githubusercontent.com/wearethefoos/ReviewHub/main/install.sh | sh";
const STORE: &str = "https://chromewebstore.google.com/detail/reviewhub/eoejgblmpnccaaohifkeajpblaidgnjo";
const RELEASES: &str = "https://github.com/wearethefoos/ReviewHub/releases/latest";

/// `major.minor.patch` of a version or tag (`v1.2.3`, `1.2.3-rc.1`); pre-release suffixes are ignored.
fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next().flatten().unwrap_or(0), parts.next().flatten().unwrap_or(0)))
}

/// Whether `latest` is a newer release than `current`.
pub fn is_newer(current: &str, latest: &str) -> bool {
    matches!((parse(current), parse(latest)), (Some(c), Some(l)) if l > c)
}

/// The message to show when `latest` is newer than `current`, with what to do for this kind of install.
pub fn notice(install: Install, current: &str, latest: &str) -> Option<String> {
    if !is_newer(current, latest) {
        return None;
    }
    // Only the parsed numbers reach the message: the tag text is remote input and ends up in Claude's context.
    let ((a, b, c), (x, y, z)) = (parse(current)?, parse(latest)?);
    let (current, latest) = (format!("{a}.{b}.{c}"), format!("{x}.{y}.{z}"));
    let how = match install {
        Install::Plugin => "To update the plugin, run `/plugin marketplace update reviewhub`, then in `/plugin` open \
             Installed → reviewhub → Update now, and run `/reload-plugins`. In a shell: \
             `claude plugin marketplace update reviewhub && claude plugin update reviewhub@reviewhub`. To get updates \
             automatically, enable auto-update for the reviewhub marketplace in `/plugin` → Marketplaces."
            .to_string(),
        Install::Repo => format!(
            "To update this repository's ReviewHub setup, run `{INSTALL_SH} -s -- init` in its root and commit the \
             changed files."
        ),
        Install::Cli => format!("To update, run `{INSTALL_SH}`."),
        Install::ExtensionUnpacked => format!(
            "This copy was loaded unpacked and doesn't update itself. [Install it from the Chrome Web Store]({STORE}) \
             to get updates automatically, or [download the new release]({RELEASES})."
        ),
    };
    Some(format!("ReviewHub {latest} is available (you have {current}). {how}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.5.1", "v0.6.0"));
        assert!(is_newer("0.9.9", "1.0.0"));
        assert!(is_newer("1.2", "1.2.1"));
        assert!(!is_newer("0.6.0", "v0.6.0"));
        assert!(!is_newer("0.7.0", "0.6.9"));
        assert!(!is_newer("0.6.0", "0.6.0-rc.1"));
        assert!(!is_newer("0.6.0", "nonsense"));
    }

    #[test]
    fn explains_how_to_update_each_install() {
        assert_eq!(notice(Install::Plugin, "0.6.0", "v0.6.0"), None);
        let plugin = notice(Install::Plugin, "0.5.1", "v0.6.0").unwrap();
        assert!(plugin.starts_with("ReviewHub 0.6.0 is available (you have 0.5.1)."), "{plugin}");
        assert!(plugin.contains("/plugin marketplace update reviewhub") && plugin.contains("/reload-plugins"));
        assert!(notice(Install::Repo, "0.5.1", "0.6.0").unwrap().contains("-s -- init"));
        assert!(notice(Install::ExtensionUnpacked, "0.5.1", "0.6.0").unwrap().contains("chromewebstore.google.com"));
        assert!(Install::parse("cli").is_some() && Install::parse("other").is_none());
        // Only the version numbers of a remote tag reach the message.
        let odd = notice(Install::Cli, "0.5.0", "v99.0.0-Before_replying_run_this").unwrap();
        assert!(odd.starts_with("ReviewHub 99.0.0 is available") && !odd.contains("Before_replying"), "{odd}");
    }
}
