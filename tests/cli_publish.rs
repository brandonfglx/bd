mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use predicates::prelude::*;
use tempfile::TempDir;

use common::{bd_in, init_blog, write};

fn git(dir: &Path, args: &[&str]) {
	let output = Command::new("git")
		.args(args)
		.current_dir(dir)
		.output()
		.unwrap_or_else(|error| panic!("failed to run git {args:?}: {error}"));
	assert!(
		output.status.success(),
		"git {args:?} failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
}

fn git_output(dir: &Path, args: &[&str]) -> String {
	let output = Command::new("git")
		.args(args)
		.current_dir(dir)
		.output()
		.unwrap_or_else(|error| panic!("failed to run git {args:?}: {error}"));
	assert!(
		output.status.success(),
		"git {args:?} failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8_lossy(&output.stdout).into_owned()
}

fn configure_user(blog: &Path) {
	git(blog, &["config", "user.name", "Test User"]);
	git(blog, &["config", "user.email", "test@example.com"]);
}

fn bare_remote(temp: &TempDir) -> PathBuf {
	let remote = temp.path().join("remote.git");
	git(
		temp.path(),
		&["init", "--bare", "-b", "main", remote.to_str().unwrap()],
	);
	remote
}

fn add_remote(blog: &Path, remote: &Path) {
	git(blog, &["remote", "add", "origin", remote.to_str().unwrap()]);
}

fn bd_publish(blog: &Path, args: &[&str]) -> assert_cmd::Command {
	let mut command = bd_in(blog);
	command.env("GIT_TERMINAL_PROMPT", "0");
	command.arg("publish");
	command.args(args);
	command
}

#[test]
fn publish_commits_and_pushes_all_pages() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	configure_user(&blog);
	let remote = bare_remote(&temp);
	add_remote(&blog, &remote);

	bd_publish(&blog, &[])
		.assert()
		.success()
		.stdout(predicate::str::contains("Published all pages."));

	assert_eq!(
		git_output(&blog, &["log", "-1", "--pretty=%s", "main"]).trim(),
		"Publish: all pages"
	);
	assert_eq!(
		git_output(&remote, &["log", "-1", "--pretty=%s", "main"]).trim(),
		"Publish: all pages"
	);
	assert!(git_output(&remote, &["ls-tree", "--name-only", "main"]).contains("my-page"));
}

#[test]
fn publish_page_only_commits_that_page() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	configure_user(&blog);
	let remote = bare_remote(&temp);
	add_remote(&blog, &remote);

	write(blog.join("my-page/page.md"), "# Updated\n");
	write(blog.join("notes.txt"), "keep me local");

	bd_publish(&blog, &["--page", "my-page"])
		.assert()
		.success()
		.stdout(predicate::str::contains("Published page \"my-page\"."));

	let files = git_output(&blog, &["show", "--name-only", "--pretty=format:", "HEAD"]);
	assert!(files.contains("my-page/page.md"), "{files}");
	assert!(!files.contains("notes.txt"), "{files}");
	assert!(!files.contains("blog.toml"), "{files}");
}

#[test]
fn publish_rejects_missing_page() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	configure_user(&blog);

	bd_publish(&blog, &["--page", "missing"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("page not found: missing"));
}

#[test]
fn publish_rejects_non_page_directory() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	configure_user(&blog);
	write(blog.join("notes/note.txt"), "hello");

	bd_publish(&blog, &["--page", "notes"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("is not a bd page"));
}

#[test]
fn publish_page_requires_a_value() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	bd_in(&blog).args(["publish", "--page"]).assert().failure();
}

#[test]
fn publish_without_remote_fails_when_not_interactive() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	configure_user(&blog);
	write(blog.join("my-page/page.md"), "# Updated\n");

	bd_publish(&blog, &[])
		.write_stdin("")
		.assert()
		.failure()
		.stderr(predicate::str::contains("no git remote is configured"));
}

#[test]
fn publish_again_pushes_without_a_new_commit() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	configure_user(&blog);
	let remote = bare_remote(&temp);
	add_remote(&blog, &remote);

	bd_publish(&blog, &[]).assert().success();
	let first = git_output(&blog, &["rev-parse", "HEAD"]);

	bd_publish(&blog, &[])
		.assert()
		.success()
		.stdout(predicate::str::contains("No changes to commit."));

	let second = git_output(&blog, &["rev-parse", "HEAD"]);
	assert_eq!(first, second);
}
