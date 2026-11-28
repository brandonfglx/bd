use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use toml_edit::DocumentMut;

use crate::commands::add::default_page_markdown;
use crate::config::{BLOG_CONFIG, BlogConfig, BuildConfig, CssFramework};
use crate::git;
use crate::page::{PAGE_META, PageFile, format_date, today};
use crate::theme::ThemeName;

const GITIGNORE: &str = include_str!("../base/.gitignore");
const ABOUT: &str = include_str!("../base/about.md");
const PAGES_WORKFLOW: &str = include_str!("../base/pages.yml");
const README_TEMPLATE: &str = include_str!("../base/README.md");
const DEFAULT_PAGE: &str = "my-page";
const BD_REPO: &str = "brandonfglx/bd";
const BD_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(
	location: &Path,
	force: bool,
	upgrade: bool,
	css: Option<CssFramework>,
	template: Option<ThemeName>,
) -> Result<()> {
	let location = std::path::absolute(location)
		.with_context(|| format!("init: failed to resolve {}", location.display()))?;

	if upgrade {
		return upgrade_scaffolding(&location, css, template);
	}

	if !force {
		if location.join(BLOG_CONFIG).is_file() {
			if css.is_none() && template.is_none() {
				refuse_existing(&location)?;
			}

			if let Some(template) = template {
				write_template_setting(&location, template)?;
			}

			if let Some(css) = css {
				write_css_setting(&location, css)?;
			}

			return Ok(());
		}

		refuse_existing(&location)?;
	}

	if PAGES_WORKFLOW.trim().is_empty() {
		bail!(
			"init: embedded GitHub Pages workflow is empty; fill src/base/pages.yml and rebuild bd"
		);
	}

	let needs_git_init = !location.join(".git").exists();
	if needs_git_init {
		git::check()?;
	}

	fs::create_dir_all(&location)
		.with_context(|| format!("init: failed to create {}", location.display()))?;

	let theme = template.unwrap_or_default();
	let mut blog = default_blog();
	blog.template = theme;
	if let Some(css) = css {
		blog.css = css;
	}

	write_blog_config(&location, &blog)?;
	write_gitignore(&location.join(".gitignore"))?;
	write_workflow(&location)?;
	write_templates(&location, theme)?;
	write_static(&location, theme)?;
	write_readme(&location, &blog, force)?;
	write_about(&location, force)?;
	write_default_page(&location)?;

	if needs_git_init {
		git::init(&location)?;
	}

	println!("Initialized bd blog in {}", location.display());
	println!();
	println!("Next steps:");
	println!("    1. Use `bd add [name]` to create a new page");
	println!(
		"    2. Edit the blog.toml with your own information and edit page.md inside each directory"
	);
	println!(
		"    3. Run `bd build` and then `bd preview` to check the site locally using localhost."
	);
	println!(
		"    3. When done, execute `bd publish` (which may prompt to add a remote HTTP or SSH) - make sure Repo Settings -> Pages -> Source: GitHub Actions."
	);

	Ok(())
}

pub fn default_blog() -> BlogConfig {
	BlogConfig {
		title: "Default Blog".to_string(),
		description: "Auto-generated blog by bd".to_string(),
		author: "User".to_string(),
		language: "en".to_string(),
		template: ThemeName::Classic,
		css: CssFramework::Manual,
		favicon: String::new(),
		footer: Vec::new(),
		build: BuildConfig::default(),
	}
}

