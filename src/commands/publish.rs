use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::commands::build::{BACKUP_PREFIX, STAGING_PREFIX};
use crate::commands::init;
use crate::config::BlogConfig;
use crate::git;
use crate::page::PAGE_META;
use crate::slug;
use crate::version;

pub fn run(page: Option<&str>, message: Option<&str>, force: bool, upgrade: bool) -> Result<()> {
	let cwd = std::env::current_dir().context("failed to determine current directory")?;
	let root = BlogConfig::find_root(&cwd)?;
	run_in(&root, page, message, force, upgrade)
}

pub fn run_in(
	root: &Path,
	page: Option<&str>,
	message: Option<&str>,
	force: bool,
	upgrade: bool,
) -> Result<()> {
	if upgrade {
		init::run(root, false, true, None, None)?;
	}

	let blog = BlogConfig::load(root)?;

	git::check()?;
	git::require_repo(root)?;

	let pathspecs = pathspecs(root, &blog, page)?;
	let label = match page {
		Some(page) => format!("page \"{page}\""),
		None => "all pages".to_string(),
	};
	let commit_message = match message {
		Some(message) if message.trim().is_empty() => {
			bail!("commit message cannot be empty");
		}
		Some(message) => message.to_string(),
		None => match page {
			Some(page) => format!("Publish: {page}"),
			None => "Publish: all pages".to_string(),
		},
	};

	if !force && !upgrade {
		version::ensure_current()?;
	}

	let remote = ensure_remote(root)?;
	let remote_url = git::remote_url(root, &remote)?;
	let branch = git::current_branch(root)?;
	let set_upstream = !git::has_upstream(root)?;

	println!("bd publish configuration:");
	println!("\tRepository: {}", root.display());
	println!("\tRemote: {remote} ({})", remote_url);
	println!("\tBranch: {branch}");

	if page.is_some() {
		git::add(root, &pathspecs)?;
	} else {
		let output = output_path(&blog);
		git::add_all_except(root, |path| is_excluded(path, &output))?;
	}

	if git::has_staged_changes(root, &pathspecs)? {
		let changes = git::staged_changes(root, &pathspecs)?;
		println!("Commit message: {commit_message}");

		if !changes.is_empty() {
			println!("Changes:");
			for change in &changes {
				println!("\t{} {}", change.status, change.path);
			}
		}

		git::commit(root, &commit_message, &pathspecs)?;
		let short = git::head_short(root)?;
		println!("Committed {label} as {short}.");
	} else if !git::has_head(root)? {
		bail!("nothing to publish: the repository has no commits and no changes");
	} else {
		println!("No changes to commit.");
	}

	git::push(root, &remote, &branch, set_upstream)?;

	println!("Pushed to {remote}/{branch}");
	println!("Published {label}.");

	Ok(())
}

fn pathspecs(root: &Path, blog: &BlogConfig, page: Option<&str>) -> Result<Vec<String>> {
	let Some(page) = page else {
		let mut specs = vec![".".to_string()];
		let output = output_path(blog);
		if !output.is_empty() {
			specs.push(format!(":(exclude,literal){output}"));
		}

		for prefix in [STAGING_PREFIX, BACKUP_PREFIX] {
			specs.push(format!(":(exclude,glob){prefix}*"));
			specs.push(format!(":(exclude,glob)**/{prefix}*"));
		}

		return Ok(specs);
	};

	slug::validate_existing(page)?;
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

	print!("Remote URL (e.g. git@github.com:user/blog.git): ");
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

fn output_path(blog: &BlogConfig) -> String {
	blog.build
		.output
		.to_string_lossy()
		.replace('\\', "/")
		.trim_end_matches('/')
		.to_string()
}

fn is_excluded(path: &str, output: &str) -> bool {
	if !output.is_empty()
		&& (path == output
			|| path
				.strip_prefix(output)
				.is_some_and(|rest| rest.starts_with('/')))
	{
		return true;
	}

	path.split('/').any(|component| {
		component.starts_with(STAGING_PREFIX) || component.starts_with(BACKUP_PREFIX)
	})
}
