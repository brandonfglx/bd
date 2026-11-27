use crate::config::CssFramework;

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
		#[arg(long, conflicts_with = "upgrade")]
		force: bool,
		#[arg(long)]
		upgrade: bool,
		#[arg(long, value_enum, value_name = "STYLE")]
		css: Option<CssFramework>,
	},
	Add {
		title: String,
		#[arg(long)]
		slug: Option<String>,
		#[arg(long)]
		date: Option<String>,
		#[arg(long)]
		draft: bool,
		#[arg(long = "tag", value_name = "TAG")]
		tags: Vec<String>,
	},
	Rm {
		slug: String,
		#[arg(short, long)]
		force: bool,
		#[arg(long = "tag", value_name = "TAG")]
		tags: Vec<String>,
	},
	Build {
		#[arg(long)]
		drafts: bool,
		#[arg(long, value_name = "DIR")]
		output: Option<std::path::PathBuf>,
		#[arg(short, long)]
		verbose: bool,
		#[arg(long)]
		force: bool,
	},
	Publish {
		#[arg(long, value_name = "PAGE", conflicts_with = "upgrade")]
		page: Option<String>,
		#[arg(short, long, value_name = "MESSAGE")]
		message: Option<String>,
		#[arg(long, conflicts_with = "upgrade")]
		force: bool,
		#[arg(long)]
		upgrade: bool,
	},
	Preview {
		#[arg(long, default_value_t = 10203, value_name = "PORT")]
		port: u16,
	},
}
