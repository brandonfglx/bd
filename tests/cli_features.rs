mod common;

use std::fs;
use std::path::Path;

use predicates::prelude::*;
use tempfile::TempDir;

use common::{assert_contains, bd_in, init_blog, read, write};

fn add_tagged(blog: &Path, title: &str, slug: &str, date: &str, tags: &[&str]) {
	let mut cmd = bd_in(blog);
	cmd.args(["add", title, "--slug", slug, "--date", date]);
	for tag in tags {
		cmd.args(["--tag", tag]);
	}
	cmd.assert().success();
}

fn add_raw_tags(blog: &Path, title: &str, slug: &str, date: &str, tags: &[&str]) {
	bd_in(blog)
		.args(["add", title, "--slug", slug, "--date", date])
		.assert()
		.success();

	let path = blog.join(slug).join("page.toml");
	let mut meta = read(&path);
	let values = tags
		.iter()
		.map(|tag| format!("{tag:?}"))
		.collect::<Vec<_>>()
		.join(", ");
	meta.push_str(&format!("tags=[{values}]\n"));
	write(&path, meta);
}

fn assert_all(html: &str, needles: &[&str]) {
	for needle in needles {
		assert_contains(html, needle);
	}
}

#[test]
fn build_renders_tags_search_and_about() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_tagged(&blog, "Post", "post", "2024-01-01", &["Post", "Posts"]);
	write(blog.join("about.md"), "# About Me\n");

	bd_in(&blog).arg("build").assert().success();

	let tags = read(blog.join("build/tags/index.html"));
	assert_contains(&tags, "Post");
	assert_contains(&tags, "Posts");

	let post = read(blog.join("build/tags/post/index.html"));
	assert_contains(&post, "Post");
	assert_contains(&post, "1 post");

	let post = read(blog.join("build/post/index.html"));
	assert_contains(&post, "tags/post/");

	let search: serde_json::Value =
		serde_json::from_str(&read(blog.join("build/search.json"))).unwrap();
	let posts = search["posts"].as_array().unwrap();
	let post_post = posts
		.iter()
		.find(|post| post["slug"].as_str() == Some("post"))
		.expect("post should be in search.json");
	assert_eq!(post_post["title"].as_str(), Some("Post"));
	assert_eq!(post_post["tags"][0]["name"].as_str(), Some("Post"));

	let about = read(blog.join("build/about/index.html"));
	assert_contains(&about, "<h1>About Me</h1>");
}

#[test]
fn future_posts_are_built_but_hidden_from_listings_and_search() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_tagged(
		&blog,
		"Future Post",
		"future-post",
		"2999-01-01",
		&["Future Tag"],
	);
	bd_in(&blog).arg("build").assert().success();

	let index = read(blog.join("build/index.html"));
	assert_contains(&index, "href=\"tags/\"");
	assert_contains(&read(blog.join("build/tags/index.html")), "No tags yet");
	let tag_page = read(blog.join("build/tags/future-tag/index.html"));
	assert_contains(&tag_page, "0 posts");
	assert_contains(&tag_page, "No posts with this tag");

	assert!(blog.join("build/future-post/index.html").is_file());
	assert!(!read(blog.join("build/index.html")).contains("Future Post"));
	assert!(!read(blog.join("build/search.json")).contains("future-post"));
	assert!(!read(blog.join("build/tags/index.html")).contains("Future Tag"));
	assert!(blog.join("build/tags/future-tag/index.html").is_file());
}

#[test]
fn build_rejects_tag_slug_collisions() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_tagged(&blog, "One", "one", "2024-01-01", &["Rust"]);
	add_tagged(&blog, "Two", "two", "2024-01-02", &["rust"]);

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("both use"));
}

#[test]
fn build_rejects_windows_reserved_tag_slugs() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_raw_tags(&blog, "Reserved", "reserved", "2024-01-01", &["con"]);

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("invalid tag"));
}

#[test]
fn build_rejects_static_conflicts_with_generated_tags_and_about() {
	for (rel, message, tagged) in [
		("about/file.txt", "generated about directory", false),
		("tags/file.txt", "generated tags directory", true),
		("search.json", "generated file", false),
	] {
		let temp = TempDir::new().unwrap();
		let blog = temp.path().join("blog");
		init_blog(&blog);
		if tagged {
			add_tagged(&blog, "Tagged", "tagged", "2024-01-01", &["Rust"]);
		}
		write(blog.join("static").join(rel), "x");

		bd_in(&blog)
			.arg("build")
			.assert()
			.failure()
			.stderr(predicate::str::contains(message));
	}
}

