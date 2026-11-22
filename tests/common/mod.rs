#![allow(dead_code)]

use std::path::{Path, PathBuf};

use assert_cmd::Command;

pub fn bd() -> Command {
	Command::cargo_bin("bd").expect("bd binary should be built")
}

pub fn bd_in(dir: &Path) -> Command {
	let mut cmd = bd();
	cmd.current_dir(dir);
	cmd
}

pub fn init_blog(dir: &Path) {
	bd().arg("init").arg(dir).assert().success();
}

pub fn read(path: impl AsRef<Path>) -> String {
	let path = path.as_ref();
	std::fs::read_to_string(path)
		.unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

pub fn read_bytes(path: impl AsRef<Path>) -> Vec<u8> {
	let path = path.as_ref();
	std::fs::read(path).unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) {
	let path = path.as_ref();
	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent)
			.unwrap_or_else(|error| panic!("failed to create {}: {error}", path.display()));
	}
	std::fs::write(path, contents)
		.unwrap_or_else(|error| panic!("failed to create {}: {error}", path.display()));
}

pub fn page_toml(blog: &Path, slug: &str) -> PathBuf {
	blog.join(slug).join("page.toml")
}

pub fn md(blog: &Path, slug: &str) -> PathBuf {
	blog.join(slug).join("page.md")
}

pub fn assert_contains(sum: &str, part: &str) {
	assert!(sum.contains(part), "expected to find {part:?} in: {sum}");
}

pub fn assert_not_contains(sum: &str, part: &str) {
	assert!(
		!sum.contains(part),
		"expected to NOT find {part:?} in: {sum}"
	);
}

pub fn assert_date(value: &str) {
	let bytes = value.as_bytes();

	assert_eq!(bytes.len(), 10, "date should be YYYY-MM-DD: {value:?}");
	assert_eq!(bytes[4], b'-', "date should be YYYY-MM-DD: {value:?}");
	assert_eq!(bytes[7], b'-', "date should be YYYY-MM-DD: {value:?}");
	assert!(
		bytes[..4].iter().all(|b| b.is_ascii_digit()),
		"year should be digits: {value:?}"
	);
	assert!(
		bytes[5..7].iter().all(|b| b.is_ascii_digit()),
		"month should be digits: {value:?}"
	);
	assert!(
		bytes[8..].iter().all(|b| b.is_ascii_digit()),
		"day should be digits: {value:?}"
	);
}
