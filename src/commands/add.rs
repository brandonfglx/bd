use std::fs;

use anyhow::{Context, Result, bail};

use crate::config::BlogConfig;
use crate::page::{PAGE_META, PageFile, format_date, format_offset, local_now, parse_date};
use crate::slug;

const PAGE_MD: &str = include_str!("../base/page.md");
const FALLBACK_PAGE_MD: &str =
	"Write your post here. You can use **Markdown**, lists, links, images, and code blocks.\n";

pub(crate) fn default_page_markdown() -> &'static str {
	if PAGE_MD.trim().is_empty() {
		FALLBACK_PAGE_MD
	} else {
		PAGE_MD
	}
}

pub fn run(title: &str, slug_arg: Option<&str>, date_arg: Option<&str>, draft: bool) -> Result<()> {
	let cwd = std::env::current_dir().context("failed to determine current directory")?;
	let root = BlogConfig::find_root(&cwd)?;

	let title = title.trim();
	if title.is_empty() {
		bail!("title cannot be empty");
	}

	let slug = match slug_arg {
		Some(slug) => slug.to_string(),
		None => slug::slugify(title),
	};

	slug::validate(&slug)?;

	let dir = root.join(&slug);
	if dir.exists() {
		bail!("page directory already exists: {}", dir.display());
	}

	let (date, local_offset) = match date_arg {
		Some(value) => (
			parse_date(value).with_context(|| format!("invalid --date value {value:?}"))?,
			None,
		),
		None => {
			let (date, offset) = local_now();
			(date, Some(offset))
		}
	};

	let date_str = format_date(date)?;
	let page = PageFile {
		title: title.to_string(),
		description: String::new(),
		date: date_str.clone(),
		draft,
		content: "page.md".to_string(),
	};
	let meta = toml::to_string_pretty(&page).context("failed to serialize page.toml")?;

	fs::create_dir(&dir).with_context(|| format!("failed to create {}", dir.display()))?;

	let result = (|| -> Result<()> {
		fs::write(dir.join(PAGE_META), meta)
			.with_context(|| format!("failed to write {}", dir.join(PAGE_META).display()))?;
		fs::write(dir.join("page.md"), default_page_markdown())
			.with_context(|| format!("failed to write {}", dir.join("page.md").display()))?;
		Ok(())
	})();

	if let Err(error) = result {
		let _ = fs::remove_dir_all(&dir);
		return Err(error);
	}

	let date_note = match local_offset {
		Some(offset) => format!("(date {date_str}, {})", format_offset(offset)),
		None => format!("(date {date_str})"),
	};
	println!("Created page \"{slug}\" at {} {date_note}", dir.display());
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn embedded_page_markdown_is_not_empty() {
		assert!(
			!PAGE_MD.trim().is_empty(),
			"src/base/page.md must contain the starter Markdown"
		);
	}

	#[test]
	fn default_page_markdown_falls_back_when_embedded_file_is_empty() {
		let markdown = default_page_markdown();
		assert!(!markdown.trim().is_empty());
		assert!(markdown.ends_with('\n'), "{markdown:?}");
	}
}
