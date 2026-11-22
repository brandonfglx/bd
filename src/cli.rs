#[derive(Debug, clap::Parser)]
#[command(name = "bd", version, about = "A small blogging system hosted on git.")]
pub struct Cli {
	#[command(subcommand)]
	pub command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
	Init {
		location: std::path::PathBuf,
	},
	Add {
		title: String,
		#[arg(long)]
		slug: Option<String>,
		#[arg(long)]
		date: Option<String>,
		#[arg(long)]
		draft: bool,
	},
	Rm {
		slug: String,
		#[arg(short, long)]
		force: bool,
	},
	Build {
		#[arg(long)]
		drafts: bool,
		#[arg(long, value_name = "DIR")]
		output: Option<std::path::PathBuf>,
		#[arg(short, long)]
		verbose: bool,
	},
}
