mod common;

use predicates::prelude::*;
use tempfile::TempDir;

use common::{assert_contains, assert_not_contains, bd, bd_in, init_blog, read, write};

#[test]
fn init_scaffolds_selected_theme() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");

	bd().args(["init", "--template", "grid"])
		.arg(&blog)
		.assert()
		.success();

	let config: toml::Value = toml::from_str(&read(blog.join("blog.toml"))).unwrap();
	assert_eq!(config["template"].as_str(), Some("grid"));
	assert_contains(&read(blog.join("templates/index.html")), "post-grid");
}

#[test]
fn init_upgrade_switches_theme_and_preserves_comments() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(
		blog.join("blog.toml"),
		"# Keep this comment!\ntitle = \"Custom\"\n",
	);

	bd_in(&blog)
		.args(["init", "--upgrade", "--template", "personal", "."])
		.assert()
		.success();

	let config_text = read(blog.join("blog.toml"));
	assert_contains(&config_text, "# Keep this comment!");
	assert_contains(&config_text, "template = \"personal\"");
	assert_contains(&read(blog.join("templates/index.html")), "hero");
}

#[test]
fn init_rejects_invalid_theme() {
	let temp = TempDir::new().unwrap();

	bd().args(["init", "--template", "neon"])
		.arg(temp.path().join("blog"))
		.assert()
		.failure()
		.stderr(predicate::str::contains("invalid value"));
}

#[test]
fn grid_images_resolve_on_index_tag_and_post() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");

	bd().args(["init", "--template", "grid"])
		.arg(&blog)
		.assert()
		.success();

	bd_in(&blog)
		.args([
			"add",
			"Covered",
			"--slug",
			"covered",
			"--date",
			"2024-01-01",
			"--tag",
			"Rust",
		])
		.assert()
		.success();

	write(blog.join("covered/cover.jpg"), b"image");
	let meta = read(blog.join("covered/page.toml")).replace(
		"content = \"page.md\"",
		"content = \"page.md\"\nimage = \"cover.jpg\"",
	);
	write(blog.join("covered/page.toml"), meta);

	bd_in(&blog).arg("build").assert().success();

	let index = read(blog.join("build/index.html"));
	assert_contains(&index, "src=\"covered/cover.jpg\"");

	let tag = read(blog.join("build/tags/rust/index.html"));
	assert_contains(&tag, "src=\"../../covered/cover.jpg\"");

	let post = read(blog.join("build/covered/index.html"));
	assert_contains(&post, "src=\"cover.jpg\"");
}

#[test]
fn not_found_uses_root_relative_assets() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog).arg("build").assert().success();

	let html = read(blog.join("build/404.html"));
	assert_contains(&html, "href=\"/style.css\"");
	assert_contains(&html, "data-index=\"/search.json\"");
	assert_contains(&html, "data-home=\"/\"");
}

#[test]
fn static_404_overrides_generated_page() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(blog.join("static/404.html"), "CUSTOM 404\n");
	bd_in(&blog).arg("build").assert().success();

	assert_eq!(read(blog.join("build/404.html")), "CUSTOM 404\n");
}

#[test]
fn init_template_upgrades_existing_files() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(
		blog.join("blog.toml"),
		"# Keep this comment!\ntitle = \"Custom\"\ntemplate = \"classic\"\n",
	);
	write(blog.join("templates/index.html"), "outdated\n");
	write(blog.join("static/style.css"), "outdated\n");
	write(blog.join(".github/workflows/pages.yml"), "name: outdated\n");

	bd_in(&blog)
		.args(["init", "--template", "grid", "."])
		.assert()
		.success();

	let config_text = read(blog.join("blog.toml"));
	assert_contains(&config_text, "# Keep this comment!");
	assert_contains(&config_text, "template = \"grid\"");
	assert_contains(&read(blog.join("templates/index.html")), "post-grid");
	assert_contains(&read(blog.join("static/style.css")), "Site-wide");
	assert_contains(
		&read(blog.join(".github/workflows/pages.yml")),
		"brandonfglx/bd",
	);
}

#[test]
fn init_css_upgrades_existing_files() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(
		blog.join("blog.toml"),
		"# Keep this comment!\ntitle = \"Custom\"\ncss = \"manual\"\n",
	);
	write(blog.join("templates/base.html"), "outdated\n");
	write(blog.join("static/style.css"), "outdated\n");

	bd_in(&blog)
		.args(["init", "--css", "tailwind", "."])
		.assert()
		.success();

	let config_text = read(blog.join("blog.toml"));
	assert_contains(&config_text, "# Keep this comment!");
	assert_contains(&config_text, "css = \"tailwind\"");
	let base = read(blog.join("templates/base.html"));
	assert_contains(&base, "@tailwindcss/browser@4");
	assert_contains(&base, ".content h1");
	assert_contains(&read(blog.join("static/style.css")), "Site-wide");
}

#[test]
fn tailwind_build_includes_markdown_typography() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(
		blog.join("blog.toml"),
		"title = \"Default Blog\"\ncss = \"tailwind\"\n",
	);
	write(
		blog.join("my-page/page.md"),
		"# Heading 1\n\n## Heading 2\n\n- one\n- two\n",
	);

	bd_in(&blog).arg("build").assert().success();

	let post = read(blog.join("build/my-page/index.html"));
	assert_contains(&post, ".content h1");
	assert_contains(&post, "class=\"content\"");
	assert_not_contains(&post, "prose prose-neutral");
}

#[test]
fn build_falls_back_to_configured_theme() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");

	bd().args(["init", "--template", "grid"])
		.arg(&blog)
		.assert()
		.success();
	std::fs::remove_file(blog.join("templates/index.html")).unwrap();

	bd_in(&blog).arg("build").assert().success();

	assert_contains(&read(blog.join("build/index.html")), "post-grid");
}
