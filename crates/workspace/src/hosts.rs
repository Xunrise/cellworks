//! Deciding whether a remote host is reachable: a background `ssh` probe
//! behind a `HostProbe` trait, one of each per launch (ADR-0003).
//!
//! STORY-078, requirement 4: `ssh -o BatchMode=yes -o ConnectTimeout=5 --
//! <host> true` on a background thread; exit 0 is connected, anything else
//! (including a missing `ssh`) is offline. The binary passes `SshProbe`;
//! every test here passes a fake, so no test here ever spawns `ssh`.
//! `--` before the host keeps a host name that looks like an option from
//! reaching ssh's argument vector, together with the name check in
//! `settings::read`.

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;

use crate::event::WorkspaceEvent;

/// Whether a remote host answers. The binary passes [`SshProbe`]; every test
/// passes a fake, so no test here ever spawns `ssh`.
pub trait HostProbe {
    fn connected(&self, host: &str) -> bool;
}

/// [`HostProbe`] by running `ssh -o BatchMode=yes -o ConnectTimeout=5 -- <host> true`
/// as a child process: exit 0 is connected, anything else — a refused
/// connection, a password prompt `BatchMode` suppresses, a missing `ssh`
/// binary — is offline.
pub struct SshProbe;

impl SshProbe {
    /// The child process for one probe of `host`: fixed options, then `--`,
    /// then the host and `true`, so a host name that looks like an option
    /// never reaches ssh's argument vector as one — the name check in
    /// `settings::read` guards the same thing from the other side.
    fn command(host: &str) -> Command {
        let mut command = Command::new("ssh");
        command
            .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "--"])
            .arg(host)
            .arg("true")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }
}

impl HostProbe for SshProbe {
    fn connected(&self, host: &str) -> bool {
        Self::command(host)
            .status()
            .is_ok_and(|status| status.success())
    }
}

/// Probes every host in `hosts` on a thread of its own, so one slow or
/// hanging host never delays another's report.
pub fn start_probes(
    hosts: Vec<String>,
    probe: Arc<dyn HostProbe + Send + Sync>,
) -> mpsc::Receiver<WorkspaceEvent> {
    let (sender, receiver) = mpsc::channel();
    for host in hosts {
        let sender = sender.clone();
        let probe = Arc::clone(&probe);
        std::thread::spawn(move || {
            let connected = probe.connected(&host);
            // A send that fails is the UI having gone away.
            let _ = sender.send(WorkspaceEvent::HostProbed { host, connected });
        });
    }
    receiver
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::WorkspaceEvent;

    /// A fake [`HostProbe`] that answers exactly what it is told to, so no
    /// test here spawns `ssh`.
    struct Fake(std::collections::BTreeMap<&'static str, bool>);

    impl HostProbe for Fake {
        fn connected(&self, host: &str) -> bool {
            self.0.get(host).copied().unwrap_or(false)
        }
    }

    /// STORY-078, requirement 4 and the ssh risk: the host is the last
    /// argument, after a `--`, so a host named like an option never becomes
    /// one.
    #[test]
    fn the_ssh_probe_puts_the_host_after_the_options_and_double_dash() {
        let command = SshProbe::command("az.wsl");

        assert_eq!(command.get_program(), "ssh");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                "--",
                "az.wsl",
                "true",
            ]
            .map(OsStr::new)
        );
    }

    /// STORY-078, requirement 4: each host is probed on a thread of its own,
    /// and every probe is reported, connected or not.
    #[test]
    fn start_probes_reports_each_host_over_the_channel() {
        let probe = Arc::new(Fake(std::collections::BTreeMap::from([
            ("az.wsl", true),
            ("gcp.vm", false),
        ])));

        let receiver = start_probes(vec!["az.wsl".to_owned(), "gcp.vm".to_owned()], probe);

        let mut reported = Vec::new();
        for _ in 0..2 {
            match receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("every host probed is reported")
            {
                WorkspaceEvent::HostProbed { host, connected } => reported.push((host, connected)),
                other => panic!("expected HostProbed, got {other:?}"),
            }
        }
        reported.sort();

        assert_eq!(
            reported,
            vec![("az.wsl".to_owned(), true), ("gcp.vm".to_owned(), false)]
        );
    }
}
