use std::process::Command;

use anyhow::{Context, Result, bail};
use semver::Version;

const BD_REPO: &str = "brandonfglx/bd";
const SKIP_ENV: &str = "BD_SKIP_UPDATE_CHECK";

pub fn ensure_current() -> Result<()> {
	if std::env::var_os(SKIP_ENV).is_some() {
		return Ok(());
	}

	let latest = match latest_release() {
		Ok(latest) => latest,
		Err(error) => {
			eprintln!("bd: warning: could not check for updates: {error:#}");
			return Ok(());
		}
	};

	let current = current_version();
	if latest > current {
		bail!(
			"A newer version of bd is available: v{latest} (current v{current}).\nRun `bd publish --upgrade` to upgrade this repository or `bd publish --force` to publish with the current version.\n\nIt is recommended to upgrade to the latest version."
		);
	}

	Ok(())
}

pub fn current_version() -> Version {
	Version::parse(env!("CARGO_PKG_VERSION")).expect("CARGO_PKG_VERSION should be valid semver")
}

fn latest_release() -> Result<Version> {
	let url = format!("https://github.com/{BD_REPO}.git");
	let output = Command::new("git")
		.args(["ls-remote", "--tags", "--refs", url.as_str(), "v*"])
		.env("GIT_TERMINAL_PROMPT", "0")
		.output()
		.context("failed to run `git ls-remote`")?;

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		bail!("`git ls-remote` failed: {}", stderr.trim());
	}

	let stdout = String::from_utf8_lossy(&output.stdout);
	latest_tag(&stdout).context("no valid release tags found")
}

fn latest_tag(output: &str) -> Option<Version> {
	output
		.lines()
		.filter_map(|line| line.split_whitespace().nth(1))
		.filter_map(|reference| reference.strip_prefix("refs/tags/v"))
		.filter_map(|tag| Version::parse(tag).ok())
		.filter(|version| version.pre.is_empty())
		.max()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn latest_tag_picks_highest_stable_release() {
		let output = "\
abc\trefs/tags/v0.1.0
def\trefs/tags/v0.1.10
ghi\trefs/tags/v0.2.0-rc.1
jkl\trefs/tags/v0.1.9
";
		assert_eq!(latest_tag(output), Some(Version::new(0, 1, 10)));
	}

	#[test]
	fn latest_tag_ignores_invalid_tags() {
		assert_eq!(latest_tag("abc\trefs/tags/vnot-a-version\n"), None);
	}

	#[test]
	fn current_version_is_valid() {
		let _ = current_version();
	}
}
