use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use time::format_description::FormatItem;
use time::macros::format_description;
use time::{Date, OffsetDateTime, UtcOffset};

use crate::slug;
use crate::tags::{self, Tag};

pub const PAGE_META: &str = "page.toml";
pub const DEFAULT_CONTENT: &str = "page.md";
const DATE_FORMAT: &[FormatItem<'static>] = format_description!("[year]-[month]-[day]");
const DATE_DISPLAY_FORMAT: &[FormatItem<'static>] =
	format_description!("[month repr:long] [day padding:none], [year]");

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageFile {
	pub title: String,
	#[serde(default)]
	pub description: String,
	pub date: String,
	#[serde(default)]
	pub draft: bool,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub tags: Vec<String>,
	#[serde(default, skip_serializing_if = "String::is_empty")]
	pub image: String,
	#[serde(default = "default_content")]
	pub content: String,
}

pub struct Page {
	pub slug: String,
	pub title: String,
	pub description: String,
	pub date: Date,
	pub date_str: String,
	pub date_display: String,
	pub draft: bool,
	pub tags: Vec<Tag>,
	pub image: String,
	pub dir: PathBuf,
	pub content_rel: PathBuf,
	pub content_path: PathBuf,
}

impl Page {
	pub fn is_future(&self, today: Date) -> bool {
		self.date > today
	}
}

// Returns a String of the default content
fn default_content() -> String {
	DEFAULT_CONTENT.to_string()
}

impl PageFile {
	pub fn load(dir: &Path) -> Result<Self> {
		let path = dir.join(PAGE_META);
		let text = fs::read_to_string(&path)
			.with_context(|| format!("failed to read {}", path.display()))?;
		let file: Self =
			toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))?;
		file.validate()
			.with_context(|| format!("failed to write {}", path.display()))?;
		Ok(file)
	}

	pub fn save(&self, dir: &Path) -> Result<()> {
		self.validate()?;
		let text = toml::to_string_pretty(self).context("failed to serialize page.toml")?;
		let path = dir.join(PAGE_META);
		fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))?;
		Ok(())
	}

	pub fn validate(&self) -> Result<()> {
		if self.title.trim().is_empty() {
			bail!("page title cannot be empty");
		}

		parse_date(&self.date)?;

		let image = self.image.trim();
		if !image.is_empty() && !is_remote_asset(image) {
			let image = image.strip_prefix("./").unwrap_or(image);
			validate_relative_path(image, "image")?;
		}

		Ok(())
	}
}

pub fn is_remote_asset(value: &str) -> bool {
	value.starts_with('/') || value.contains("://") || value.starts_with("data:")
}

