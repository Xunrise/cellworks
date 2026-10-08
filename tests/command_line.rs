//! The command-line contract of the built binary, as a user meets it:
//! `cellworks --version` and an argument `cellworks` does not know.
//!
//! The binary is located with Cargo's own `CARGO_BIN_EXE_` variable, which
//! points at the right artifact under `--release` and `--target` alike. No test
//! here opens a window: neither invocation reaches `run`.

use std::process::Command;

#[test]
fn version_prints_the_compiled_in_version_and_exits_zero() {
    let output = Command::new(env!("CARGO_BIN_EXE_cellworks"))
        .arg("--version")
        .output()
        .expect("the built cellworks binary runs");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("cellworks {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn an_unknown_argument_prints_the_usage_to_stderr_and_exits_two() {
    let output = Command::new(env!("CARGO_BIN_EXE_cellworks"))
        .arg("--nope")
        .output()
        .expect("the built cellworks binary runs");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--nope"), "stderr was {stderr:?}");
    assert!(stderr.contains("--version"), "stderr was {stderr:?}");
}
