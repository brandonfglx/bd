mod common;

use std::fs;
use std::path::Path;

use predicates::prelude::*;
use tempfile::TempDir;

use common::{assert_contains, bd_in, init_blog, read, write};

fn add_ok(blog: &Path, args: &[&str]) {
	bd_in(blog).arg("add").args(args).assert().success();
}

#[test]
fn add_creates_page_from_title() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(&blog, &["Hello, World!", "--date", "2000-04-01"]);

	let meta_path = blog.join("hello-world/page.toml");
	assert!(meta_path.is_file());
	assert!(blog.join("hello-world/page.md").is_file());

	let meta: toml::Value =
		toml::from_str(&read(&meta_path)).expect("page.toml should be valid TOML");

	assert_eq!(
		meta.get("title").and_then(|value| value.as_str()),
		Some("Hello, World!")
	);
	assert_eq!(
		meta.get("date").and_then(|value| value.as_str()),
		Some("2000-04-01")
	);
	assert_eq!(
		meta.get("draft").and_then(|value| value.as_bool()),
		Some(false)
	);
	assert_eq!(
		meta.get("content").and_then(|value| value.as_str()),
		Some("page.md")
	);
	assert!(!read(blog.join("hello-world/page.md")).trim().is_empty());
}

#[test]
fn add_supports_custom_slug_date_and_draft() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(
		&blog,
		&[
			"Custom",
			"--slug",
			"custom-slug",
			"--date",
			"1955-07-17",
			"--draft",
		],
	);

	let meta: toml::Value =
		toml::from_str(&read(blog.join("custom-slug/page.toml"))).expect("valid page.toml");

	assert_eq!(
		meta.get("title").and_then(|value| value.as_str()),
		Some("Custom")
	);
	assert_eq!(
		meta.get("date").and_then(|value| value.as_str()),
		Some("1955-07-17")
	);
	assert_eq!(
		meta.get("draft").and_then(|value| value.as_bool()),
		Some(true)
	);
}

#[test]
fn add_rejects_duplicate_slug() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(&blog, &["First", "--slug", "same"]);

	bd_in(&blog)
		.args(["add", "Second", "--slug", "same"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("already exists"));

	assert_contains(&read(blog.join("same/page.toml")), "title = \"First\"");
}

#[test]
fn add_rejects_invalid_date() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args([
			"add",
			"Bad date",
			"--slug",
			"bad-date",
			"--date",
			"2024-13-01",
		])
		.assert()
		.failure()
		.stderr(predicate::str::contains("invalid --date"));

	assert!(!blog.join("bad-date").exists());
}

#[test]
fn add_rejects_invalid_slugs() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	for slug in [
		"",
		"Build",
		"my_slug",
		"my page",
		"trailing-",
		"../escape",
		"a/b",
	] {
		bd_in(&blog)
			.args(["add", "Title", "--slug", slug, "--date", "2024-01-01"])
			.assert()
			.failure();
	}

	assert!(!blog.join("Build").exists());
	assert!(!blog.join("my_slug").exists());
	assert!(!blog.join("my page").exists());
}

#[test]
fn add_rejects_reserved_slugs() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	// `com6` is included intentionally; add it to `RESERVED` in src/slug.rs.
	for slug in ["build", "con", "com1", "com6", "lpt9"] {
		bd_in(&blog)
			.args(["add", "Reserved", "--slug", slug, "--date", "2024-01-01"])
			.assert()
			.failure()
			.stderr(predicate::str::contains("reserved"));
	}
}

#[test]
fn add_rejects_long_slug() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	let slug = "a".repeat(81);

	bd_in(&blog)
		.args([
			"add",
			"Long",
			"--slug",
			slug.as_str(),
			"--date",
			"2024-01-01",
		])
		.assert()
		.failure()
		.stderr(predicate::str::contains("at most 80 characters"));
}

#[test]
fn add_rejects_empty_title() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["add", "   "])
		.assert()
		.failure()
		.stderr(predicate::str::contains("title cannot be empty"));
}

#[test]
fn add_outside_blog_fails() {
	let temp = TempDir::new().unwrap();

	bd_in(temp.path())
		.args(["add", "No blog", "--date", "2024-01-01"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("No blog.toml found"));
}

#[test]
fn add_from_nested_directory() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	let nested = blog.join("notes");
	fs::create_dir(&nested).unwrap();

	bd_in(&nested)
		.args(["add", "Nested post", "--date", "2024-03-04"])
		.assert()
		.success();

	assert!(blog.join("nested-post/page.toml").is_file());
	assert!(!nested.join("nested-post").exists());
}

#[test]
fn rm_force_removes_page() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["rm", "my-page", "--force"])
		.assert()
		.success()
		.stdout(predicate::str::contains("Removed page \"my-page\""));

	assert!(!blog.join("my-page").exists());
}

#[test]
fn rm_without_force_non_tty_fails() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["rm", "my-page"])
		.write_stdin("")
		.assert()
		.failure()
		.stderr(predicate::str::contains(
			"refusing to delete without --force",
		));

	assert!(blog.join("my-page/page.toml").is_file());
}

#[test]
fn rm_rejects_missing_page() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["rm", "missing", "--force"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("page not found"));
}

#[test]
fn rm_rejects_non_page_directory() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	write(blog.join("notes/note.txt"), "hello");

	bd_in(&blog)
		.args(["rm", "notes", "--force"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("is not a bd page"));
}

#[test]
fn rm_rejects_path_traversal() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	for slug in ["../escape", "a/b", "..", "."] {
		bd_in(&blog)
			.args(["rm", slug, "--force"])
			.assert()
			.failure();
	}

	assert!(blog.join("my-page/page.toml").is_file());
}

#[test]
fn rm_from_nested_directory() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	let nested = blog.join("notes");
	fs::create_dir(&nested).unwrap();

	bd_in(&nested)
		.args(["rm", "my-page", "--force"])
		.assert()
		.success();

	assert!(!blog.join("my-page").exists());
}

#[cfg(unix)]
#[test]
fn rm_refuses_symlink() {
	use std::os::unix::fs::symlink;

	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	symlink(blog.join("my-page"), blog.join("linked-page")).unwrap();

	bd_in(&blog)
		.args(["rm", "linked-page", "--force"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("refusing to remove symlink"));

	assert!(blog.join("my-page/page.toml").is_file());
}
