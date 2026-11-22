use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::config::BlogConfig;
use crate::git;
use crate::page::PAGE_META;
use crate::slug;

pub fn run(page: Option<&str>) -> Result<()> {
	let cwd = std::env::current_dir().context("failed to determine current directory")?;
	let root = BlogConfig::find_root(&cwd)?;
	let blog = BlogConfig::load(&root)?;

	git::check()?;
	git::require_repo(&root)?;

	let pathspecs = pathspecs(&root, &blog, page)?;
	let label = match page {
		Some(page) => format!("page \"{page}\""),
		None => "all pages".to_string(),
	};
	let message = match page {
		Some(page) => format!("Publish: {page}"),
		None => "Publish: all pages".to_string(),
	};

	git::add(&root, &pathspecs)?;

	if git::has_staged_changes(&root, &pathspecs)? {
		git::commit(&root, &message, &pathspecs)?;
		println!("Committed {label}.");
	} else if !git::has_head(&root)? {
		bail!("nothing to publish: the repository has no commits and no changes");
	} else {
		println!("No changes to commit.");
	}

	let remote = ensure_remote(&root)?;
	let branch = git::current_branch(&root)?;
	let set_upstream = !git::has_upstream(&root)?;
	git::push(&root, &remote, &branch, set_upstream)?;

	println!("Published {label}.");

	Ok(())
}

fn pathspecs(root: &Path, blog: &BlogConfig, page: Option<&str>) -> Result<Vec<String>> {
	let Some(page) = page else {
		let mut specs = vec![".".to_string()];
		let output = blog.build.output.to_string_lossy().replace('\\', "/");
		if !output.is_empty() {
			specs.push(format!(":(exclude,literal){output}"));
		}

		for prefix in [".bd-build-", ".bd-old-"] {
			specs.push(format!(":(exclude,glob){prefix}*"));
			specs.push(format!(":(exclude,glob)**/{prefix}*"));
		}

		return Ok(specs);
	};

	slug::validate(page)?;
	let dir = root.join(page);
	if !dir.is_dir() {
		bail!("page not found: {page}");
	}

	if !dir.join(PAGE_META).is_file() {
		bail!("{page} is not a bd page (missing {PAGE_META})");
	}

	Ok(vec![page.to_string()])
}

fn ensure_remote(root: &Path) -> Result<String> {
	let remotes = git::remotes(root)?;
	if let Some(origin) = remotes.iter().find(|remote| remote.as_str() == "origin") {
		return Ok(origin.clone());
	}
	if let Some(remote) = remotes.first() {
		return Ok(remote.clone());
	}

	if !io::stdin().is_terminal() {
		bail!(
			"no git remote is configured; add one with `git remote add origin <URL>` and run `bd publish` again"
		);
	}

	print!("Remote URL (for example git@github.com:user/blog.git): ");
	io::stdout().flush().context("failed to flush stdout")?;

	let mut url = String::new();
	io::stdin()
		.read_line(&mut url)
		.context("failed to read remote URL")?;
	let url = url.trim();
	if url.is_empty() {
		bail!("no remote URL provided");
	}

	git::add_remote(root, "origin", url)?;
	println!("Added remote \"origin\" ({url})");
	Ok("origin".to_string())
}