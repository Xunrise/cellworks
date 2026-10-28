//! The command line of `cellworks`: three arguments, parsed without a
//! dependency, and without a process or a window.

/// What the arguments ask the binary to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Open the window.
    Run,
    /// Report the version this binary was built as.
    Version,
    /// Print what Cellworks knows about a project, as JSON.
    Facts { repo: String, kind: String },
    /// Build a project lens's data from its definition.
    LensBuild { repo: String, name: String },
}

/// Why the arguments could not be read.
#[derive(Debug, PartialEq, Eq)]
pub enum CliError {
    /// The named argument is not one `cellworks` accepts.
    Unknown(String),
}

/// Reads the arguments that follow the program name.
///
/// Nothing but no arguments and a single `--version` is accepted; everything
/// else, `--help` included, is unknown. Later stories add modes, and they must
/// leave the meaning of these two alone.
pub fn parse_args(args: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let args: Vec<String> = args.collect();

    match args.as_slice() {
        [] => Ok(Command::Run),
        [only] if only == "--version" => Ok(Command::Version),
        [facts, repo] if facts == "facts" => Ok(Command::Facts {
            repo: repo.clone(),
            kind: "files".to_owned(),
        }),
        [facts, repo, kind] if facts == "facts" => Ok(Command::Facts {
            repo: repo.clone(),
            kind: kind.clone(),
        }),
        [lens, build, repo, name] if lens == "lens" && build == "build" => Ok(Command::LensBuild {
            repo: repo.clone(),
            name: name.clone(),
        }),
        [first, ..] => Err(CliError::Unknown(first.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(given: &[&str]) -> impl Iterator<Item = String> {
        given
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn no_arguments_runs_the_app() {
        assert_eq!(parse_args(args(&[])), Ok(Command::Run));
    }

    #[test]
    fn version_reports_the_version() {
        assert_eq!(parse_args(args(&["--version"])), Ok(Command::Version));
    }

    #[test]
    fn an_unknown_argument_is_an_error_naming_it() {
        assert_eq!(
            parse_args(args(&["--nope"])),
            Err(CliError::Unknown("--nope".to_owned()))
        );
    }

    #[test]
    fn a_recognised_argument_twice_is_an_error_too() {
        assert_eq!(
            parse_args(args(&["--version", "--version"])),
            Err(CliError::Unknown("--version".to_owned()))
        );
    }
}
