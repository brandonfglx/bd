use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::commands::add::default_page_markdown;
use crate::config::{BLOG_CONFIG, BlogConfig, BuildConfig};
use crate::page::{PAGE_META, PageFile, format_date, today};

const GITIGNORE: &str = include_str!("../base/.gitignore");
const PAGES_WORKFLOW: &str = include_str!("../base/pages.yml");
const DEFAULT_PAGE: &str = "my-page";
const BD_REPO: &str = "brandonfglx/bd";
const BD_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(location: &Path) -> Result<()> {
	let location = std::path::absolute(location)
		.with_context(|| format!("init: failed to resolve {}", location.display()))?;

	if location.join(BLOG_CONFIG).exists() {
		bail!(
			"init: {} already contains {BLOG_CONFIG}",
			location.display()
		);
	}

	if location.join(DEFAULT_PAGE).exists() {
		bail!(
			"init: {} already contains a {DEFAULT_PAGE} page",
			location.display()
		);
	}

	if location.join(".github/workflows/pages.yml").exists() {
		bail!(
			"init: {} already contains .github/workflows/pages.yml",
			location.display()
		);
	}

	if PAGES_WORKFLOW.trim().is_empty() {
		bail!(
			"init: embedded GitHub Pages workflow is empty; fill src/base/pages.yml and rebuild bd"
		);
	}

	let needs_git_init = !location.join(".git").exists();
	if needs_git_init {
		check_git()?;
	}

	fs::create_dir_all(&location)
		.with_context(|| format!("init: failed to create {}", location.display()))?;

	let blog = BlogConfig {
		title: "My Blog".to_string(),
		description: "Auto-generated blog by bd".to_string(),
		author: "User".to_string(),
		language: "en".to_string(),
		build: BuildConfig::default(),
	};
	let blog_toml = toml::to_string_pretty(&blog).context("init: failed to serialize blog.toml")?;
	fs::write(location.join(BLOG_CONFIG), blog_toml).with_context(|| {
		format!(
			"init: failed to write {}",
			location.join(BLOG_CONFIG).display()
		)
	})?;

	write_gitignore(&location.join(".gitignore"))?;

	let workflow_dir = location.join(".github/workflows");
	fs::create_dir_all(&workflow_dir)
		.with_context(|| format!("init: failed to create {}", workflow_dir.display()))?;
	let workflow = PAGES_WORKFLOW
		.replace("__BD_VERSION__", BD_VERSION)
		.replace("__BD_REPO__", BD_REPO);
	let workflow_path = workflow_dir.join("pages.yml");
	fs::write(&workflow_path, workflow)
		.with_context(|| format!("init: failed to write {}", workflow_path.display()))?;

	let page_dir = location.join(DEFAULT_PAGE);
	fs::create_dir_all(&page_dir)
		.with_context(|| format!("init: failed to create {}", page_dir.display()))?;
	let page = PageFile {
		title: "My First Page".to_string(),
		description: "My first post.".to_string(),
		date: format_date(today())?,
		draft: false,
		content: "page.md".to_string(),
	};
	let page_toml = toml::to_string_pretty(&page).context(format!(
		"init: failed to serialize {DEFAULT_PAGE}/page.toml"
	))?;
	fs::write(page_dir.join(PAGE_META), page_toml).with_context(|| {
		format!(
			"init: failed to write {}",
			page_dir.join(PAGE_META).display()
		)
	})?;
	fs::write(page_dir.join("page.md"), default_page_markdown()).with_context(|| {
		format!(
			"init: failed to write {}",
			page_dir.join("page.md").display()
		)
	})?;

	if needs_git_init {
		git_init(&location)?;
	}

	println!("Initialized bd blog in {}", location.display());
	println!();
	println!("Next steps:");
	println!("    cd {}", location.display());
	println!("    $EDITOR blog.toml");
	println!("    bd add \"My first post\"");
	println!("    bd build");
	println!("    git add . && git commit -m \"First post\"");
	println!(
		"    # Enable GitHub Pages from workflow file in Repository Settings -> Pages -> Source: GitHub Actions"
	);
	println!("    git remote add origin [Repo URL/SSH here] && git push -u origin main");

	Ok(())
}

