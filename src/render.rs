use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use minijinja::value::{Serde, Value};
use minijinja::{Environment, context};
use serde::Serialize;
use time::OffsetDateTime;
use walkdir::WalkDir;

use crate::config::BlogConfig;
use crate::page::Page;

pub(crate) const BASE_TEMPLATE: &str = include_str!("templates/base.html");
pub(crate) const INDEX_TEMPLATE: &str = include_str!("templates/index.html");
pub(crate) const POST_TEMPLATE: &str = include_str!("templates/post.html");

pub struct Renderer {
	env: Environment<'static>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PostView<'a> {
	pub slug: &'a str,
	pub title: &'a str,
	pub description: &'a str,
	pub date: &'a str,
	pub date_display: &'a str,
	pub draft: bool,
}

impl<'a> From<&'a Page> for PostView<'a> {
	fn from(page: &'a Page) -> Self {
		Self {
			slug: &page.slug,
			title: &page.title,
			description: &page.description,
			date: &page.date_str,
			date_display: &page.date_display,
			draft: page.draft,
		}
	}
}

impl Renderer {
	pub fn new() -> Result<Self> {
		let mut env = Environment::new();
		env.add_template("base.html", BASE_TEMPLATE)
			.context("failed to load base.html template")?;
		env.add_template("index.html", INDEX_TEMPLATE)
			.context("failed to load index.html template")?;
		env.add_template("post.html", POST_TEMPLATE)
			.context("failed to load post.html template")?;

		Ok(Self { env })
	}

	pub fn with_root(root: &Path) -> Result<Self> {
		let mut renderer = Self::new()?;
		load_custom_templates(&mut renderer.env, root)?;
		Ok(renderer)
	}

	pub fn index(&self, blog: &BlogConfig, posts: &[PostView<'_>]) -> Result<String> {
		let year = OffsetDateTime::now_utc().year();
		self.env
			.get_template("index.html")
			.context("failed to find index.html template")?
			.render(context! {
				blog => Serde(blog),
				posts => Serde(posts),
				year => year,
				home_url => "./",
				static_root => Value::from_safe_string("./".to_string()),
			})
			.context("failed to render index.html template")
	}

	pub fn post(&self, blog: &BlogConfig, post: &PostView<'_>, content: &str) -> Result<String> {
		let year = OffsetDateTime::now_utc().year();
		self.env
			.get_template("post.html")
			.context("failed to find post.html template")?
			.render(context! {
				blog => Serde(blog),
				post => Serde(post),
				content => content,
				year => year,
				home_url => "../",
				static_root => Value::from_safe_string("../".to_string())
			})
			.context("failed to render post.html template")
	}
}

fn load_custom_templates(env: &mut Environment<'static>, root: &Path) -> Result<()> {
	let dir = root.join("templates");
	let metadata = match fs::symlink_metadata(&dir) {
		Ok(metadata) => metadata,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
		Err(error) => {
			return Err(error).with_context(|| format!("failed to inspect {}", dir.display()));
		}
	};

	if metadata.file_type().is_symlink() {
		bail!(
			"cannot use symlink for templates directory: {}",
			dir.display()
		);
	}

	if !metadata.is_dir() {
		bail!("templates path is not a directory: {}", dir.display());
	}

	for entry in WalkDir::new(&dir).follow_links(false).sort_by_file_name() {
		let entry = entry.with_context(|| format!("failed to walk {}", dir.display()))?;
		if entry.file_type().is_symlink() || !entry.file_type().is_file() {
			continue;
		}

		let rel = entry
			.path()
			.strip_prefix(&dir)
			.with_context(|| format!("failed to relativeize {}", entry.path().display()))?;
		let name = rel.to_string_lossy().replace('\\', "/");
		if name.split('/').any(|part| part.starts_with('.')) {
			continue;
		}

		if !is_template_file(rel) {
			continue;
		}

		let source = fs::read_to_string(entry.path())
			.with_context(|| format!("failed to read template {}", entry.path().display()))?;
		env.add_template_owned(name, source)
			.with_context(|| format!("failed to parse template {}", entry.path().display()))?;
	}

	Ok(())
}

fn is_template_file(path: &Path) -> bool {
	let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
		return false;
	};

