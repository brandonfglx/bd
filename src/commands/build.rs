use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use crate::config::BlogConfig;
use crate::markdown;
use crate::page::{Page, load_pages};
use crate::render::{PostView, Renderer};

const OUTPUT_MARKER: &str = ".bd-output";
const STAGING_PREFIX: &str = ".bd-build-";
const BACKUP_PREFIX: &str = ".bd-old-";

#[derive(Debug, Clone, Default)]
pub struct BuildArgs {
	pub drafts: bool,
	pub output: Option<PathBuf>,
	pub verbose: bool,
	pub force: bool,
}

#[derive(Debug)]
pub struct BuildOutcome {
	pub output: PathBuf,
	pub pages: usize,
	pub skipped: usize,
}

pub fn run(
	include_drafts_flag: bool,
	output_override: Option<&Path>,
	verbose: bool,
	force: bool,
) -> Result<()> {
	let outcome = build(&BuildArgs {
		drafts: include_drafts_flag,
		output: output_override.map(Path::to_path_buf),
		verbose,
		force,
	})?;
	print_summary(&outcome);
	Ok(())
}

pub fn build(args: &BuildArgs) -> Result<BuildOutcome> {
	let cwd = std::env::current_dir().context("failed to determine current directory")?;
	let root = BlogConfig::find_root(&cwd)?;
	let blog = BlogConfig::load(&root)?;

	let output_rel = args
		.output
		.clone()
		.unwrap_or_else(|| blog.build.output.clone());
	let output = resolve_output(&root, &output_rel)?;
	let include_drafts = args.drafts || blog.build.include_drafts;

	let all_pages = load_pages(&root, &output)?;
	validate_output(&output, args.force)?;
	let total = all_pages.len();
	let page_slugs: Vec<String> = all_pages.iter().map(|page| page.slug.clone()).collect();
	let pages: Vec<Page> = all_pages
		.into_iter()
		.filter(|page| include_drafts || !page.draft)
		.collect();
	let skipped = total - pages.len();

	let renderer = Renderer::with_root(&root)?;
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

	let parent = output
		.parent()
		.context("build output has no parent directory")?;
	fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

	let staging = Staging::create(parent)?;
	write_build(
		&root,
		staging.path(),
		&rendered,
		&index,
		&page_slugs,
		args.verbose,
	)?;
	staging.commit(&output, args.force)?;

	Ok(BuildOutcome {
		output,
		pages: pages.len(),
		skipped,
	})
}

pub fn print_summary(outcome: &BuildOutcome) {
	println!(
		"Built {} page{} to {}",
		outcome.pages,
		if outcome.pages == 1 { "" } else { "s" },
		outcome.output.display()
	);
	if outcome.skipped > 0 {
		println!(
			"Skipped {} draft{}.",
			outcome.skipped,
			if outcome.skipped == 1 { "" } else { "s" }
		);
	}
}

