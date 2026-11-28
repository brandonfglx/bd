use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::config::BlogConfig;
use crate::page::{PAGE_META, PageFile, format_date, format_offset, local_now, parse_date};
use crate::{slug, tags};

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

pub fn run(
	title: &str,
	slug_arg: Option<&str>,
	date_arg: Option<&str>,
	draft: bool,
	tags: &[String],
) -> Result<()> {
	let cwd = std::env::current_dir().context("failed to determine current directory")?;
	let root = BlogConfig::find_root(&cwd)?;
	run_in(&root, title, slug_arg, date_arg, draft, tags)
}

pub fn run_in(
	root: &Path,
	title: &str,
	slug_arg: Option<&str>,
	date_arg: Option<&str>,
	draft: bool,
	tags: &[String],
) -> Result<()> {
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
	let existing = match fs::symlink_metadata(&dir) {
		Ok(metadata) => Some(metadata),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
		Err(error) => {
			return Err(error).with_context(|| format!("failed to inspect {}", dir.display()));
		}
	};

	if let Some(metadata) = existing {
		if tags.is_empty() {
			bail!("page directory already exists: {}", dir.display());
		}

		if metadata.file_type().is_symlink() {
			bail!("cannot edit symlink: {}", dir.display());
		}

		if !metadata.is_dir() {
			bail!("page path is not a directory: {}", dir.display());
		}

		if date_arg.is_some() || draft {
			bail!("--date and --draft cannot be used when adding tags to an existing page");
		}

		return add_tags(&dir, &slug, tags);
	}

	let tag_names = tags::normalize(tags, &dir.join(PAGE_META))?
		.into_iter()
		.map(|tag| tag.name)
		.collect();

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
		tags: tag_names,
		image: String::new(),
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

fn add_tags(dir: &Path, slug: &str, new_tags: &[String]) -> Result<()> {
	let meta_path = dir.join(PAGE_META);
	if !meta_path.is_file() {
		bail!("{slug} is not a bd page (missing {PAGE_META}");
	}

	let metadata = fs::symlink_metadata(&meta_path)
		.with_context(|| format!("failed to inspect {}", meta_path.display()))?;
	if metadata.file_type().is_symlink() {
		bail!("cannot edit symlink: {}", meta_path.display());
	}

	let text = fs::read_to_string(&meta_path)
		.with_context(|| format!("failed to read {}", meta_path.display()))?;
	let mut page: PageFile = toml::from_str(&text)
		.with_context(|| format!("failed to parse {}", meta_path.display()))?;

	let before: Vec<String> = page
		.tags
		.iter()
		.map(|tag| slug::slugify(tag.trim()))
		.collect();

	let mut combined = page.tags.clone();
	combined.extend(new_tags.iter().cloned());
	let normalized = tags::normalize(&combined, &meta_path)?;

	let added = normalized
		.iter()
		.filter(|tag| !before.contains(&tag.slug))
		.count();
	if added == 0 {
		println!("Page \"{slug}\" already has the requested tags");
		return Ok(());
	}

	page.tags = normalized.into_iter().map(|tag| tag.name).collect();
	let meta = toml::to_string_pretty(&page).context("failed to serialize page.toml")?;
	fs::write(&meta_path, meta)
		.with_context(|| format!("failed to write {}", meta_path.display()))?;

	println!(
		"Added {added} tag{} to existing page \"{slug}\"",
		if added == 1 { "" } else { "s" }
	);
	Ok(())
}
