use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::page::{PAGE_META, Page};
use crate::slug;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tag {
	pub name: String,
	pub slug: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TagSummary {
	pub name: String,
	pub slug: String,
	pub count: usize,
}

pub fn normalize(raw: &[String], source: &Path) -> Result<Vec<Tag>> {
	let mut tags: Vec<Tag> = Vec::new();

	for value in raw {
		let name = value.trim();
		if name.is_empty() {
			bail!("{}: tag cannot be empty", source.display());
		}

		let tag_slug = slug::slugify(name);
		if tag_slug.is_empty() {
			bail!(
				"{}: tag {name:?} cannot be converted to a URL slug",
				source.display()
			);
		}

		slug::validate_tag(&tag_slug)
			.with_context(|| format!("{}: invalid tag {name:?}", source.display()))?;

		if let Some(ext) = tags.iter().find(|tag| tag.slug == tag_slug) {
			if ext.name == name {
				continue;
			}

			bail!(
				"{}: tags {:?} and {:?} both use the slug {tag_slug:?}",
				source.display(),
				ext.name,
				name
			);
		}

		tags.push(Tag {
			name: name.to_string(),
			slug: tag_slug,
		});
	}

	tags.sort_by(|left, right| left.slug.cmp(&right.slug));
	Ok(tags)
}

pub fn unique(pages: &[&Page]) -> Result<Vec<Tag>> {
	let mut tags: Vec<Tag> = Vec::new();

	for page in pages {
		for tag in &page.tags {
			match tags.iter().find(|ext| ext.slug == tag.slug) {
				Some(ext) if ext.name != tag.name => {
					bail!(
						"unique: {:?} and {:?} both use {:?} as slug; rename one of them",
						page.dir.join(PAGE_META).display(),
						tag.name,
						tag.slug
					);
				}
				Some(_) => {}
				None => tags.push(tag.clone()),
			}
		}
	}

	tags.sort_by(|left, right| left.slug.cmp(&right.slug));
	Ok(tags)
}

pub fn summarize(pages: &[&Page]) -> Result<Vec<TagSummary>> {
	let unique_tags = unique(pages)?;
	let mut summaries = Vec::with_capacity(unique_tags.len());

	for tag in unique_tags {
		let count = pages
			.iter()
			.filter(|page| page.tags.iter().any(|t| t.slug == tag.slug))
			.count();

		summaries.push(TagSummary {
			name: tag.name,
			slug: tag.slug,
			count,
		});
	}

	summaries.sort_by(|left, right| {
		left.name
			.to_lowercase()
			.cmp(&right.name.to_lowercase())
			.then_with(|| left.slug.cmp(&right.slug))
	});

	Ok(summaries)
}
