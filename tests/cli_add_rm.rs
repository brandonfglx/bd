mod common;

use std::fs;
use std::path::Path;

use predicates::prelude::*;
use tempfile::TempDir;
use time::OffsetDateTime;
use time::macros::format_description;

use common::{assert_contains, assert_date, bd_in, init_blog, read, write};

fn add_ok(blog: &Path, args: &[&str]) {
	bd_in(blog).arg("add").args(args).assert().success();
}

fn local_date() -> String {
	let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
	now.date()
		.format(format_description!("[year]-[month]-[day]"))
		.unwrap()
}

fn tags_of(blog: &Path, slug: &str) -> Vec<String> {
	let meta: toml::Value = toml::from_str(&read(common::page_toml(blog, slug)))
		.expect("page.toml should be valid TOML");
	meta.get("tags")
		.and_then(|value| value.as_array())
		.map(|tags| {
			tags.iter()
				.filter_map(|tag| tag.as_str().map(str::to_string))
				.collect()
		})
		.unwrap_or_default()
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
fn add_uses_local_date_and_reports_timezone() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	let before = local_date();
	bd_in(&blog)
		.args(["add", "Local Time"])
		.assert()
		.success()
		.stdout(predicate::str::contains("UTC"));
	let after = local_date();

	let meta: toml::Value =
		toml::from_str(&read(blog.join("local-time/page.toml"))).expect("valid page.toml");
	let date = meta["date"].as_str().expect("date should be a string");

	assert_date(date);
	assert!(
		date == before.as_str() || date == after.as_str(),
		"date {date} should be the local date ({before} or {after})"
	);
}

#[test]
fn add_reports_explicit_date_without_timezone() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["add", "Explicit Date", "--date", "2000-04-01"])
		.assert()
		.success()
		.stdout(predicate::str::contains("date 2000-04-01"))
		.stdout(predicate::str::contains("UTC").not());
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

#[test]
fn add_supports_tags() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(
		&blog,
		&[
			"Tagged",
			"--slug",
			"tagged",
			"--date",
			"2024-01-01",
			"--tag",
			"Web Dev",
			"--tag",
			"Rust",
		],
	);

	assert_eq!(tags_of(&blog, "tagged"), ["Rust", "Web Dev"]);
}

#[test]
fn add_deduplicates_tags() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(
		&blog,
		&[
			"Duplicate",
			"--date",
			"2024-01-01",
			"--tag",
			"Rust",
			"--tag",
			" Rust ",
		],
	);

	assert_eq!(tags_of(&blog, "duplicate"), ["Rust"]);
}

#[test]
fn add_rejects_invalid_tags() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog)
		.args(["add", "Empty", "--date", "2024-01-01", "--tag", " "])
		.assert()
		.failure()
		.stderr(predicate::str::contains("tag cannot be empty"));
	assert!(!blog.join("empty").exists());

	bd_in(&blog)
		.args(["add", "Reserved", "--date", "2024-01-01", "--tag", "con"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("invalid tag"));
	assert!(!blog.join("reserved").exists());

	bd_in(&blog)
		.args([
			"add",
			"Collision",
			"--date",
			"2024-01-01",
			"--tag",
			"Rust",
			"--tag",
			"rust",
		])
		.assert()
		.failure()
		.stderr(predicate::str::contains("both use the slug"));
	assert!(!blog.join("collision").exists());
}

#[test]
fn add_tags_to_existing_page() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(
		&blog,
		&[
			"Existing",
			"--slug",
			"existing",
			"--date",
			"2024-01-01",
			"--tag",
			"Rust",
		],
	);
	add_ok(
		&blog,
		&["Different Title", "--slug", "existing", "--tag", "Web Dev"],
	);

	assert_eq!(tags_of(&blog, "existing"), ["Rust", "Web Dev"]);

	let meta: toml::Value =
		toml::from_str(&read(blog.join("existing/page.toml"))).expect("valid page.toml");
	assert_eq!(
		meta.get("title").and_then(|value| value.as_str()),
		Some("Existing")
	);
}

