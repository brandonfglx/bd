use anyhow::Result;

use crate::commands::build::{self, BuildArgs};
use crate::server::PreviewServer;

pub fn run(port: u16) -> Result<()> {
	let outcome = build::build(&BuildArgs::default())?;
	build::print_summary(&outcome);

	let server = PreviewServer::bind(outcome.output, "127.0.0.1", port)?;
	println!("Serving preview at http://{}", server.addr());
	println!("Preview is static: re-run `bd preview` after changing pages.");
	println!("Press Ctrl+C to stop.");

	server.run()
}