fn validate_relative_path(value: &str, field: &str) -> Result<PathBuf> {
	let rel = Path::new(value);
	if rel.as_os_str().is_empty() {
		bail!("{field} path cannot be empty");
	}

	if rel.is_absolute()
		|| rel
			.components()
			.any(|comp| !matches!(comp, Component::Normal(_)))
	{
		bail!("{field} path must be a relative path without '.' or '..'");
	}

	Ok(rel.to_path_buf())
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

// Returns the local date and UTC offset, falling back to UTC when the platform cannot determine the local offset
pub fn local_now() -> (Date, UtcOffset) {
	match OffsetDateTime::now_local() {
		Ok(now) => (now.date(), now.offset()),
		Err(_) => {
			let now = OffsetDateTime::now_utc();
			(now.date(), now.offset())
		}
	}
}

// Gets today's date in local time
pub fn today() -> Date {
	local_now().0
}

// Formats an offset as UTC+HH:MM or UTC-HH:MM
pub fn format_offset(offset: UtcOffset) -> String {
	let sign = if offset.is_negative() { '-' } else { '+' };
	let hours = offset.whole_hours().abs();
	let mins = offset.minutes_past_hour().abs();
	format!("UTC{sign}{hours:02}:{mins:02}")
}

// Formats the date if possible
pub fn format_date(date: Date) -> Result<String> {
	date.format(DATE_FORMAT)
		.context("format_date: cannot format date into YYYY-MM-DD")
}

pub fn format_date_display(date: Date) -> Result<String> {
	date.format(DATE_DISPLAY_FORMAT)
		.context("format_date_display: cannot format date for display")
}

// Extract a page from the given directory (page.md with metadata in page.toml)
pub fn load_page(dir: &Path) -> Result<Page> {
	let meta_path = dir.join(PAGE_META);
	let file = PageFile::load(dir)?;

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
	let date_display = format_date_display(date)?;
	let tags = tags::normalize(&file.tags, &meta_path)?;

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

	let image = file
		.image
		.trim()
		.strip_prefix("./")
		.unwrap_or(file.image.trim())
		.to_string();

	if !image.is_empty() && is_remote_asset(&image) {
		let rel = validate_relative_path(&image, "image")?;
		let image_path = dir.join(rel);
		if !image_path.is_file() {
			bail!(
				"load_page: image file not found: {} (referenced by {})",
				image_path.display(),
				meta_path.display()
			);
		}

		let image_canonical = image_path
			.canonicalize()
			.with_context(|| format!("load_page: failed to resolve {}", image_path.display()))?;
		if !image_canonical.starts_with(&dir_canonical) {
			bail!(
				"load_page: image file {} is outside of page directory {}",
				image_path.display(),
				dir_canonical.display()
			);
		}
	}

	Ok(Page {
		slug: slug.to_string(),
		title: title.to_string(),
		description: file.description,
		date,
		date_str,
		date_display,
		draft: file.draft,
		tags,
		image,
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
		let name_str = name.to_string_lossy();
		if name_str.starts_with(".") {
			continue;
		}
		if name_str == "static" || name_str == "templates" {
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
		assert_eq!(format_date_display(date).unwrap(), "February 29, 2024");
	}

	#[test]
	fn format_date_display_uses_long_month_and_unpadded_day() {
		assert_eq!(
			format_date_display(parse_date("2024-01-01").unwrap()).unwrap(),
			"January 1, 2024"
		);
		assert_eq!(
			format_date_display(parse_date("2024-12-31").unwrap()).unwrap(),
			"December 31, 2024"
		);
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
	fn format_offset_includes_sign_hours_and_minutes() {
		use time::macros::offset;

		assert_eq!(format_offset(UtcOffset::UTC), "UTC+00:00");
		assert_eq!(format_offset(offset!(+5:30)), "UTC+05:30");
		assert_eq!(format_offset(offset!(-8:00)), "UTC-08:00");
		assert_eq!(format_offset(offset!(-3:30)), "UTC-03:30");
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
		assert_eq!(page.date_display, "January 2, 2024");
		assert_eq!(page.content_rel.as_path(), std::path::Path::new("post.md"));
		assert_eq!(page.content_path, dir.join("post.md"));
	}

	#[test]
	fn load_pages_skips_static_and_templates() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path();

		fs::create_dir(root.join("static")).unwrap();
		fs::write(root.join("static/page.md"), "# Static\n").unwrap();
		fs::create_dir(root.join("templates")).unwrap();
		fs::write(root.join("templates/index.html"), "template").unwrap();

		let page_dir = root.join("post");
		fs::create_dir(&page_dir).unwrap();
		fs::write(
			page_dir.join(PAGE_META),
			"title = \"Post\"\ndate = \"2024-01-01\"\n",
		)
		.unwrap();
		fs::write(page_dir.join(DEFAULT_CONTENT), "# Post\n").unwrap();

		let output = root.join("build");
		let pages = load_pages(root, &output).unwrap();

		assert_eq!(pages.len(), 1);
		assert_eq!(pages[0].slug, "post");
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

	#[test]
	fn page_file_save_and_load_round_trip() {
		let temp = tempfile::tempdir().unwrap();
		let dir = temp.path().join("post");
		fs::create_dir(&dir).unwrap();
		fs::write(dir.join("cover.jpg"), b"image").unwrap();
		fs::write(dir.join("page.md"), "# Post\n").unwrap();

		let page = PageFile {
			title: "Post".to_string(),
			description: String::new(),
			date: "2024-01-01".to_string(),
			draft: false,
			tags: vec!["Rust".to_string()],
			image: "cover.jpg".to_string(),
			content: "page.md".to_string(),
		};
		page.save(&dir).unwrap();

		let loaded = PageFile::load(&dir).unwrap();
		assert_eq!(loaded, page);
	}

	#[test]
	fn page_file_validate_rejects_bad_image_path() {
		let page = PageFile {
			title: "Post".to_string(),
			description: String::new(),
			date: "2024-01-01".to_string(),
			draft: false,
			tags: Vec::new(),
			image: "../cover.jpg".to_string(),
			content: "page.md".to_string(),
		};
		assert!(page.validate().is_err());
	}
}
