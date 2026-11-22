use anyhow::{Context, Result};
use minijinja::value::Serde;
use minijinja::{Environment, context};
use serde::Serialize;
use time::OffsetDateTime;

use crate::config::BlogConfig;
use crate::page::Page;

pub struct Renderer {
	env: Environment<'static>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PostView<'a> {
	pub slug: &'a str,
	pub title: &'a str,
	pub description: &'a str,
	pub date: &'a str,
	pub draft: bool,
}

impl<'a> From<&'a Page> for PostView<'a> {
	fn from(page: &'a Page) -> Self {
		Self {
			slug: &page.slug,
			title: &page.title,
			description: &page.description,
			date: &page.date_str,
			draft: page.draft,
		}
	}
}

impl Renderer {
	pub fn new() -> Result<Self> {
		let mut env = Environment::new();
		env.add_template("base.html", include_str!("templates/base.html"))
			.context("failed to load base.html template")?;
		env.add_template("index.html", include_str!("templates/index.html"))
			.context("failed to load index.html template")?;
		env.add_template("post.html", include_str!("templates/post.html"))
			.context("failed to load post.html template")?;

		Ok(Self { env })
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
			})
			.context("failed to render post.html template")
	}
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
				draft: false,
			},
			PostView {
				slug: "older",
				title: "Older",
				description: "",
				date: "2024-01-01",
				draft: false,
			},
		];

		let html = renderer.index(&blog(), &posts).unwrap();

		assert!(html.starts_with("<!DOCTYPE html>"));
		assert!(html.contains("<html lang=\"en\">"));
		assert!(html.contains("<title>My Blog</title>"));
		assert!(html.contains("<meta name=\"description\" content=\"A description\">"));
		assert!(html.contains("<a href=\"./\">My Blog</a>"));
		assert!(html.contains("<a href=\"newer/\">Newer</a>"));
		assert!(html.contains("<a href=\"older/\">Older</a>"));
		assert!(html.contains("<time datetime=\"2024-02-01\">2024-02-01</time>"));
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
			draft: true,
		};

		let html = renderer.post(&blog(), &post, "<p>Body</p>").unwrap();

		assert!(html.contains("<title>Hello &mdash; My Blog</title>"));
		assert!(html.contains("<h1>Hello</h1>"));
		assert!(html.contains("<meta name=\"description\" content=\"About hello\">"));
		assert!(html.contains("<strong>draft</strong>"));
		assert!(html.contains("<div class=\"content\">"));
		assert!(html.contains("<p>Body</p>"));
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
			draft: false,
		};

		let html = renderer.post(&blog(), &post, "").unwrap();

		assert!(html.contains("<title>A &amp; B &mdash; My Blog</title>"));
		assert!(html.contains("<h1>A &amp; B</h1>"));
		assert!(html.contains("content=\"&lt;tag&gt;\""));
		assert!(!html.contains("<tag>"));
	}
}
