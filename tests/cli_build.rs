mod common;

use std::fs;
use std::path::Path;

use predicates::prelude::*;
use tempfile::TempDir;

use common::{assert_contains, assert_not_contains, bd_in, init_blog, read, read_bytes, write};

fn add_ok(blog: &Path, title: &str, slug: &str, date: &str) {
	bd_in(blog)
		.args(["add", title, "--slug", slug, "--date", date])
		.assert()
		.success();
}

fn build_ok(blog: &Path, args: &[&str]) {
	bd_in(blog).arg("build").args(args).assert().success();
}

#[test]
fn build_renders_index_and_posts() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(&blog, "First Post", "first-post", "2024-01-01");
	add_ok(&blog, "Second Post", "second-post", "2024-02-01");
	write(
		blog.join("first-post/page.md"),
		"# Heading\n\nThis is **bold** and *italic*.\n\n- one\n- two\n\n| A | B |\n|---|---|\n| 1 | 2 |\n",
	);

	bd_in(&blog)
		.arg("build")
		.assert()
		.success()
		.stdout(predicate::str::contains("Built 3 pages"));

	let index = read(blog.join("build/index.html"));
	assert_contains(&index, "Default Blog");
	assert_contains(&index, "First Post");
	assert_contains(&index, "Second Post");
	assert_contains(&index, "second-post/");
	assert_contains(&index, "first-post/");
	assert!(
		index.find("Second Post").unwrap() < index.find("First Post").unwrap(),
		"index should list newer posts first"
	);

	let post = read(blog.join("build/first-post/index.html"));
	assert_contains(&post, "<h1>Heading</h1>");
	assert_contains(&post, "<strong>bold</strong>");
	assert_contains(&post, "<em>italic</em>");
	assert_contains(&post, "<li>one</li>");
	assert_contains(&post, "<table>");
	assert_contains(&post, "First Post");
	assert_contains(&post, "2024-01-01");
	assert_contains(&post, "All posts");

	assert!(blog.join("build/.nojekyll").is_file());
}

#[test]
fn build_skips_drafts_by_default() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args([
			"add",
			"Draft Post",
			"--slug",
			"draft-post",
			"--date",
			"2024-03-01",
			"--draft",
		])
		.assert()
		.success();

	bd_in(&blog)
		.arg("build")
		.assert()
		.success()
		.stdout(predicate::str::contains("Skipped 1 draft"));

	assert!(!blog.join("build/draft-post").exists());
	assert_not_contains(&read(blog.join("build/index.html")), "Draft Post");

	build_ok(&blog, &["--drafts"]);

	assert!(blog.join("build/draft-post/index.html").is_file());
	assert_contains(&read(blog.join("build/index.html")), "Draft Post");
}

#[test]
fn build_honors_include_drafts_config() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args([
			"add",
			"Config Draft",
			"--slug",
			"config-draft",
			"--date",
			"2024-04-01",
			"--draft",
		])
		.assert()
		.success();

	let config_path = blog.join("blog.toml");
	let config = read(&config_path).replace("include_drafts = false", "include_drafts = true");
	write(&config_path, config);

	build_ok(&blog, &[]);

	assert!(blog.join("build/config-draft/index.html").is_file());
	assert_contains(&read(blog.join("build/index.html")), "Config Draft");
}

#[test]
fn build_honors_output_override() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	build_ok(&blog, &["--output", "public"]);

	assert!(blog.join("public/index.html").is_file());
	assert!(!blog.join("build").exists());
}

#[test]
fn build_honors_output_config() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	let config_path = blog.join("blog.toml");
	let config = read(&config_path).replace("output = \"build\"", "output = \"site\"");
	write(&config_path, config);

	build_ok(&blog, &[]);

	assert!(blog.join("site/index.html").is_file());
	assert!(!blog.join("build").exists());
}

#[test]
fn build_rejects_output_outside_root() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["build", "--output", "../outside"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("must not contain '.' or '..'"));

	assert!(!temp.path().join("outside").exists());
}

