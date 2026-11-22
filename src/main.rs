mod cli;
mod commands;
mod config;
mod markdown;
mod page;
mod render;
mod slug;

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
		Command::Init { location } => commands::init::run(&location),
		Command::Add {
			title,
			slug,
			date,
			draft,
		} => commands::add::run(&title, slug.as_deref(), date.as_deref(), draft),
		Command::Rm { slug, force } => commands::rm::run(&slug, force),
		Command::Build {
			drafts,
			output,
			verbose,
		} => commands::build::run(drafts, output.as_deref(), verbose),
	}
}
