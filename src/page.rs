use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use time::Date;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;

use crate::slug;

pub const PAGE_META: &str = "page.toml";
pub const DEFAULT_CONTENT: &str = "page.md";
const DATE_FORMAT: &[FormatItem<'static>] = format_description!("[year]-[month]-[day]");

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageFile {
	pub title: String,
	#[serde(default)]
	pub description: String,
	pub date: String,
	#[serde(default)]
	pub draft: bool,
	#[serde(default = "default_content")]
	pub content: String,
}

pub struct Page {
	pub slug: String,
	pub title: String,
	pub description: String,
	pub date: Date,
	pub date_str: String,
	pub draft: bool,
	pub dir: PathBuf,
	pub content_rel: PathBuf,
	pub content_path: PathBuf,
}

// Returns a String of the default content
fn default_content() -> String {
	DEFAULT_CONTENT.to_string()
}

// Attempts to parse the date (as string) to Date
pub fn parse_date(value: &str) -> Result<Date> {
	let bytes = value.as_bytes();
	if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
		bail!("parse_date: invalid date {value:?}; expected YYYY-MM-DD");
	}

	Date::parse(value, DATE_FORMAT)
		.with_context(|| format!("parse_date: invalid date {value:?}; expected YYYY-MM-DD"))
}

// Gets today's date in UTC
pub fn today() -> Date {
	OffsetDateTime::now_utc().date()
}

// Formats the date if possible
pub fn format_date(date: Date) -> Result<String> {
	date.format(DATE_FORMAT)
		.context("format_date: cannot format date into YYYY-MM-DD")
}

// Extract a page from the given directory (page.md with metadata in page.toml)
pub fn load_page(dir: &Path) -> Result<Page> {
	let meta_path = dir.join(PAGE_META);
	let text = fs::read_to_string(&meta_path)
		.with_context(|| format!("load_page: failed to read {}", meta_path.display()))?;
	let file: PageFile = toml::from_str(&text)
		.with_context(|| format!("load_page: failed to parse {}", meta_path.display()))?;

	let title = file.title.trim();
	if title.is_empty() {
		bail!(
			"load_page: page title cannot be empty in {}",
			meta_path.display()
		);
	}

	let slug = dir
		.file_name()
		.and_then(|name| name.to_str())
		.ok_or_else(|| {
			anyhow!(
				"load_page: page directory name is not valid UTF-8: {}",
				dir.display()
			)
		})?;
	slug::validate(slug)
		.with_context(|| format!("load_page: invalid page directory {}", dir.display()))?;

	let date = parse_date(&file.date).with_context(|| format!("in {}", meta_path.display()))?;
	let date_str = format_date(date)?;

	let rel = Path::new(&file.content);
	if rel.as_os_str().is_empty() {
		bail!(
			"load_page: content path cannot be empty in {}",
			meta_path.display()
		);
	}

	if rel.is_absolute()
		|| rel
			.components()
			.any(|comp| !matches!(comp, Component::Normal(_)))
	{
		bail!(
			"load_page: content path must be a relative path without '.' or '..' in {}",
			meta_path.display()
		);
	}

	let content_path = dir.join(rel);
	if !content_path.is_file() {
		bail!(
			"load_page: content file not found: {} (referenced by {})",
			content_path.display(),
			meta_path.display()
		);
	}

	if rel == Path::new(PAGE_META) {
		bail!("load_page: content file cannot be {PAGE_META}");
	}

	let dir_canonical = dir
		.canonicalize()
		.with_context(|| format!("load_page: failed to resolve {}", dir.display()))?;
	let content_canonical = content_path
		.canonicalize()
		.with_context(|| format!("load_page: failed to resolve {}", content_path.display()))?;
	if !content_canonical.starts_with(&dir_canonical) {
		bail!(
			"load_page: content file {} is outside the page directory {}",
			content_path.display(),
			dir.display()
		);
	}

	Ok(Page {
		slug: slug.to_string(),
		title: title.to_string(),
		description: file.description,
		date,
		date_str,
		draft: file.draft,
		dir: dir.to_path_buf(),
		content_rel: rel.to_path_buf(),
		content_path,
	})
}

