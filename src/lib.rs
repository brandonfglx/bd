pub mod cli;
pub mod commands;
pub mod config;
pub mod git;
pub mod markdown;
pub mod page;
pub mod render;
pub mod server;
pub mod slug;
pub mod tags;
pub mod theme;
pub mod version;

#[cfg(feature = "gui")]
pub mod gui;

use clap::Parser;

use crate::cli::{Cli, Command};

pub fn run_cli() -> anyhow::Result<()> {
	let cli = Cli::parse();

	match cli.command {
		Command::Init {
			location,
			force,
			upgrade,
			css,
			template,
		} => commands::init::run(&location, force, upgrade, css, template),
		Command::Add {
			title,
			slug,
			date,
			draft,
			tags,
		} => commands::add::run(&title, slug.as_deref(), date.as_deref(), draft, &tags),
		Command::Rm { slug, force, tags } => commands::rm::run(&slug, force, &tags),
		Command::Build {
			drafts,
			output,
			verbose,
			force,
		} => commands::build::run(drafts, output.as_deref(), verbose, force),
		Command::Publish {
			page,
			message,
			force,
			upgrade,
		} => commands::publish::run(page.as_deref(), message.as_deref(), force, upgrade),
		Command::Preview { port } => commands::preview::run(port),
		#[cfg(feature = "gui")]
		Command::Gui { path } => gui::run(path),
	}
}
