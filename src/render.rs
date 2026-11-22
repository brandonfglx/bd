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