	matches!(extension, "html" | "htm" | "jinja" | "j2" | "txt" | "xml")
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::BuildConfig;

	fn blog() -> BlogConfig {
		BlogConfig {
			title: "My Blog".to_string(),
			description: "A description".to_string(),
			author: "Ada".to_string(),
			language: "en".to_string(),
			build: BuildConfig::default(),
		}
	}

	#[test]
	fn renders_index_with_posts() {
		let renderer = Renderer::new().unwrap();
		let posts = [
			PostView {
				slug: "newer",
				title: "Newer",
				description: "Fresh",
				date: "2024-02-01",
				date_display: "February 1, 2024",
				draft: false,
			},
			PostView {
				slug: "older",
				title: "Older",
				description: "",
				date: "2024-01-01",
				date_display: "January 1, 2024",
				draft: false,
			},
		];

		let html = renderer.index(&blog(), &posts).unwrap();

		assert!(html.starts_with("<!DOCTYPE html>"));
		assert!(html.contains("<html lang=\"en\">"));
		assert!(html.contains("<title>My Blog</title>"));
		assert!(html.contains("<meta name=\"description\" content=\"A description\">"));
		assert!(html.contains("<a href=\"./\">My Blog</a>"));
		assert!(html.contains("<link rel=\"stylesheet\" href=\"./style.css\">"));
		assert!(html.contains("<a href=\"newer/\">Newer</a>"));
		assert!(html.contains("<a href=\"older/\">Older</a>"));
		assert!(html.contains("<time datetime=\"2024-02-01\">February 1, 2024</time>"));
		assert!(html.contains("Fresh"));
		assert!(html.contains("&copy;"));
		assert!(html.contains("Ada"));
	}

	#[test]
	fn renders_empty_index() {
		let renderer = Renderer::new().unwrap();
		let html = renderer.index(&blog(), &[]).unwrap();

		assert!(html.contains("No posts yet."));
		assert!(!html.contains("<ul class=\"posts\">"));
	}

	#[test]
	fn escapes_index_values() {
		let renderer = Renderer::new().unwrap();
		let mut config = blog();
		config.title = "A & B <Blog>".to_string();
		let posts = [PostView {
			slug: "xss",
			title: "<script>alert(1)</script>",
			description: "\"quoted\" & more",
			date: "2024-01-01",
			date_display: "January 1, 2024",
			draft: false,
		}];

		let html = renderer.index(&config, &posts).unwrap();

		assert!(html.contains("A &amp; B &lt;Blog&gt;"));
		assert!(html.contains("&lt;script&gt;alert(1)&lt;&#x2f;script&gt;"));
		assert!(!html.contains("<script>alert(1)</script>"));
		assert!(html.contains("&quot;quoted&quot; &amp; more"));
	}

	#[test]
	fn renders_post_with_content_and_draft_badge() {
		let renderer = Renderer::new().unwrap();
		let post = PostView {
			slug: "hello",
			title: "Hello",
			description: "About hello",
			date: "2024-03-04",
			date_display: "March 4, 2024",
			draft: true,
		};

		let html = renderer.post(&blog(), &post, "<p>Body</p>").unwrap();

		assert!(html.contains("<title>Hello - My Blog</title>"));
		assert!(html.contains("<h1>Hello</h1>"));
		assert!(html.contains("<meta name=\"description\" content=\"About hello\">"));
		assert!(html.contains("<strong>Draft</strong>"));
		assert!(html.contains("<div class=\"content\">"));
		assert!(html.contains("<p>Body</p>"));
		assert!(html.contains("<link rel=\"stylesheet\" href=\"../style.css\">"));
		assert!(html.contains("<a href=\"../\">&larr; All posts</a>"));
	}

