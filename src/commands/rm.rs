use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::config::BlogConfig;
use crate::page::{PAGE_META, PageFile};
use crate::slug;

pub fn run(slug: &str, force: bool, tags: &[String]) -> Result<()> {
	let cwd = std::env::current_dir().context("failed to determine current directory")?;
	let root = BlogConfig::find_root(&cwd)?;
	run_in(&root, slug, force, tags)
}

pub fn run_in(root: &Path, slug: &str, force: bool, tags: &[String]) -> Result<()> {
	slug::validate_existing(slug)?;

	let dir = root.join(slug);
	if !dir.is_dir() {
		bail!("page not found: {slug}");
	}

	if !dir.join(PAGE_META).is_file() {
		bail!("{slug} is not a bd page (missing {PAGE_META})");
	}

	let metadata = fs::symlink_metadata(&dir)
		.with_context(|| format!("failed to inspect {}", dir.display()))?;
	if metadata.file_type().is_symlink() {
		let action = if tags.is_empty() { "remove" } else { "edit" };
		bail!("refusing to {action} symlink: {}", dir.display());
	}

	if !tags.is_empty() {
		return remove_tags(&dir, slug, tags);
	}

	if !force {
		if !io::stdin().is_terminal() {
			bail!("refusing to delete without --force (stdin is not a terminal)");
		}

		print!("Delete page \"{slug}\"? [y/N] ");
		io::stdout().flush().context("failed to flush stdout")?;

		let mut answer = String::new();
		io::stdin()
			.read_line(&mut answer)
			.context("failed to read confirmation")?;

		if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
			println!("Cancelled.");
			return Ok(());
		}
	}

	fs::remove_dir_all(&dir).with_context(|| format!("failed to remove {}", dir.display()))?;
	println!("Removed page \"{slug}\"");
	Ok(())
}

fn remove_tags(dir: &Path, slug: &str, tags: &[String]) -> Result<()> {
	let meta_path = dir.join(PAGE_META);
	let metadata = fs::symlink_metadata(&meta_path)
		.with_context(|| format!("failed to inspect {}", meta_path.display()))?;
	if metadata.file_type().is_symlink() {
		bail!("cannot edit symlink: {}", meta_path.display());
	}

	let text = fs::read_to_string(&meta_path)
		.with_context(|| format!("failed to read {}", meta_path.display()))?;
	let mut page: PageFile = toml::from_str(&text)
		.with_context(|| format!("failed to parse {}", meta_path.display()))?;

	let requested = requested_slugs(tags)?;
	let before = page.tags.len();
	page.tags.retain(|tag| {
		let tag_slug = slug::slugify(tag.trim());
		!requested.iter().any(|req| req == &tag_slug)
	});
	let removed = before - page.tags.len();

	if removed == 0 {
		bail!("page \"{slug}\" has none of the requested tags");
	}

	let meta = toml::to_string_pretty(&page).context("failed to serialize page.toml")?;
	fs::write(&meta_path, meta)
		.with_context(|| format!("failed to write {}", meta_path.display()))?;

	println!(
		"Removed {removed} tag{} from page \"{slug}\"",
		if removed == 1 { "" } else { "s" }
	);
	Ok(())
}

fn requested_slugs(tags: &[String]) -> Result<Vec<String>> {
	let mut slugs = Vec::new();

	for value in tags {
		let name = value.trim();
		if name.is_empty() {
			bail!("tag cannot be empty");
		}

		let tag_slug = slug::slugify(name);
		if tag_slug.is_empty() {
			bail!("tag {name:?} cannot be converted to a URL slug");
		}

		if !slugs.contains(&tag_slug) {
			slugs.push(tag_slug);
		}
	}

	Ok(slugs)
}
