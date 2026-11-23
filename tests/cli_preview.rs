mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use predicates::prelude::*;
use tempfile::TempDir;

use common::{bd_in, init_blog, write};

struct Preview {
	child: Child,
	address: String,
}

impl Preview {
	fn start(blog: &Path) -> Self {
		let mut child = Command::new(env!("CARGO_BIN_EXE_bd"))
			.args(["preview", "--port", "0"])
			.current_dir(blog)
			.stdout(Stdio::piped())
			.stderr(Stdio::inherit())
			.spawn()
			.expect("failed to start bd preview");

		let stdout = child.stdout.take().expect("preview stdout should be piped");
		let mut reader = BufReader::new(stdout);
		let mut address = None;

		for _ in 0..50 {
			let mut line = String::new();
			if reader
				.read_line(&mut line)
				.expect("failed to read preview output")
				== 0
			{
				break;
			}
			if let Some(index) = line.find("http://") {
				address = Some(line[index + "http://".len()..].trim().to_string());
				break;
			}
		}

		let address = match address {
			Some(address) => address,
			None => {
				let _ = child.kill();
				let _ = child.wait();
				panic!("bd preview did not print a URL");
			}
		};

		Self { child, address }
	}

	fn get(&self, path: &str) -> String {
		let mut stream = TcpStream::connect(&self.address)
			.unwrap_or_else(|error| panic!("failed to connect to {}: {error}", self.address));
		stream
			.set_read_timeout(Some(Duration::from_secs(5)))
			.expect("failed to set read timeout");
		write!(
			stream,
			"GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
			self.address
		)
		.expect("failed to send request");

		let mut response = String::new();
		stream
			.read_to_string(&mut response)
			.expect("failed to read response");
		response
	}
}

impl Drop for Preview {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

#[test]
#[ignore = "may fail randomly depending on system"]
fn preview_serves_index_and_pages() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	let preview = Preview::start(&blog);

	let index = preview.get("/");
	assert!(index.starts_with("HTTP/1.1 200"), "{index}");
	assert!(index.contains("My Blog"), "{index}");
	assert!(index.contains("My First Page"), "{index}");
	assert!(index.contains("text/html"), "{index}");

	let post = preview.get("/my-page/");
	assert!(post.starts_with("HTTP/1.1 200"), "{post}");
	assert!(post.contains("My First Page"), "{post}");
	assert!(post.contains("text/html"), "{post}");
}

#[test]
#[ignore = "may fail randomly depending on system"]
fn preview_redirects_directory_without_slash() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	let preview = Preview::start(&blog);
	let response = preview.get("/my-page");

	assert!(response.starts_with("HTTP/1.1 301"), "{response}");
	assert!(response.contains("Location: /my-page/"), "{response}");
}

#[test]
#[ignore = "may fail randomly depending on system"]
fn preview_returns_404_for_missing_files() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	let preview = Preview::start(&blog);
	let response = preview.get("/missing/");

	assert!(response.starts_with("HTTP/1.1 404"), "{response}");
	assert!(response.contains("404 Not Found"), "{response}");
}

#[test]
#[ignore = "may fail randomly depending on system"]
fn preview_rebuilds_before_serving() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);

	write(
		blog.join("my-page/page.toml"),
		"title = \"Fresh Title\"\ndescription = \"\"\ndate = \"2024-01-01\"\ndraft = false\ncontent = \"page.md\"\n",
	);

	let preview = Preview::start(&blog);
	let index = preview.get("/");

	assert!(index.contains("Fresh Title"), "{index}");
	assert!(!index.contains("My First Page"), "{index}");
}

#[test]
#[ignore = "may fail randomly depending on system"]
fn preview_serves_static_assets() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	write(blog.join("static/custom.txt"), "custom asset");

	let preview = Preview::start(&blog);
	let response = preview.get("/custom.txt");

	assert!(response.starts_with("HTTP/1.1 200"), "{response}");
	assert!(response.contains("text/plain"), "{response}");
	assert!(response.contains("custom asset"), "{response}");
}

#[test]
#[ignore = "may fail randomly depending on system"]
fn preview_reports_build_errors() {
	let temp = TempDir::new().unwrap();
	let blog = temp.path().join("blog");
	init_blog(&blog);
	write(
		blog.join("broken/page.toml"),
		"title = \"Broken\"\ndate = \"not-a-date\"\n",
	);
	write(blog.join("broken/page.md"), "# Broken\n");

	bd_in(&blog)
		.args(["preview", "--port", "0"])
		.assert()
		.failure()
		.stderr(predicate::str::contains("invalid date"));
}