fn check_git() -> Result<()> {
	let output = Command::new("git")
		.arg("--version")
		.output()
		.context("failed to run `git --version`; git may not be installed: see FAQ at https://github.com/brandonfglx/bd/blob/main/README.md")?;
	if !output.status.success() {
		bail!(
			"`git --version` failed: {}",
			String::from_utf8_lossy(&output.stderr).trim()
		);
	}

	Ok(())
}

fn git_init(location: &Path) -> Result<()> {
	let output = Command::new("git")
		.arg("init")
		.arg("-b")
		.arg("main")
		.current_dir(location)
		.output()
		.context("failed to run `git init`; git may not be installed: see FAQ at https://github.com/brandonfglx/bd/blob/main/README.md")?;
	if !output.status.success() {
		bail!(
			"`git init -b main` failed: {}",
			String::from_utf8_lossy(&output.stderr).trim()
		);
	}

	Ok(())
}

fn write_gitignore(path: &Path) -> Result<()> {
	let mut contents = if path.exists() {
		fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?
	} else {
		String::new()
	};

	if !contents.is_empty() && !contents.ends_with('\n') {
		contents.push('\n');
	}

	let mut lines: Vec<&str> = GITIGNORE
		.lines()
		.map(str::trim)
		.filter(|line| !line.is_empty())
		.collect();
	if lines.is_empty() {
		lines = vec!["/build", ".DS_Store"];
	}

	for line in lines {
		if !contents.lines().any(|existing| existing.trim() == line) {
			contents.push_str(line);
			contents.push('\n');
		}
	}

	fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn embedded_workflow_is_not_empty_and_has_placeholders() {
		assert!(
			!PAGES_WORKFLOW.trim().is_empty(),
			"src/base/pages.yml must contain the GitHub Pages workflow"
		);
		assert!(PAGES_WORKFLOW.contains("__BD_VERSION__"));
		assert!(PAGES_WORKFLOW.contains("__BD_REPO__"));
	}

	#[test]
	fn run_refuses_existing_blog() {
		let temp = tempfile::tempdir().unwrap();
		fs::write(temp.path().join(BLOG_CONFIG), "title = \"Existing\"\n").unwrap();

		let error = run(temp.path()).unwrap_err().to_string();

		assert!(error.contains("already contains blog.toml"), "{error}");
	}

	#[test]
	fn write_gitignore_creates_file() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join(".gitignore");

		write_gitignore(&path).unwrap();

		let contents = fs::read_to_string(&path).unwrap();
		assert!(contents.contains("/build"));
		assert!(contents.ends_with('\n'));
	}

	#[test]
	fn write_gitignore_preserves_existing_entries_and_deduplicates() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join(".gitignore");
		// No trailing newline on purpose.
		fs::write(&path, "target\n.DS_Store").unwrap();

		write_gitignore(&path).unwrap();
		write_gitignore(&path).unwrap();

		let contents = fs::read_to_string(&path).unwrap();
		assert!(contents.starts_with("target\n.DS_Store\n"), "{contents}");
		assert_eq!(contents.matches(".DS_Store").count(), 1);
		assert_eq!(contents.matches("/build").count(), 1);

		for line in GITIGNORE
			.lines()
			.map(str::trim)
			.filter(|line| !line.is_empty())
		{
			let count = contents
				.lines()
				.filter(|existing| existing.trim() == line)
				.count();
			assert_eq!(count, 1, "{line:?} should appear exactly once:\n{contents}");
		}
	}

	#[test]
	fn write_gitignore_is_idempotent() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join(".gitignore");

		write_gitignore(&path).unwrap();
		let first = fs::read_to_string(&path).unwrap();

		write_gitignore(&path).unwrap();

		assert_eq!(fs::read_to_string(&path).unwrap(), first);
	}
}