#[test]
fn build_rejects_absolute_output() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	let absolute = temp.path().join("absolute-output");

	bd_in(&blog)
		.args(["build", "--output", absolute.to_str().unwrap()])
		.assert()
		.failure()
		.stderr(predicate::str::contains(
			"must be relative to the blog root",
		));

	assert!(!absolute.exists());
}

#[test]
fn build_rejects_output_equal_to_page() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["build", "--output", "my-page"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("is a page directory"));
}

#[test]
fn build_rejects_output_inside_page() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["build", "--output", "my-page/site"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("is inside page directory"));
}

#[test]
fn build_rejects_hidden_output() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["build", "--output", ".site"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("hidden path components"));
}

#[test]
fn build_rejects_output_that_is_a_file() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	write(blog.join("output-file"), "not a directory");

	bd_in(&blog)
		.args(["build", "--output", "output-file"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("not a directory"));
}

#[test]
fn build_copies_assets() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(&blog, "Assets", "assets", "2024-05-01");
	write(blog.join("assets/photo.bin"), vec![0u8, 1, 2, 3]);
	write(blog.join("assets/files/note.txt"), "hello");
	write(blog.join("assets/.DS_Store"), "ignored");

	build_ok(&blog, &[]);

	assert_eq!(
		read_bytes(blog.join("build/assets/photo.bin")),
		vec![0, 1, 2, 3]
	);
	assert_eq!(read(blog.join("build/assets/files/note.txt")), "hello");
	assert!(!blog.join("build/assets/.DS_Store").exists());
	assert!(!blog.join("build/assets/page.toml").exists());
	assert!(!blog.join("build/assets/page.md").exists());
}

#[test]
fn build_rejects_asset_index_html() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(&blog, "Conflict", "conflict", "2024-06-01");
	write(blog.join("conflict/index.html"), "custom");

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("conflicts with generated page"));
}

#[test]
fn build_fails_on_page_md_without_page_toml() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	write(blog.join("orphan/page.md"), "# Orphan\n");

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("found page.md but no page.toml"));
}

#[test]
fn build_fails_on_invalid_page_toml() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	write(
		blog.join("broken/page.toml"),
		"title = \"Broken\"\ndate = \"not-a-date\"\n",
	);
	write(blog.join("broken/page.md"), "# Broken\n");

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("invalid date"));
}

#[test]
fn build_rebuild_removes_stale_output() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(&blog, "Temporary", "temporary", "2024-07-01");
	build_ok(&blog, &[]);
	assert!(blog.join("build/temporary/index.html").is_file());

	bd_in(&blog)
		.args(["rm", "temporary", "--force"])
		.assert()
		.success();

	build_ok(&blog, &[]);
	assert!(!blog.join("build/temporary").exists());
}

#[test]
fn build_empty_blog_renders_no_posts() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["rm", "my-page", "--force"])
		.assert()
		.success();

	bd_in(&blog)
		.arg("build")
		.assert()
		.success()
		.stdout(predicate::str::contains("Built 0 pages"));

	assert_contains(&read(blog.join("build/index.html")), "No posts yet.");
}

#[test]
fn build_escapes_titles_and_preserves_markdown_html() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(&blog, "<script>alert(1)</script>", "xss", "2024-08-01");
	write(
		blog.join("xss/page.md"),
		"<span id=\"raw\">raw html</span>\n",
	);

	build_ok(&blog, &[]);

	let index = read(blog.join("build/index.html"));
	assert_contains(&index, "&lt;script&gt;alert(1)&lt;&#x2f;script&gt;");
	assert_not_contains(&index, "<script>alert(1)</script>");

	let post = read(blog.join("build/xss/index.html"));
	assert_contains(&post, "<span id=\"raw\">raw html</span>");
}

