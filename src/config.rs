use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const BLOG_CONFIG: &str = "blog.toml";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum CssFramework {
	#[default]
	Manual,
	Tailwind,
}

impl CssFramework {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Manual => "manual",
			Self::Tailwind => "tailwind",
		}
	}
}

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
	pub favicon: String,
	#[serde(default)]
	pub css: CssFramework,
	#[serde(default)]
	pub footer: Vec<FooterLink>,
	#[serde(default)]
	pub build: BuildConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct BuildConfig {
	pub output: PathBuf,
	pub include_drafts: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FooterLink {
	pub title: String,
	pub url: String,
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

		for link in &config.footer {
			if link.title.trim().is_empty() {
				bail!(
					"config: footer link title cannot be empty in {}",
					path.display()
				);
			}

			if link.url.trim().is_empty() {
				bail!(
					"config: footer link URL cannot be empty in {}",
					path.display()
				);
			}
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

	pub fn uses_tailwind(&self) -> bool {
		self.css == CssFramework::Tailwind
	}
}

#[cfg(test)]
mod tests {
	use std::path::{Path, PathBuf};

	use super::*;

	fn write_config(dir: &Path, contents: &str) {
		std::fs::write(dir.join(BLOG_CONFIG), contents).unwrap();
	}

	#[test]
	fn load_applies_defaults() {
		let temp = tempfile::tempdir().unwrap();
		write_config(temp.path(), "title = \"Blog\"\n");

		let config = BlogConfig::load(temp.path()).unwrap();
		assert_eq!(config.title, "Blog");
		assert_eq!(config.description, "");
		assert_eq!(config.author, "");
		assert_eq!(config.language, "en");
		assert_eq!(config.favicon, "");
		assert_eq!(config.css, CssFramework::Manual);
		assert!(config.footer.is_empty());
		assert_eq!(config.build.output, PathBuf::from("build"));
		assert!(!config.build.include_drafts);
	}

	#[test]
	fn load_reads_build_settings() {
		let temp = tempfile::tempdir().unwrap();
		write_config(
			temp.path(),
			"title = \"Blog\"\n\n[build]\noutput = \"site\"\ninclude_drafts = true\n",
		);

		let config = BlogConfig::load(temp.path()).unwrap();
		assert_eq!(config.build.output, PathBuf::from("site"));
		assert!(config.build.include_drafts);
	}

	#[test]
	fn load_rejects_unknown_fields() {
		let temp = tempfile::tempdir().unwrap();
		write_config(temp.path(), "title = \"Blog\"\nunknown = true\n");
		assert!(BlogConfig::load(temp.path()).is_err());
	}

	#[test]
	fn load_rejects_empty_title_and_language() {
		let temp = tempfile::tempdir().unwrap();

		write_config(temp.path(), "title = \"   \"\n");
		assert!(BlogConfig::load(temp.path()).is_err());

		write_config(temp.path(), "title = \"Blog\"\nlanguage = \"\"\n");
		assert!(BlogConfig::load(temp.path()).is_err());
	}

	#[test]
	fn find_root_walks_up_from_nested_directory() {
		let temp = tempfile::tempdir().unwrap();
		write_config(temp.path(), "title = \"Blog\"\n");
		let nested = temp.path().join("a/b");
		std::fs::create_dir_all(&nested).unwrap();

		let root = BlogConfig::find_root(&nested).unwrap();
		assert_eq!(root, temp.path().canonicalize().unwrap());
	}

	#[test]
	fn find_root_errors_without_blog_toml() {
		let temp = tempfile::tempdir().unwrap();
		assert!(BlogConfig::find_root(temp.path()).is_err());
	}

	#[test]
	fn load_reads_footer_links() {
		let temp = tempfile::tempdir().unwrap();
		write_config(
			temp.path(),
			"title = \"Blog\"\n\n[[footer]]\ntitle = \"GitHub\"\nurl = \"https://github.com/example\"\n\n[[footer]]\ntitle = \"RSS\"\nurl = \"/feed.xml\"\n",
		);

		let config = BlogConfig::load(temp.path()).unwrap();
		assert_eq!(config.footer.len(), 2);
		assert_eq!(config.footer[0].title, "GitHub");
		assert_eq!(config.footer[0].url, "https://github.com/example");
		assert_eq!(config.footer[1].title, "RSS");
		assert_eq!(config.footer[1].url, "/feed.xml");
	}

	#[test]
	fn load_rejects_empty_footer_link_fields() {
		let temp = tempfile::tempdir().unwrap();

		write_config(
			temp.path(),
			"title = \"Blog\"\n\n[[footer]]\ntitle = \" \"\nurl = \"https://example.com\"\n",
		);
		assert!(BlogConfig::load(temp.path()).is_err());

		write_config(
			temp.path(),
			"title = \"Blog\"\n\n[[footer]]\ntitle = \"GitHub\"\nurl = \" \"\n",
		);
		assert!(BlogConfig::load(temp.path()).is_err());
	}

	#[test]
	fn load_reads_css_framework() {
		let temp = tempfile::tempdir().unwrap();
		write_config(temp.path(), "title = \"Blog\"\ncss = \"tailwind\"\n");

		let config = BlogConfig::load(temp.path()).unwrap();
		assert_eq!(config.css, CssFramework::Tailwind);
		assert!(config.uses_tailwind());
	}
}
