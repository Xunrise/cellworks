use std::process::ExitCode;

use cellworks::app;
use cellworks::cli::{self, CliError, Command};
use cellworks::tools;

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
        Ok(Command::Facts { repo, kind }) => match tools::facts(&repo, &kind) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("cellworks: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(Command::LensBuild { repo, name }) => match tools::lens_build(&repo, &name) {
            Ok(report) => {
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("cellworks: {error}");
                ExitCode::FAILURE
            }
        },
        Err(CliError::Unknown(argument)) => {
            eprintln!("cellworks: unrecognised argument '{argument}'");
            eprintln!(
                "usage: cellworks [--version | facts <repo> [files|items|calls|imports|cochange] | lens build <repo> <name>]"
            );
            ExitCode::from(2)
        }
    }
}