	#[test]
	fn does_not_mark_published_post_as_draft() {
		let renderer = Renderer::new().unwrap();
		let post = PostView {
			slug: "hello",
			title: "Hello",
			description: "",
			date: "2024-03-04",
			date_display: "March 4, 2024",
			draft: false,
		};

		let html = renderer.post(&blog(), &post, "").unwrap();

		assert!(!html.contains("<strong>draft</strong>"));
		assert!(!html.contains("<meta name=\"description\""));
	}

	#[test]
	fn post_content_is_not_escaped() {
		let renderer = Renderer::new().unwrap();
		let post = PostView {
			slug: "raw",
			title: "Raw",
			description: "",
			date: "2024-01-01",
			date_display: "January 1, 2024",
			draft: false,
		};

		let html = renderer
			.post(&blog(), &post, "<span id=\"raw\">raw</span>")
			.unwrap();

		assert!(html.contains("<span id=\"raw\">raw</span>"));
	}

	#[test]
	fn escapes_post_title_and_description() {
		let renderer = Renderer::new().unwrap();
		let post = PostView {
			slug: "xss",
			title: "A & B",
			description: "<tag>",
			date: "2024-01-01",
			date_display: "January 1, 2024",
			draft: false,
		};

		let html = renderer.post(&blog(), &post, "").unwrap();

		assert!(html.contains("<title>A &amp; B - My Blog</title>"));
		assert!(html.contains("<h1>A &amp; B</h1>"));
		assert!(html.contains("content=\"&lt;tag&gt;\""));
		assert!(!html.contains("<tag>"));
	}

	#[test]
	fn loads_custom_templates_and_partials() {
		let temp = tempfile::tempdir().unwrap();
		let templates = temp.path().join("templates");
		fs::create_dir(&templates).unwrap();
		fs::write(templates.join("partial.html"), "PARTIAL").unwrap();
		fs::write(
			templates.join("index.html"),
			"{% include \"partial.html\" %}:{{ blog.title }}:{{ static_root }}style.css",
		)
		.unwrap();

		let renderer = Renderer::with_root(temp.path()).unwrap();
		let html = renderer.index(&blog(), &[]).unwrap();

		assert_eq!(html, "PARTIAL:My Blog:./style.css");
	}

	#[test]
	fn falls_back_to_embedded_template() {
		let temp = tempfile::tempdir().unwrap();
		let templates = temp.path().join("templates");
		fs::create_dir(&templates).unwrap();
		fs::write(templates.join("index.html"), "CUSTOM").unwrap();

		let renderer = Renderer::with_root(temp.path()).unwrap();
		let post = PostView {
			slug: "hello",
			title: "Hello",
			description: "",
			date: "2024-01-01",
			date_display: "January 1, 2024",
			draft: false,
		};
		let html = renderer.post(&blog(), &post, "").unwrap();

		assert!(html.contains("All posts"));
		assert!(!html.contains("CUSTOM"));
	}

	#[test]
	fn rejects_invalid_custom_template() {
		let temp = tempfile::tempdir().unwrap();
		let templates = temp.path().join("templates");
		fs::create_dir(&templates).unwrap();
		fs::write(templates.join("index.html"), "{% if %}").unwrap();

		let error = match Renderer::with_root(temp.path()) {
			Ok(_) => panic!("expected error"),
			Err(error) => error.to_string(),
		};

		assert!(error.contains("failed to parse template"), "{error}");
	}

	#[cfg(unix)]
	#[test]
	fn rejects_symlink_templates_directory() {
		use std::os::unix::fs::symlink;

		let temp = tempfile::tempdir().unwrap();
		let outside = temp.path().join("outside");
		fs::create_dir(&outside).unwrap();
		symlink(&outside, temp.path().join("templates")).unwrap();

		let error = match Renderer::with_root(temp.path()) {
			Ok(_) => panic!("expected error"),
			Err(error) => error.to_string(),
		};

		assert!(error.contains("cannot use symlink"), "{error}");
	}
}
