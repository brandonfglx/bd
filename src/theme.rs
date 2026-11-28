use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
	#[default]
	Classic,
	Grid,
	Personal,
}

impl ThemeName {
	pub const ALL: [Self; 3] = [Self::Classic, Self::Grid, Self::Personal];

	pub fn as_str(self) -> &'static str {
		match self {
			Self::Classic => "classic",
			Self::Grid => "grid",
			Self::Personal => "personal",
		}
	}

	pub fn theme(self) -> &'static Theme {
		match self {
			Self::Classic => &CLASSIC,
			Self::Grid => &GRID,
			Self::Personal => &PERSONAL,
		}
	}
}

#[derive(Debug)]
pub struct Theme {
	pub name: ThemeName,
	pub base: &'static str,
	pub index: &'static str,
	pub post: &'static str,
	pub tag: &'static str,
	pub tags: &'static str,
	pub about: &'static str,
	pub not_found: &'static str,
	pub style_css: &'static str,
}

pub static CLASSIC: Theme = Theme {
	name: ThemeName::Classic,
	base: include_str!("themes/shared/base.html"),
	index: include_str!("themes/classic/index.html"),
	post: include_str!("themes/classic/post.html"),
	tag: include_str!("themes/shared/tag.html"),
	tags: include_str!("themes/shared/tags.html"),
	about: include_str!("themes/shared/about.html"),
	not_found: include_str!("themes/shared/404.html"),
	style_css: concat!(
		include_str!("themes/shared/style.css"),
		"\n",
		include_str!("themes/classic/style.css")
	),
};

pub static GRID: Theme = Theme {
	name: ThemeName::Grid,
	base: include_str!("themes/shared/base.html"),
	index: include_str!("themes/grid/index.html"),
	post: include_str!("themes/grid/post.html"),
	tag: include_str!("themes/shared/tag.html"),
	tags: include_str!("themes/shared/tags.html"),
	about: include_str!("themes/shared/about.html"),
	not_found: include_str!("themes/shared/404.html"),
	style_css: concat!(
		include_str!("themes/shared/style.css"),
		"\n",
		include_str!("themes/grid/style.css")
	),
};

pub static PERSONAL: Theme = Theme {
	name: ThemeName::Personal,
	base: include_str!("themes/shared/base.html"),
	index: include_str!("themes/personal/index.html"),
	post: include_str!("themes/personal/post.html"),
	tag: include_str!("themes/shared/tag.html"),
	tags: include_str!("themes/shared/tags.html"),
	about: include_str!("themes/shared/about.html"),
	not_found: include_str!("themes/shared/404.html"),
	style_css: concat!(
		include_str!("themes/shared/style.css"),
		"\n",
		include_str!("themes/personal/style.css")
	),
};

impl Theme {
	pub fn templates(&self) -> [(&'static str, &'static str); 7] {
		[
			("base.html", self.base),
			("index.html", self.index),
			("post.html", self.post),
			("tag.html", self.tag),
			("tags.html", self.tags),
			("about.html", self.about),
			("404.html", self.not_found),
		]
	}

	pub fn write(&self, root: &Path) -> Result<()> {
		let templates = root.join("templates");
		fs::create_dir_all(&templates)
			.with_context(|| format!("failed to create {}", templates.display()))?;

		for (name, contents) in self.templates() {
			let path = templates.join(name);
			fs::write(&path, contents)
				.with_context(|| format!("failed to write {}", path.display()))?;
		}

		let static_dir = root.join("static");
		fs::create_dir_all(&static_dir)
			.with_context(|| format!("failed to create {}", static_dir.display()))?;

		let style = static_dir.join("style.css");
		fs::write(&style, self.style_css)
			.with_context(|| format!("failed to write {}", style.display()))?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_theme_has_templates_and_css() {
		for name in ThemeName::ALL {
			let theme = name.theme();
			assert_eq!(theme.name, name);
			assert!(!theme.style_css.trim().is_empty());

			for (template, contents) in theme.templates() {
				assert!(!contents.trim().is_empty(), "{template} is empty");
			}
		}
	}
}
