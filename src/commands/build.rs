use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use crate::config::BlogConfig;
use crate::markdown;
use crate::page::{Page, load_pages};
use crate::render::{PostView, Renderer};

pub fn run(include_drafts_flag: bool, output_override: Option<&Path>, verbose: bool) -> Result<()> {
	let cwd = std::env::current_dir().context("failed to determine current directory")?;
	let root = BlogConfig::find_root(&cwd)?;
	let blog = BlogConfig::load(&root)?;

	let output_rel = output_override
		.map(Path::to_path_buf)
		.unwrap_or_else(|| blog.build.output.clone());
	let output = resolve_output(&root, &output_rel)?;
	let include_drafts = include_drafts_flag || blog.build.include_drafts;

	let all_pages = load_pages(&root, &output)?;
	let total = all_pages.len();
	let pages: Vec<Page> = all_pages
		.into_iter()
		.filter(|page| include_drafts || !page.draft)
		.collect();
	let skipped = total - pages.len();

	let renderer = Renderer::new()?;
	let mut rendered = Vec::with_capacity(pages.len());

	for page in &pages {
		let md_text = fs::read_to_string(&page.content_path)
			.with_context(|| format!("failed to read {}", page.content_path.display()))?;
		let content = markdown::render(&md_text);
		let view = PostView::from(page);
		let html = renderer
			.post(&blog, &view, &content)
			.with_context(|| format!("failed to render page {}", page.slug))?;
		rendered.push((page, html));
	}

	let views: Vec<PostView<'_>> = pages.iter().map(PostView::from).collect();
	let index = renderer
		.index(&blog, &views)
		.context("failed to render index")?;

	remove_existing_output(&output)?;
	fs::create_dir_all(&output)
		.with_context(|| format!("failed to create {}", output.display()))?;
	fs::write(output.join(".nojekyll"), "")
		.with_context(|| format!("failed to create {}", output.join(".nojekyll").display()))?;

	for (page, html) in &rendered {
		let page_output = output.join(&page.slug);
		fs::create_dir_all(&page_output)
			.with_context(|| format!("failed to create {}", page_output.display()))?;
		let assets = copy_assets(page, &page_output, verbose)?;
		fs::write(page_output.join("index.html"), html.as_bytes()).with_context(|| {
			format!(
				"failed to write {}",
				page_output.join("index.html").display()
			)
		})?;

		if verbose {
			println!(
				"    {} ({} asset{})",
				page.slug,
				assets,
				if assets == 1 { "" } else { "s" }
			);
		}
	}

	fs::write(output.join("index.html"), index.as_bytes())
		.with_context(|| format!("failed to write {}", output.join("index.html").display()))?;

	println!(
		"Built {} page{} to {}",
		pages.len(),
		if pages.len() == 1 { "" } else { "s" },
		output.display()
	);
	if skipped > 0 {
		println!(
			"Skipped {} draft{}.",
			skipped,
			if skipped == 1 { "" } else { "s" }
		);
	}

	Ok(())
}

fn resolve_output(root: &Path, rel: &Path) -> Result<PathBuf> {
	if rel.as_os_str().is_empty() {
		bail!("build output cannot be empty");
	}

	if rel.is_absolute() {
		bail!("build output must be relative to the blog root");
	}

	for comp in rel.components() {
		match comp {
			Component::Normal(name) => {
				if name.to_string_lossy().starts_with(".") {
					bail!("build output cannot contain hidden path components");
				}
			}
			_ => bail!("build output must not contain '.' or '..'"),
		}
	}

	let output = root.join(rel);
	if output.as_path() == root {
		bail!("build output cannot be the blog root");
	}

	let mut existing = output.as_path();
	while !existing.exists() {
		let Some(parent) = existing.parent() else {
			bail!("build output has no existing parent");
		};

		existing = parent;
	}

	let canonical = existing
		.canonicalize()
		.with_context(|| format!("failed to resolve {}", existing.display()))?;
	if !canonical.starts_with(root) {
		bail!(
			"build output {} resolves outside the blog root",
			output.display()
		);
	}

	Ok(output)
}