#[cfg(unix)]
#[test]
fn build_rejects_symlinked_about_page() {
	use std::os::unix::fs::symlink;

	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	let outside = temp.path().join("outside.md");
	write(&outside, "# Outside\n");
	fs::remove_file(blog.join("about.md")).unwrap();
	symlink(&outside, blog.join("about.md")).unwrap();

	bd_in(&blog)
		.arg("build")
		.assert()
		.failure()
		.stderr(predicate::str::contains("cannot use symlink as about page"));
}

#[test]
fn build_uses_correct_relative_navigation() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_tagged(&blog, "Post", "post", "2024-01-01", &["Rust"]);
	write(blog.join("about.md"), "# About\n");
	bd_in(&blog).arg("build").assert().success();

	let index = read(blog.join("build/index.html"));
	assert_all(
		&index,
		&[
			"href=\"tags/\"",
			"href=\"about/\"",
			"href=\"./style.css\"",
			"data-index=\"./search.json\"",
			"data-home=\"./\"",
		],
	);

	let post = read(blog.join("build/post/index.html"));
	assert_all(
		&post,
		&[
			"href=\"../tags/\"",
			"href=\"../about/\"",
			"href=\"../style.css\"",
			"data-index=\"../search.json\"",
			"data-home=\"../\"",
		],
	);

	let tags = read(blog.join("build/tags/index.html"));
	assert_all(
		&tags,
		&[
			"href=\"rust/\"",
			"href=\"../about/\"",
			"href=\"../style.css\"",
			"data-index=\"../search.json\"",
			"data-home=\"../\"",
		],
	);

	let tag = read(blog.join("build/tags/rust/index.html"));
	assert_all(
		&tag,
		&[
			"href=\"../\"",
			"href=\"../../\"",
			"href=\"../../about/\"",
			"href=\"../../post/\"",
			"href=\"../../tags/rust/\"",
			"href=\"../../style.css\"",
			"data-index=\"../../search.json\"",
			"data-home=\"../../\"",
		],
	);

	let about = read(blog.join("build/about/index.html"));
	assert_all(
		&about,
		&[
			"href=\"../tags/\"",
			"href=\"../\"",
			"href=\"./\"",
			"href=\"../style.css\"",
			"data-index=\"../search.json\"",
			"data-home=\"../\"",
		],
	);
}

#[test]
fn tags_render_below_date_on_index_and_post() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	add_tagged(&blog, "Post", "post", "2024-01-01", &["Rust"]);

	let meta_path = blog.join("post/page.toml");
	let meta = read(&meta_path).replace("description = \"\"", "description = \"A description\"");
	write(&meta_path, meta);

	bd_in(&blog).arg("build").assert().success();

	for path in ["build/index.html", "build/post/index.html"] {
		let html = read(blog.join(path));
		let date = html.find("<time").expect("date should be rendered");
		let description = html
			.rfind("A description")
			.expect("description should be rendered");
		let tags = html
			.find("class=\"post-tags\"")
			.expect("tags should be rendered");
		println!("{date} {description} {tags}");
		assert!(
			date < description,
			"{path}: description should follow the date"
		);
		assert!(
			description < tags,
			"{path}: description should precede the tags"
		);
	}
}

#[test]
fn rm_tag_updates_build_output() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	add_tagged(&blog, "Post", "post", "2024-01-01", &["Rust", "Web Dev"]);

	bd_in(&blog).arg("build").assert().success();
	assert!(blog.join("build/tags/rust/index.html").is_file());
	assert!(blog.join("build/tags/web-dev/index.html").is_file());

	bd_in(&blog)
		.args(["rm", "--tag", "Rust", "post"])
		.assert()
		.success();

	bd_in(&blog).arg("build").assert().success();
	assert!(!blog.join("build/tags/rust").exists());
	assert!(blog.join("build/tags/web-dev/index.html").is_file());

	let post = read(blog.join("build/post/index.html"));
	assert!(!post.contains("../tags/rust/"));
	assert_contains(&post, "../tags/web-dev/");
}
