mod common;

use predicates::prelude::*;
use tempfile::TempDir;

use common::{assert_contains, bd, bd_in, init_blog, read, write};

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