fn remove_existing_output(output: &Path) -> Result<()> {
	let metadata = match fs::symlink_metadata(output) {
		Ok(metadata) => metadata,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
		Err(error) => {
			Err(error).with_context(|| format!("failed to inspect {}", output.display()))?
		}
	};

	if metadata.file_type().is_symlink() {
		bail!(
			"refusing to use symlink as build output: {}",
			output.display()
		);
	}

	if !metadata.is_dir() {
		bail!("build output is not a directory: {}", output.display());
	}

	fs::remove_dir_all(output).with_context(|| format!("failed to remove {}", output.display()))
}

fn copy_assets(page: &Page, out_dir: &Path, verbose: bool) -> Result<usize> {
	let mut copied = 0;

	for entry in WalkDir::new(&page.dir)
		.follow_links(false)
		.sort_by_file_name()
	{
		let entry = entry.with_context(|| format!("failed to walk {}", page.dir.display()))?;
		let rel = entry
			.path()
			.strip_prefix(&page.dir)
			.with_context(|| format!("failed to relativize {}", entry.path().display()))?;

		if rel.as_os_str().is_empty() {
			continue;
		}

		if rel == Path::new("page.toml") || rel == page.content_rel.as_path() {
			continue;
		}

		let file_name = rel.file_name().and_then(|name| name.to_str()).unwrap_or("");
		if file_name == ".DS_Store" || file_name == "Thumbs.db" {
			continue;
		}

		if entry.file_type().is_symlink() {
			if verbose {
				eprintln!("warning: skipping symlink {}", entry.path().display());
			}
			continue;
		}

		if !entry.file_type().is_file() {
			continue;
		}

		if rel == Path::new("index.html") {
			bail!(
				"asset {} conflicts with generated page in {}",
				rel.display(),
				page.dir.display()
			);
		}

		let dest = out_dir.join(rel);
		if let Some(parent) = dest.parent() {
			fs::create_dir_all(parent)
				.with_context(|| format!("failed to create {}", parent.display()))?;
		}

		fs::copy(entry.path(), &dest).with_context(|| {
			format!(
				"failed to copy {} to {}",
				entry.path().display(),
				dest.display()
			)
		})?;
		copied += 1;
	}

	Ok(copied)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::page::{Page, parse_date};

	fn page_fixture(root: &Path, slug: &str) -> Page {
		let dir = root.join(slug);
		Page {
			slug: slug.to_string(),
			title: "Title".to_string(),
			description: String::new(),
			date: parse_date("2024-01-02").unwrap(),
			date_str: "2024-01-02".to_string(),
			draft: false,
			content_rel: PathBuf::from("page.md"),
			content_path: dir.join("page.md"),
			dir,
		}
	}

	#[test]
	fn resolve_output_rejects_empty_absolute_and_dot_paths() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().canonicalize().unwrap();

		for rel in [Path::new(""), Path::new("."), Path::new("..")] {
			assert!(resolve_output(&root, rel).is_err(), "{rel:?}");
		}

		let absolute = root.join("absolute");
		let error = resolve_output(&root, &absolute).unwrap_err().to_string();
		assert!(error.contains("must be relative"), "{error}");
	}

	#[test]
	fn resolve_output_rejects_hidden_components() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().canonicalize().unwrap();

		for rel in [".site", "nested/.site"] {
			let error = resolve_output(&root, Path::new(rel))
				.unwrap_err()
				.to_string();
			assert!(error.contains("hidden path components"), "{error}");
		}
	}

	#[test]
	fn resolve_output_accepts_nested_path() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().canonicalize().unwrap();

		let output = resolve_output(&root, Path::new("nested/site")).unwrap();

		assert_eq!(output, root.join("nested/site"));
	}

	#[cfg(unix)]
	#[test]
	fn resolve_output_rejects_symlink_escape() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().join("blog");
		let outside = temp.path().join("outside");
		fs::create_dir(&root).unwrap();
		fs::create_dir(&outside).unwrap();
		symlink(&outside, root.join("link")).unwrap();

		let error = resolve_output(&root, Path::new("link/site"))
			.unwrap_err()
			.to_string();

		assert!(error.contains("outside the blog root"), "{error}");
	}

	#[test]
	fn remove_existing_output_handles_missing_file_and_directory() {
		let temp = tempfile::tempdir().unwrap();
		let missing = temp.path().join("missing");
		remove_existing_output(&missing).unwrap();

		let file = temp.path().join("file");
		fs::write(&file, "not a directory").unwrap();
		let error = remove_existing_output(&file).unwrap_err().to_string();
		assert!(error.contains("not a directory"), "{error}");

		let dir = temp.path().join("build");
		fs::create_dir(&dir).unwrap();
		fs::write(dir.join("index.html"), "old").unwrap();
		remove_existing_output(&dir).unwrap();
		assert!(!dir.exists());
	}

	#[cfg(unix)]
	#[test]
	fn remove_existing_output_refuses_symlink() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let target = temp.path().join("target");
		fs::create_dir(&target).unwrap();
		let link = temp.path().join("build");
		symlink(&target, &link).unwrap();

		let error = remove_existing_output(&link).unwrap_err().to_string();

		assert!(error.contains("refusing to use symlink"), "{error}");
		assert!(target.is_dir());
	}

	#[test]
	fn copy_assets_copies_files_and_skips_metadata() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path();
		let page = page_fixture(root, "post");
		fs::create_dir_all(page.dir.join("files")).unwrap();
		fs::write(page.dir.join("page.toml"), "title = \"T\"\n").unwrap();
		fs::write(page.dir.join("page.md"), "# Body\n").unwrap();
		fs::write(page.dir.join("photo.bin"), [0u8, 1, 2]).unwrap();
		fs::write(page.dir.join("files/note.txt"), "hello").unwrap();
		fs::write(page.dir.join(".DS_Store"), "ignored").unwrap();
		fs::write(page.dir.join("Thumbs.db"), "ignored").unwrap();

		let out = root.join("out");
		fs::create_dir(&out).unwrap();
		let copied = copy_assets(&page, &out, false).unwrap();

		assert_eq!(copied, 2);
		assert_eq!(fs::read(out.join("photo.bin")).unwrap(), vec![0, 1, 2]);
		assert_eq!(
			fs::read_to_string(out.join("files/note.txt")).unwrap(),
			"hello"
		);
		assert!(!out.join("page.toml").exists());
		assert!(!out.join("page.md").exists());
		assert!(!out.join(".DS_Store").exists());
		assert!(!out.join("Thumbs.db").exists());
	}

	#[test]
	fn copy_assets_rejects_index_html() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path();
		let page = page_fixture(root, "post");
		fs::create_dir_all(&page.dir).unwrap();
		fs::write(page.dir.join("index.html"), "custom").unwrap();

		let out = root.join("out");
		fs::create_dir(&out).unwrap();
		let error = copy_assets(&page, &out, false).unwrap_err().to_string();

		assert!(error.contains("conflicts with generated page"), "{error}");
	}

	#[cfg(unix)]
	#[test]
	fn copy_assets_skips_symlinks() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let root = temp.path();
		let page = page_fixture(root, "post");
		fs::create_dir_all(&page.dir).unwrap();
		let outside = root.join("outside.txt");
		fs::write(&outside, "outside").unwrap();
		symlink(&outside, page.dir.join("link.txt")).unwrap();

		let out = root.join("out");
		fs::create_dir(&out).unwrap();
		let copied = copy_assets(&page, &out, false).unwrap();

		assert_eq!(copied, 0);
		assert!(!out.join("link.txt").exists());
	}
}
