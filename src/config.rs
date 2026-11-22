use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const BLOG_CONFIG: &str = "blog.toml";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlogConfig {
	pub title: String,
	#[serde(default)]
	pub description: String,
	#[serde(default)]
	pub author: String,
	#[serde(default = "default_language")]
	pub language: String,
	#[serde(default)]
	pub build: BuildConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct BuildConfig {
	pub output: PathBuf,
	pub include_drafts: bool,
}

impl Default for BuildConfig {
	fn default() -> Self {
		Self {
			output: PathBuf::from("build"),
			include_drafts: false,
		}
	}
}

fn default_language() -> String {
	String::from("en")
}

impl BlogConfig {
	pub fn load(root: &Path) -> Result<Self> {
		let path = root.join(BLOG_CONFIG);
		let text = std::fs::read_to_string(&path)
			.with_context(|| format!("config: failed to read {}", path.display()))?;
		let config: Self = toml::from_str(&text)
			.with_context(|| format!("config: failed to parse {}", path.display()))?;

		if config.title.trim().is_empty() {
			bail!("config: blog title cannot be empty in {}", path.display());
		}

		if config.language.trim().is_empty() {
			bail!(
				"config: blog language cannot be empty in {}",
				path.display()
			);
		}

		Ok(config)
	}

	pub fn find_root(start: &Path) -> Result<PathBuf> {
		let start = start
			.canonicalize()
			.with_context(|| format!("find_root: failed to resolve {}", start.display()))?;

		for dir in start.ancestors() {
			if dir.join(BLOG_CONFIG).is_file() {
				return Ok(dir.to_path_buf());
			}
		}

		bail!(
			"find_root: No blog.toml found in {} or any parent directory; run `bd init <LOCATION>` first!",
			start.display()
		)
	}
}
