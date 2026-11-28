use std::process::ExitCode;

fn main() -> ExitCode {
	match bd::run_cli() {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("bd: error: {error:#}");
			ExitCode::FAILURE
		}
	}
}