#[test]
fn build_supports_custom_content_file() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(&blog, "Custom Content", "custom-content", "2024-09-01");
	fs::rename(
		blog.join("custom-content/page.md"),
		blog.join("custom-content/post.md"),
	)
	.unwrap();

	let meta_path = blog.join("custom-content/page.toml");
	let meta = read(&meta_path).replace("content = \"page.md\"", "content = \"post.md\"");
	write(&meta_path, meta);
	write(
		blog.join("custom-content/post.md"),
		"# Custom\n\nFrom post.md\n",
	);

	build_ok(&blog, &[]);

	let post = read(blog.join("build/custom-content/index.html"));
	assert_contains(&post, "<h1>Custom</h1>");
	assert_contains(&post, "From post.md");
	assert!(!blog.join("build/custom-content/post.md").exists());
}

#[test]
fn build_from_nested_directory() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	let nested = blog.join("notes");
	fs::create_dir(&nested).unwrap();

	bd_in(&nested).arg("build").assert().success();

	assert!(blog.join("build/index.html").is_file());
	assert!(!nested.join("build").exists());
}

#[test]
fn build_same_date_sorted_by_slug() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(&blog, "Zulu", "zulu", "2024-10-01");
	add_ok(&blog, "Alpha", "alpha", "2024-10-01");

	build_ok(&blog, &[]);

	let index = read(blog.join("build/index.html"));
	assert!(index.find("Alpha").unwrap() < index.find("Zulu").unwrap());
}

#[test]
fn build_verbose_reports_assets() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(&blog, "Verbose", "verbose", "2024-11-01");
	write(blog.join("verbose/file.txt"), "x");

	bd_in(&blog)
		.args(["build", "--verbose"])
		.assert()
		.success()
		.stdout(predicate::str::contains("verbose (1 asset)"));
}

#[test]
fn build_rejects_output_equal_to_root() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["build", "--output", "."])
		.assert()
		.failure()
		.stderr(predicate::str::contains("must not contain '.' or '..'"));
}

#[test]
fn build_rejects_empty_output() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["build", "--output", ""])
		.assert()
		.failure();
}

#[test]
fn build_preserves_old_output_on_render_error() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(&blog, "Stable", "stable", "2024-12-01");
	build_ok(&blog, &[]);
	let old_index = read(blog.join("build/index.html"));

	write(
		blog.join("broken/page.toml"),
		"title = \"Broken\"\ndate = \"bad\"\n",
	);
	write(blog.join("broken/page.md"), "# Broken\n");

	bd_in(&blog).arg("build").assert().failure();

	assert_eq!(read(blog.join("build/index.html")), old_index);
	assert!(blog.join("build/stable/index.html").is_file());
}

#[test]
fn build_copies_static_files() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(blog.join("static/style.css"), "body { color: red; }\n");
	write(blog.join("static/robots.txt"), "User-agent: *\n");
	write(blog.join("static/CNAME"), "example.com\n");
	write(blog.join("static/nested/note.txt"), "hello");
	write(blog.join("static/.gitkeep"), "");
	write(blog.join("static/.DS_Store"), "ignored");

	build_ok(&blog, &[]);

	assert_eq!(read(blog.join("build/style.css")), "body { color: red; }\n");
	assert_eq!(read(blog.join("build/robots.txt")), "User-agent: *\n");
	assert_eq!(read(blog.join("build/CNAME")), "example.com\n");
	assert_eq!(read(blog.join("build/nested/note.txt")), "hello");
	assert!(!blog.join("build/.gitkeep").exists());
	assert!(!blog.join("build/.DS_Store").exists());
}

#[test]
fn build_static_rejects_index_conflict() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(blog.join("static/index.html"), "custom");

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("conflicts with a generated file"));
}

#[test]
fn build_static_rejects_page_conflict() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	add_ok(&blog, "Static Conflict", "static-conflict", "2024-01-01");

	write(blog.join("static/static-conflict/file.txt"), "x");

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("conflicts with page directory"));
}

#[test]
fn build_ignores_page_md_in_static() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(blog.join("static/page.md"), "# Static\n");

	build_ok(&blog, &[]);

	assert_eq!(read(blog.join("build/page.md")), "# Static\n");
}

