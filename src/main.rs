mod cli;
mod commands;
mod config;
mod git;
mod markdown;
mod page;
mod render;
mod server;
mod slug;
mod tags;
mod version;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> ExitCode {
	match run() {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("bd: error: {error:#}");
			ExitCode::FAILURE
		}
	}
}

fn run() -> anyhow::Result<()> {
	let cli = Cli::parse();

	match cli.command {
		Command::Init {
			location,
			force,
			upgrade,
			css,
		} => commands::init::run(&location, force, upgrade, css),
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
	}
}
