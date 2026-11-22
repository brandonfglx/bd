use std::fs;
use std::io::{self, IsTerminal, Write};

use anyhow::{Context, Result, bail};

use crate::config::BlogConfig;
use crate::page::PAGE_META;
use crate::slug;

pub fn run(slug: &str, force: bool) -> Result<()> {
	let cwd = std::env::current_dir().context("failed to determine current directory")?;
	let root = BlogConfig::find_root(&cwd)?;

	slug::validate(slug)?;

	let dir = root.join(slug);
	if !dir.is_dir() {
		bail!("page not found: {slug}");
	}

	if !dir.join(PAGE_META).is_file() {
		bail!("{slug} is not a bd page (missing {PAGE_META})");
	}

	let metadata = fs::symlink_metadata(&dir)
		.with_context(|| format!("failed to inspect {}", dir.display()))?;
	if metadata.file_type().is_symlink() {
		bail!("refusing to remove symlink: {}", dir.display());
	}

	if !force {
		if !io::stdin().is_terminal() {
			bail!("refusing to delete without --force (stdin is not a terminal)");
		}

		print!("Delete page \"{slug}\"? [y/N] ");
		io::stdout().flush().context("failed to flush stdout")?;

		let mut answer = String::new();
		io::stdin()
			.read_line(&mut answer)
			.context("failed to read confirmation")?;

		if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
			println!("Cancelled.");
			return Ok(());
		}
	}

	fs::remove_dir_all(&dir).with_context(|| format!("failed to remove {}", dir.display()))?;
	println!("Removed page \"{slug}\"");
	Ok(())
}