fn resolve_output(root: &Path, rel: &Path) -> Result<PathBuf> {
	if rel.as_os_str().is_empty() {
		bail!("build output cannot be empty");
	}

	if rel.is_absolute() {
		bail!("build output must be relative to the blog root");
	}

	if let Some(Component::Normal(first)) = rel.components().next() {
		let first = first.to_string_lossy();
		if first == "static" || first == "templates" {
			bail!("build output cannot be inside the {first} directory");
		}
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

fn validate_output(output: &Path, force: bool) -> Result<bool> {
	let metadata = match fs::symlink_metadata(output) {
		Ok(metadata) => metadata,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
		Err(error) => {
			return Err(error).with_context(|| format!("failed to inspect {}", output.display()));
		}
	};

	if metadata.file_type().is_symlink() {
		bail!("cannot use symlink as build output: {}", output.display());
	}

	if !metadata.is_dir() {
		bail!("build output is not a directory: {}", output.display());
	}

	let marker = output.join(OUTPUT_MARKER);
	match fs::symlink_metadata(&marker) {
		Ok(metadata) => {
			if !metadata.is_file() {
				bail!(
					"build output marker {} is not a regular file",
					marker.display()
				);
			}
			Ok(true)
		}
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
			if force {
				Ok(true)
			} else {
				bail!(
					"refusing to replace {} because it does not contain {} (use --force to override)",
					output.display(),
					OUTPUT_MARKER
				);
			}
		}
		Err(error) => Err(error).with_context(|| format!("failed to inspect {}", marker.display())),
	}
}

struct Staging {
	path: PathBuf,
	committed: bool,
}

impl Staging {
	fn create(parent: &Path) -> Result<Self> {
		for _ in 0..16 {
			let path = unique_path(parent, STAGING_PREFIX)?;
			match fs::create_dir(&path) {
				Ok(()) => {
					return Ok(Self {
						path,
						committed: false,
					});
				}
				Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
				Err(error) => {
					return Err(error).with_context(|| {
						format!("failed to create staging directory in {}", parent.display())
					});
				}
			}
		}

		bail!(
			"failed to create a unique staging directory in {}",
			parent.display()
		);
	}

	fn path(&self) -> &Path {
		&self.path
	}

	fn commit(mut self, output: &Path, force: bool) -> Result<()> {
		let replace = validate_output(output, force)?;
		let parent = output
			.parent()
			.context("build output has no parent directory")?;

		let backup = if replace {
			let backup = unique_path(parent, BACKUP_PREFIX)?;
			fs::rename(output, &backup).with_context(|| {
				format!(
					"failed to move previous build output {} to {}",
					output.display(),
					backup.display()
				)
			})?;
			Some(backup)
		} else {
			None
		};

		if let Err(error) = fs::rename(&self.path, output) {
			if let Some(backup) = &backup {
				match fs::rename(backup, output) {
					Ok(()) => {
						return Err(error).with_context(|| {
							format!(
								"failed to move new build into {}; previous output was restored",
								output.display()
							)
						});
					}
					Err(restore_error) => {
						return Err(error).with_context(|| {
							format!(
								"failed to move new build into {}; previous output is preserved at {} but could not be restored: {restore_error}",
								output.display(),
								backup.display()
							)
						});
					}
				}
			}

			return Err(error)
				.with_context(|| format!("failed to move new build into {}", output.display()));
		}

		self.committed = true;

		if let Some(backup) = backup
			&& let Err(error) = fs::remove_dir_all(&backup)
		{
			eprintln!(
				"warning: failed to remove previous build output {}: {error}",
				backup.display()
			);
		}

		Ok(())
	}
}

impl Drop for Staging {
	fn drop(&mut self) {
		if !self.committed {
			let _ = fs::remove_dir_all(&self.path);
		}
	}
}

fn unique_path(parent: &Path, prefix: &str) -> Result<PathBuf> {
	let nanos = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_nanos())
		.unwrap_or_default();
	let pid = std::process::id();

	for attempt in 0..100 {
		let candidate = parent.join(format!("{prefix}{pid}-{nanos}-{attempt}"));
		if !candidate.exists() {
			return Ok(candidate);
		}
	}

	bail!(
		"failed to find an unused temporary name in {}",
		parent.display()
	)
}

fn write_build(
	root: &Path,
	output: &Path,
	rendered: &[(&Page, String)],
	index: &str,
	page_slugs: &[String],
	verbose: bool,
) -> Result<()> {
	let marker = output.join(OUTPUT_MARKER);
	fs::write(&marker, format!("bd {}\n", env!("CARGO_PKG_VERSION")))
		.with_context(|| format!("failed to create {}", marker.display()))?;

	let nojekyll = output.join(".nojekyll");
	fs::write(&nojekyll, "").with_context(|| format!("failed to create {}", nojekyll.display()))?;

	copy_static(root, output, page_slugs, verbose)?;

	for (page, html) in rendered {
		let page_output = output.join(&page.slug);
		fs::create_dir_all(&page_output)
			.with_context(|| format!("failed to create {}", page_output.display()))?;
		let assets = copy_assets(page, &page_output, verbose)?;
		let index_path = page_output.join("index.html");
		fs::write(&index_path, html.as_bytes())
			.with_context(|| format!("failed to write {}", index_path.display()))?;

		if verbose {
			println!(
				"    {} ({} asset{})",
				page.slug,
				assets,
				if assets == 1 { "" } else { "s" }
			);
		}
	}

	let index_path = output.join("index.html");
	fs::write(&index_path, index.as_bytes())
		.with_context(|| format!("failed to write {}", index_path.display()))?;

	Ok(())
}