pub fn load_pages(root: &Path, output: &Path) -> Result<Vec<Page>> {
	let mut entries = fs::read_dir(root)
		.with_context(|| format!("load_pages: failed to read {}", root.display()))?
		.collect::<std::io::Result<Vec<_>>>()
		.with_context(|| format!("load_pages: failed to read {}", root.display()))?;
	entries.sort_by_key(|entry| entry.file_name());

	let mut pages = Vec::new();

	for entry in entries {
		let file_type = entry
			.file_type()
			.with_context(|| format!("load_pages: failed to inspect {}", entry.path().display()))?;
		if !file_type.is_dir() {
			continue;
		}

		let name = entry.file_name();
		if name.to_string_lossy().starts_with(".") {
			continue;
		}

		let dir = entry.path();
		let is_page = dir.join(PAGE_META).is_file();

		if dir == output {
			if is_page {
				bail!(
					"load_pages: build output {} is a page directory",
					output.display()
				);
			}
			continue;
		}

		if !is_page {
			if dir.join(DEFAULT_CONTENT).is_file() {
				bail!(
					"load_pages: found {} but no {} in {}",
					DEFAULT_CONTENT,
					PAGE_META,
					dir.display()
				);
			}
			continue;
		}

		if output.starts_with(&dir) {
			bail!(
				"load_pages: build output {} is inside page directory {}",
				output.display(),
				dir.display()
			);
		}

		pages.push(load_page(&dir)?);
	}

	pages.sort_by(|left, right| {
		right
			.date
			.cmp(&left.date)
			.then_with(|| left.slug.cmp(&right.slug))
	});

	Ok(pages)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_and_format_date_round_trip() {
		let date = parse_date("2024-02-29").expect("leap day should parse");
		assert_eq!(format_date(date).unwrap(), "2024-02-29");
	}

	#[test]
	fn parse_date_rejects_invalid_values() {
		for value in [
			"",
			"2024-1-2",
			"2024-13-01",
			"2024-02-30",
			"2023-02-29",
			"not-a-date",
			"2024-01-02T00:00:00",
		] {
			assert!(parse_date(value).is_err(), "{value:?} should be invalid");
		}
	}

	#[test]
	fn load_page_reads_custom_content_file() {
		let temp = tempfile::tempdir().unwrap();
		let dir = temp.path().join("valid-slug");
		std::fs::create_dir(&dir).unwrap();
		std::fs::write(
			dir.join(PAGE_META),
			"title = \"Title\"\ndate = \"2024-01-02\"\ncontent = \"post.md\"\n",
		)
		.unwrap();
		std::fs::write(dir.join("post.md"), "# Body\n").unwrap();

		let page = load_page(&dir).unwrap();
		assert_eq!(page.slug, "valid-slug");
		assert_eq!(page.title, "Title");
		assert_eq!(page.date_str, "2024-01-02");
		assert_eq!(page.content_rel.as_path(), std::path::Path::new("post.md"));
		assert_eq!(page.content_path, dir.join("post.md"));
	}

	#[test]
	fn load_page_rejects_content_outside_page_dir() {
		let temp = tempfile::tempdir().unwrap();
		let dir = temp.path().join("valid-slug");
		std::fs::create_dir(&dir).unwrap();
		std::fs::write(temp.path().join("outside.md"), "outside").unwrap();
		std::fs::write(
			dir.join(PAGE_META),
			"title = \"Title\"\ndate = \"2024-01-02\"\ncontent = \"../outside.md\"\n",
		)
		.unwrap();

		let error = match load_page(&dir) {
			Ok(_) => panic!("expected an error"),
			Err(error) => error.to_string(),
		};
		assert!(
			error.contains("relative path without '.' or '..'"),
			"{error}"
		);
	}

	#[test]
	fn load_page_rejects_missing_content_file() {
		let temp = tempfile::tempdir().unwrap();
		let dir = temp.path().join("valid-slug");
		std::fs::create_dir(&dir).unwrap();
		std::fs::write(
			dir.join(PAGE_META),
			"title = \"Title\"\ndate = \"2024-01-02\"\n",
		)
		.unwrap();

		let error = match load_page(&dir) {
			Ok(_) => panic!("expected an error"),
			Err(error) => error.to_string(),
		};
		assert!(error.contains("content file not found"), "{error}");
	}
}