#[test]
fn add_existing_tag_is_noop() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(
		&blog,
		&[
			"Existing",
			"--slug",
			"existing",
			"--date",
			"2024-01-01",
			"--tag",
			"Rust",
		],
	);
	let before = read(blog.join("existing/page.toml"));

	bd_in(&blog)
		.args(["add", "Existing", "--slug", "existing", "--tag", "Rust"])
		.assert()
		.success()
		.stdout(predicate::str::contains("already has the requested tags"));

	assert_eq!(read(blog.join("existing/page.toml")), before);
}

#[test]
fn add_tags_to_existing_page_reflects_date_and_draft() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(
		&blog,
		&["Existing", "--slug", "existing", "--date", "2024-01-01"],
	);

	for args in [
		vec![
			"add",
			"Existing",
			"--slug",
			"existing",
			"--tag",
			"Rust",
			"--date",
			"2025-01-01",
		],
		vec![
			"add", "Existing", "--slug", "existing", "--tag", "Rust", "--draft",
		],
	] {
		bd_in(&blog)
			.args(args)
			.assert()
			.failure()
			.stderr(predicate::str::contains("cannot be used when adding tags"));
	}
}

#[test]
fn rm_tag_removes_only_requested_tags() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(
		&blog,
		&[
			"Tagged",
			"--slug",
			"tagged",
			"--date",
			"2024-01-01",
			"--tag",
			"Rust",
			"--tag",
			"Web Dev",
			"--tag",
			"CLI",
		],
	);
	let page_md = read(blog.join("tagged/page.md"));

	bd_in(&blog)
		.args(["rm", "--tag", "Rust", "tagged"])
		.assert()
		.success()
		.stdout(predicate::str::contains("Removed 1 tag"));

	assert_eq!(tags_of(&blog, "tagged"), ["CLI", "Web Dev"]);
	assert_eq!(read(blog.join("tagged/page.md")), page_md);
	assert!(blog.join("tagged/page.toml").is_file());
}

#[test]
fn rm_tag_removes_all_tags_and_omits_field() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(
		&blog,
		&[
			"Tagged",
			"--slug",
			"tagged",
			"--date",
			"2024-01-01",
			"--tag",
			"Rust",
			"--tag",
			"CLI",
		],
	);

	bd_in(&blog)
		.args(["rm", "--tag", "rust", "--tag", "cli", "tagged"])
		.assert()
		.success();

	assert!(tags_of(&blog, "tagged").is_empty());
	assert!(!read(blog.join("tagged/page.toml")).contains("tags ="));
	assert!(blog.join("tagged/page.md").is_file());
}

#[test]
fn rm_tag_requires_matching_tag_and_non_empty() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(
		&blog,
		&[
			"Tagged",
			"--slug",
			"tagged",
			"--date",
			"2024-01-01",
			"--tag",
			"Rust",
		],
	);

	bd_in(&blog)
		.args(["rm", "--tag", "Missing", "tagged"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("none of the requested tags"));

	bd_in(&blog)
		.args(["rm", "--tag", "  ", "tagged"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("tag cannot be empty"));

	assert!(blog.join("tagged/page.toml").is_file());
}

#[test]
fn rm_tag_does_not_require_force() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	add_ok(
		&blog,
		&[
			"Tagged",
			"--slug",
			"tagged",
			"--date",
			"2024-01-01",
			"--tag",
			"Rust",
		],
	);

	bd_in(&blog)
		.args(["rm", "--tag", "Rust", "tagged"])
		.write_stdin("")
		.assert()
		.success();

	assert!(blog.join("tagged/page.toml").is_file());
}

#[test]
fn rm_tag_can_remove_invalid_existing_tag() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	add_ok(
		&blog,
		&["Reserved", "--slug", "reserved", "--date", "2024-01-01"],
	);

	let meta_path = blog.join("reserved/page.toml");
	let mut meta = read(&meta_path);
	meta.push_str("tags = [\"con\"]\n");
	write(&meta_path, meta);

	bd_in(&blog)
		.args(["rm", "--tag", "con", "reserved"])
		.assert()
		.success();

	assert!(tags_of(&blog, "reserved").is_empty());
	bd_in(&blog).arg("build").assert().success();
}

#[test]
fn rm_tag_rejects_non_page_directory() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	write(blog.join("notes/note.txt"), "hello");

	bd_in(&blog)
		.args(["rm", "--tag", "Rust", "notes"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("is not a bd page"));
}