fn write_blog_config(location: &Path, blog: &BlogConfig) -> Result<()> {
	let blog_toml = toml::to_string_pretty(&blog).context("init: failed to serialize blog.toml")?;
	fs::write(location.join(BLOG_CONFIG), blog_toml).with_context(|| {
		format!(
			"init: failed to write {}",
			location.join(BLOG_CONFIG).display()
		)
	})?;

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

fn write_workflow(location: &Path) -> Result<()> {
	let workflow_dir = location.join(".github/workflows");
	fs::create_dir_all(&workflow_dir)
		.with_context(|| format!("init: failed to create {}", workflow_dir.display()))?;

	let workflow = PAGES_WORKFLOW
		.replace("__BD_VERSION__", BD_VERSION)
		.replace("__BD_REPO__", BD_REPO);
	let workflow_path = workflow_dir.join("pages.yml");
	fs::write(&workflow_path, workflow)
		.with_context(|| format!("init: failed to write {}", workflow_path.display()))?;

	Ok(())
}

fn write_css_setting(location: &Path, css: CssFramework) -> Result<()> {
	let path = location.join(BLOG_CONFIG);
	let text = fs::read_to_string(&path)
		.with_context(|| format!("init: failed to read {}", path.display()))?;
	let mut document = text
		.parse::<DocumentMut>()
		.with_context(|| format!("init: failed to parse {}", path.display()))?;
	document
		.as_table_mut()
		.insert("css", toml_edit::value(css.as_str()));
	fs::write(&path, document.to_string())
		.with_context(|| format!("init: failed to write {}", path.display()))?;

	Ok(())
}

fn write_template_setting(location: &Path, template: ThemeName) -> Result<()> {
	let path = location.join(BLOG_CONFIG);
	let text = fs::read_to_string(&path)
		.with_context(|| format!("init: failed to read {}", path.display()))?;
	let mut document = text
		.parse::<DocumentMut>()
		.with_context(|| format!("init: failed to parse {}", path.display()))?;
	document
		.as_table_mut()
		.insert("template", toml_edit::value(template.as_str()));

	fs::write(&path, document.to_string())
		.with_context(|| format!("init: failed to write {}", path.display()))?;
	Ok(())
}

fn write_templates(location: &Path, theme: ThemeName) -> Result<()> {
	let templates_dir = location.join("templates");
	fs::create_dir_all(&templates_dir)
		.with_context(|| format!("init: failed to create {}", templates_dir.display()))?;
	for (name, contents) in theme.theme().templates() {
		let path = templates_dir.join(name);
		fs::write(&path, contents)
			.with_context(|| format!("init: failed to write {}", path.display()))?;
	}

	Ok(())
}

fn write_static(location: &Path, theme: ThemeName) -> Result<()> {
	let static_dir = location.join("static");
	fs::create_dir_all(&static_dir)
		.with_context(|| format!("init: failed to create {}", static_dir.display()))?;

	let style_path = static_dir.join("style.css");
	fs::write(&style_path, theme.theme().style_css)
		.with_context(|| format!("init: failed to write {}", style_path.display()))?;

	Ok(())
}

fn write_readme(location: &Path, blog: &BlogConfig, force: bool) -> Result<()> {
	let readme_path = location.join("README.md");
	if !force && readme_path.exists() {
		return Ok(());
	}

	let readme = README_TEMPLATE.replace("__BLOG_TITLE__", &blog.title);
	fs::write(&readme_path, readme)
		.with_context(|| format!("init: failed to write: {}", readme_path.display()))?;

	Ok(())
}

fn write_about(location: &Path, force: bool) -> Result<()> {
	let about_path = location.join("about.md");
	if !force && about_path.exists() {
		return Ok(());
	}

	fs::write(&about_path, ABOUT)
		.with_context(|| format!("init: failed to write {}", about_path.display()))?;

	Ok(())
}

fn write_default_page(location: &Path) -> Result<()> {
	let page_dir = location.join(DEFAULT_PAGE);
	fs::create_dir_all(&page_dir)
		.with_context(|| format!("init: failed to create {}", page_dir.display()))?;

	let page = PageFile {
		title: "My First Page".to_string(),
		description: "My first post.".to_string(),
		date: format_date(today())?,
		draft: false,
		tags: Vec::new(),
		image: String::new(),
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

	Ok(())
}

fn upgrade_scaffolding(
	location: &Path,
	css: Option<CssFramework>,
	template: Option<ThemeName>,
) -> Result<()> {
	if !location.join(BLOG_CONFIG).is_file() {
		bail!(
			"init --upgrade: {} does not contain {BLOG_CONFIG}; run `bd init {}` first",
			location.display(),
			location.display()
		);
	}

	let config = BlogConfig::load(location)?;
	let theme = template.unwrap_or(config.template);

	if let Some(template) = template {
		write_template_setting(location, template)?;
	}

	if let Some(css) = css {
		write_css_setting(location, css)?;
	}

	write_templates(location, theme)?;
	write_static(location, theme)?;
	write_workflow(location)?;

	println!(
		"Upgrade templates/, static/, and .github/workflows/pages.yml in {} (theme: {})",
		location.display(),
		theme.as_str()
	);
	Ok(())
}

pub fn apply_theme(location: &Path, theme: ThemeName) -> Result<()> {
	if !location.join(BLOG_CONFIG).is_file() {
		bail!(
			"apply_theme: {} does not contain {BLOG_CONFIG}",
			location.display()
		);
	}

	write_template_setting(location, theme)?;
	write_templates(location, theme)?;
	write_static(location, theme)?;
	Ok(())
}

fn refuse_existing(location: &Path) -> Result<()> {
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

	if location.join("templates").exists() {
		bail!("init: {} already contains templates", location.display());
	}

	if location.join("static").exists() {
		bail!("init: {} already contains static", location.display());
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn embedded_about_is_not_empty() {
		assert!(!ABOUT.trim().is_empty());
	}

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
	fn embedded_readme_has_title_placeholder() {
		assert!(!README_TEMPLATE.trim().is_empty());
		assert!(README_TEMPLATE.contains("__BLOG_TITLE__"));
	}

	#[test]
	fn embedded_templates_are_not_empty() {
		for (name, contents) in ThemeName::Classic.theme().templates() {
			assert!(!contents.trim().is_empty(), "template {name} is empty");
		}
	}

	#[test]
	fn embedded_style_is_not_empty() {
		assert!(!ThemeName::Classic.theme().style_css.trim().is_empty());
	}

	#[test]
	fn run_refuses_existing_blog() {
		let temp = tempfile::tempdir().unwrap();
		fs::write(temp.path().join(BLOG_CONFIG), "title = \"Existing\"\n").unwrap();

		let error = run(temp.path(), false, false, None, None)
			.unwrap_err()
			.to_string();

		assert!(error.contains("already contains blog.toml"), "{error}");
	}

	#[test]
	fn run_refuses_existing_templates() {
		let temp = tempfile::tempdir().unwrap();
		fs::create_dir(temp.path().join("templates")).unwrap();

		let error = run(temp.path(), false, false, None, None)
			.unwrap_err()
			.to_string();

		assert!(error.contains("already contains templates"), "{error}");
	}

	#[test]
	fn run_refuses_existing_static() {
		let temp = tempfile::tempdir().unwrap();
		fs::create_dir(temp.path().join("static")).unwrap();

		let error = run(temp.path(), false, false, None, None)
			.unwrap_err()
			.to_string();

		assert!(error.contains("already contains static"), "{error}");
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

	#[test]
	fn run_upgrade_requires_blog_config() {
		let temp = tempfile::tempdir().unwrap();

		let error = run(temp.path(), false, true, None, None)
			.unwrap_err()
			.to_string();

		assert!(error.contains("does not contain blog.toml"), "{error}");
	}

	#[test]
	fn run_force_resets_generated_files_and_keeps_extras() {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path();

		fs::create_dir(root.join(".git")).unwrap();
		fs::write(root.join(BLOG_CONFIG), "title = \"Custom\"\n").unwrap();
		fs::write(root.join("README.md"), "# Custom\n").unwrap();
		fs::write(root.join("about.md"), "# Custom\n").unwrap();
		fs::write(root.join(".gitignore"), "custom-ignore\n").unwrap();

		fs::create_dir_all(root.join("templates")).unwrap();
		fs::write(root.join("templates/base.html"), "old").unwrap();
		fs::write(root.join("templates/custom.html"), "keep").unwrap();

		fs::create_dir_all(root.join("static")).unwrap();
		fs::write(root.join("static/style.html"), "old").unwrap();
		fs::write(root.join("static/CNAME"), "example.com\n").unwrap();

		fs::create_dir_all(root.join(DEFAULT_PAGE)).unwrap();
		fs::write(root.join(DEFAULT_PAGE).join("page.md"), "old").unwrap();
		fs::write(root.join(DEFAULT_PAGE).join("extra.txt"), "keep").unwrap();

		fs::create_dir_all(root.join("notes")).unwrap();
		fs::write(root.join("notes/keep.txt"), "keep").unwrap();

		run(root, true, false, None, None).unwrap();

		let config: BlogConfig =
			toml::from_str(&fs::read_to_string(root.join(BLOG_CONFIG)).unwrap()).unwrap();
		assert_eq!(config.title, "Default Blog");
		assert!(
			fs::read_to_string(root.join("README.md"))
				.unwrap()
				.contains("# Default Blog")
		);
		assert!(
			fs::read_to_string(root.join("about.md"))
				.unwrap()
				.contains("Write about")
		);

		assert!(
			fs::read_to_string(root.join("templates/base.html"))
				.unwrap()
				.contains("{{ static_root }}style.css")
		);
		assert!(
			fs::read_to_string(root.join("static/style.css"))
				.unwrap()
				.contains("Site-wide")
		);
		assert!(
			fs::read_to_string(root.join(DEFAULT_PAGE).join("page.md"))
				.unwrap()
				.contains("Write your post here")
		);

		let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
		assert!(gitignore.contains("custom-ignore"));
		assert!(gitignore.contains("/build"));

		assert!(root.join("templates/custom.html").is_file());
		assert!(root.join("static/CNAME").is_file());
		assert!(root.join(DEFAULT_PAGE).join("extra.txt").is_file());
		assert!(root.join("notes/keep.txt").is_file());
	}

	#[test]
	fn write_css_setting_preserves_comments() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join(BLOG_CONFIG);
		fs::write(&path, "# Keep this comment!\ntitle = \"Blog\"\n").unwrap();

		write_css_setting(temp.path(), CssFramework::Tailwind).unwrap();

		let contents = fs::read_to_string(&path).unwrap();
		assert!(contents.contains("# Keep this comment!"));
		assert!(contents.contains("css = \"tailwind\""));
	}

	#[test]
	fn write_template_setting_preserves_comments() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join(BLOG_CONFIG);
		fs::write(&path, "# Keep this comment!\ntitle = \"Blog\"\n").unwrap();

		write_template_setting(temp.path(), ThemeName::Personal).unwrap();

		let contents = fs::read_to_string(&path).unwrap();
		assert!(contents.contains("# Keep this comment!"));
		assert!(contents.contains("template = \"personal\""));
	}
}