fn copy_static(root: &Path, output: &Path, page_slugs: &[String], verbose: bool) -> Result<usize> {
	let static_dir = root.join("static");
	let metadata = match fs::symlink_metadata(&static_dir) {
		Ok(metadata) => metadata,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
		Err(error) => {
			return Err(error)
				.with_context(|| format!("failed to inspect {}", static_dir.display()));
		}
	};

	if metadata.file_type().is_symlink() {
		bail!(
			"cannot use symlink as static directory: {}",
			static_dir.display()
		);
	}

	if !metadata.is_dir() {
		bail!("static path is not a directory: {}", static_dir.display());
	}

	let mut copied = 0;

	for entry in WalkDir::new(&static_dir)
		.follow_links(false)
		.sort_by_file_name()
	{
		let entry = entry.with_context(|| format!("failed to walk {}", static_dir.display()))?;
		let rel = entry
			.path()
			.strip_prefix(&static_dir)
			.with_context(|| format!("failed to relativize {}", entry.path().display()))?;

		if rel.as_os_str().is_empty() {
			continue;
		}

		let file_name = rel.file_name().and_then(|name| name.to_str()).unwrap_or("");
		if file_name == ".DS_Store" || file_name == "Thumbs.db" || file_name == ".gitkeep" {
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

		validate_static_path(rel, page_slugs)?;

		let dest = output.join(rel);
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

fn validate_static_path(rel: &Path, page_slugs: &[String]) -> Result<()> {
	let Some(Component::Normal(first)) = rel.components().next() else {
		bail!("static path must be relative: {}", rel.display());
	};

	let first = first.to_string_lossy();
	if first == "index.html" || first == ".nojekyll" || first == OUTPUT_MARKER {
		bail!(
			"static path {} conflicts with a generated file",
			rel.display()
		);
	}

	if page_slugs.iter().any(|slug| slug.as_str() == first) {
		bail!(
			"static path {} conflicts with page directory {}",
			rel.display(),
			first
		);
	}

	Ok(())
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
			date_display: "January 2, 2024".to_string(),
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
	fn resolve_output_rejects_static_and_template_directories() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().canonicalize().unwrap();

		for rel in [
			Path::new("static"),
			Path::new("static/site"),
			Path::new("templates"),
			Path::new("templates/site"),
		] {
			let error = resolve_output(&root, rel).unwrap_err().to_string();
			assert!(error.contains("inside the"), "{error}");
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
	fn validate_output_handles_missing_file_and_directory() {
		let temp = tempfile::tempdir().unwrap();

		let missing = temp.path().join("missing");
		assert!(!validate_output(&missing, false).unwrap());

		let file = temp.path().join("file");
		fs::write(&file, "not a directory").unwrap();
		let error = validate_output(&file, false).unwrap_err().to_string();
		assert!(error.contains("not a directory"), "{error}");

		let dir = temp.path().join("build");
		fs::create_dir(&dir).unwrap();
		fs::write(dir.join(OUTPUT_MARKER), "bd\n").unwrap();
		assert!(validate_output(&dir, false).unwrap());
	}

	#[test]
	fn validate_output_requires_marker_unless_forced() {
		let temp = tempfile::tempdir().unwrap();
		let dir = temp.path().join("build");
		fs::create_dir(&dir).unwrap();

		let error = validate_output(&dir, false).unwrap_err().to_string();
		assert!(error.contains("does not contain .bd-output"), "{error}");
		assert!(error.contains("--force"), "{error}");
		assert!(validate_output(&dir, true).unwrap());
	}

	#[test]
	fn validate_output_rejects_non_file_marker() {
		let temp = tempfile::tempdir().unwrap();
		let dir = temp.path().join("build");
		fs::create_dir(&dir).unwrap();
		fs::create_dir(dir.join(OUTPUT_MARKER)).unwrap();

		let error = validate_output(&dir, true).unwrap_err().to_string();
		assert!(error.contains("is not a regular file"), "{error}");
	}

	#[cfg(unix)]
	#[test]
	fn validate_output_refuses_symlink() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let target = temp.path().join("target");
		fs::create_dir(&target).unwrap();
		let link = temp.path().join("build");
		symlink(&target, &link).unwrap();

		let error = validate_output(&link, false).unwrap_err().to_string();

		assert!(error.contains("cannot use symlink"), "{error}");
		assert!(target.is_dir());
	}

	#[test]
	fn staging_drop_removes_uncommitted_directory() {
		let temp = tempfile::tempdir().unwrap();
		let staging = Staging::create(temp.path()).unwrap();
		let path = staging.path().to_path_buf();
		fs::write(path.join("file"), "x").unwrap();

		drop(staging);

		assert!(!path.exists());
	}

	#[test]
	fn staging_commit_replaces_existing_output() {
		let temp = tempfile::tempdir().unwrap();
		let output = temp.path().join("build");
		fs::create_dir(&output).unwrap();
		fs::write(output.join(OUTPUT_MARKER), "old").unwrap();
		fs::write(output.join("old.txt"), "old").unwrap();

		let staging = Staging::create(temp.path()).unwrap();
		fs::write(staging.path().join(OUTPUT_MARKER), "new").unwrap();
		fs::write(staging.path().join("index.html"), "new").unwrap();
		staging.commit(&output, false).unwrap();

		assert_eq!(
			fs::read_to_string(output.join("index.html")).unwrap(),
			"new"
		);
		assert_eq!(
			fs::read_to_string(output.join(OUTPUT_MARKER)).unwrap(),
			"new"
		);
		assert!(!output.join("old.txt").exists());

		let leftovers: Vec<_> = fs::read_dir(temp.path())
			.unwrap()
			.filter_map(|entry| entry.ok())
			.map(|entry| entry.file_name().to_string_lossy().into_owned())
			.filter(|name| name.starts_with(BACKUP_PREFIX))
			.collect();
		assert!(leftovers.is_empty(), "leftover backups: {leftovers:?}");
	}

	#[test]
	fn copy_static_copies_files_and_skips_noise() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path();
		let static_dir = root.join("static");
		fs::create_dir_all(static_dir.join("nested")).unwrap();
		fs::write(static_dir.join("style.css"), "body {}").unwrap();
		fs::write(static_dir.join("nested/note.txt"), "hello").unwrap();
		fs::write(static_dir.join(".gitkeep"), "").unwrap();
		fs::write(static_dir.join(".DS_Store"), "ignored").unwrap();
		fs::write(static_dir.join("Thumbs.db"), "ignored").unwrap();

		let out = root.join("out");
		fs::create_dir(&out).unwrap();
		let copied = copy_static(root, &out, &[], false).unwrap();

		assert_eq!(copied, 2);
		assert_eq!(
			fs::read_to_string(out.join("style.css")).unwrap(),
			"body {}"
		);
		assert!(!out.join(".gitkeep").exists());
		assert!(!out.join(".DS_Store").exists());
		assert!(!out.join("Thumbs.db").exists());
	}

	#[test]
	fn copy_static_rejects_generated_file_conflicts() {
		for rel in ["index.html", ".nojekyll", OUTPUT_MARKER] {
			let temp = tempfile::tempdir().unwrap();
			let root = temp.path();
			let static_dir = root.join("static");
			fs::create_dir(&static_dir).unwrap();
			fs::write(static_dir.join(rel), "x").unwrap();

			let out = root.join("out");
			fs::create_dir(&out).unwrap();
			let error = copy_static(root, &out, &[], false).unwrap_err().to_string();

			assert!(error.contains("conflicts with a generated file"), "{error}");
		}
	}

	#[test]
	fn copy_static_rejects_page_conflicts() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path();
		let static_dir = root.join("static");
		fs::create_dir_all(static_dir.join("post")).unwrap();
		fs::write(static_dir.join("post/file.txt"), "x").unwrap();

		let out = root.join("out");
		fs::create_dir(&out).unwrap();
		let error = copy_static(root, &out, &["post".to_string()], false)
			.unwrap_err()
			.to_string();

		assert!(error.contains("conflicts with page directory"), "{error}");
	}

	#[test]
	fn copy_static_missing_directory_is_ok() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path();
		let out = root.join("out");
		fs::create_dir(&out).unwrap();

		assert_eq!(copy_static(root, &out, &[], false).unwrap(), 0);
	}

	#[test]
	fn copy_static_rejects_static_file() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path();
		fs::write(root.join("static"), "not a directory").unwrap();

		let out = root.join("out");
		fs::create_dir(&out).unwrap();
		let error = copy_static(root, &out, &[], false).unwrap_err().to_string();

		assert!(error.contains("not a directory"), "{error}");
	}

	#[cfg(unix)]
	#[test]
	fn copy_static_skips_symlinks() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let root = temp.path();
		let static_dir = root.join("static");
		fs::create_dir(&static_dir).unwrap();
		let outside = root.join("outside.txt");
		fs::write(&outside, "outside").unwrap();
		symlink(&outside, static_dir.join("link.txt")).unwrap();

		let out = root.join("out");
		fs::create_dir(&out).unwrap();
		let copied = copy_static(root, &out, &[], false).unwrap();

		assert_eq!(copied, 0);
		assert!(!out.join("link.txt").exists());
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