#[test]
fn build_uses_custom_templates() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(blog.join("templates/index.html"), "CUSTOM INDEX");
	write(
		blog.join("templates/post.html"),
		"CUSTOM POST: {{ post.title }}",
	);

	build_ok(&blog, &[]);

	assert_eq!(read(blog.join("build/index.html")), "CUSTOM INDEX");
	assert_eq!(
		read(blog.join("build/my-page/index.html")),
		"CUSTOM POST: My First Page"
	);
}

#[test]
fn build_uses_custom_partial() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(blog.join("templates/partial.html"), "PARTIAL");
	write(
		blog.join("templates/index.html"),
		"{% include \"partial.html\" %}",
	);

	build_ok(&blog, &[]);

	assert_eq!(read(blog.join("build/index.html")), "PARTIAL");
}

#[test]
fn build_falls_back_to_embedded_template() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	fs::remove_file(blog.join("templates/post.html")).unwrap();

	build_ok(&blog, &[]);

	let post = read(blog.join("build/my-page/index.html"));
	assert_contains(&post, "All posts");
	assert_contains(&post, "My First Page");
}

#[test]
fn build_rejects_invalid_custom_template() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(blog.join("templates/index.html"), "{% if %}");

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("failed to parse template"));
}

#[test]
fn build_rejects_output_inside_static() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["build", "--output", "static/site"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("inside the static directory"));
}

#[test]
fn build_rejects_output_inside_templates() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["build", "--output", "templates/site"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("inside the templates directory"));
}

#[test]
fn build_writes_output_marker() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	build_ok(&blog, &[]);

	let marker = read(blog.join("build/.bd-output"));
	assert_contains(&marker, "bd ");
}

#[test]
fn build_replaces_marked_output() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	build_ok(&blog, &[]);
	write(blog.join("build/stale.txt"), "stale");

	build_ok(&blog, &[]);

	assert!(!blog.join("build/stale.txt").exists());
	assert!(blog.join("build/index.html").is_file());
}

#[test]
fn build_refuses_unmarked_output() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(blog.join("build/keep.txt"), "keep");

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("does not contain .bd-output"))
		.stderr(predicate::str::contains("--force"));

	assert_eq!(read(blog.join("build/keep.txt")), "keep");
}

#[test]
fn build_force_replaces_unmarked_output() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(blog.join("build/keep.txt"), "keep");

	build_ok(&blog, &["--force"]);

	assert!(!blog.join("build/keep.txt").exists());
	assert!(blog.join("build/.bd-output").is_file());
}

#[cfg(unix)]
#[test]
fn build_rejects_symlink_output() {
	use std::os::unix::fs::symlink;

	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	let target = blog.join("target");
	fs::create_dir(&target).unwrap();
	symlink(&target, blog.join("build")).unwrap();

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("cannot use symlink"));

	assert!(target.is_dir());
	assert!(!target.join("index.html").exists());
}

#[test]
fn build_static_rejects_marker_conflict() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(blog.join("static/.bd-output"), "custom");

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("conflicts with a generated file"));
}

#[test]
fn build_preserves_output_when_staging_fails() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	build_ok(&blog, &[]);
	let old_index = read(blog.join("build/index.html"));

	write(blog.join("static/index.html"), "custom");

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("conflicts with a generated file"));

	assert_eq!(read(blog.join("build/index.html")), old_index);
	assert!(blog.join("build/.bd-output").is_file());

	let leftovers: Vec<_> = fs::read_dir(&blog)
		.unwrap()
		.filter_map(|entry| entry.ok())
		.map(|entry| entry.file_name().to_string_lossy().into_owned())
		.filter(|name| name.starts_with(".bd-build-") || name.starts_with(".bd-old-"))
		.collect();
	assert!(
		leftovers.is_empty(),
		"leftover temporary dirs: {leftovers:?}"
	);
}
