use anyhow::Result;

use crate::commands::build::{self, BuildArgs};
use crate::server::PreviewServer;

pub fn run(port: u16) -> Result<()> {
	let outcome = build::build(&BuildArgs::default())?;
	build::print_summary(&outcome);

	let server = PreviewServer::bind(outcome.output, "127.0.0.1", port)?;
	println!("\nPreviewing static build at http://{}", server.addr());
	println!("Press Ctrl+C to stop.\n");

	server.run()
}
