use std::process::ExitCode;

use cellworks::app;
use cellworks::cli::{self, CliError, Command};

fn main() -> ExitCode {
    match cli::parse_args(std::env::args().skip(1)) {
        Ok(Command::Run) => match app::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                // No wgpu adapter, no display, a broken driver: say why rather
                // than panicking.
                eprintln!("cellworks: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(Command::Version) => {
            println!("cellworks {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Err(CliError::Unknown(argument)) => {
            eprintln!("cellworks: unrecognised argument '{argument}'");
            eprintln!("usage: cellworks [--version]");
            ExitCode::from(2)
        }
    }
}
