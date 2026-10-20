//! Embeds a running session's real screen in a pseudo-terminal: `spawn`
//! launches `claude attach <id>` (or, for a test, any program) behind a pty
//! with the clean environment of `claude::clean_env`, `pump` drains what the
//! child has written into a `vt100` screen without ever blocking the UI
//! thread, and `Drop` kills and reaps only the child, so closing a pane never
//! blocks (design.md requirement 3, the Decisions section, ADR-0010).
//!
//! STORY-081's step 1 implements `Terminal` in this file, against the tests
//! below, written first from design.md's contract.

use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::claude::clean_env;

/// A child process running behind a pseudo-terminal, its output kept as a
/// `vt100` screen. Reading the child never blocks the caller: a background
/// thread does the blocking read and forwards chunks over a channel that
/// [`Terminal::pump`] drains.
pub(crate) struct Terminal {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    output: Receiver<Vec<u8>>,
    parser: vt100::Parser,
    rows: u16,
    cols: u16,
}

impl Terminal {
    /// Spawns `program args...` in a pseudo-terminal of `rows`×`cols` cells, in
    /// the clean environment (`crate::claude::clean_env()`).
    pub(crate) fn spawn(
        program: &Path,
        args: &[&str],
        rows: u16,
        cols: u16,
    ) -> Result<Terminal, String> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("failed to open a pseudo-terminal: {error}"))?;

        let mut cmd = CommandBuilder::new(program);
        cmd.env_clear();
        for (key, value) in clean_env() {
            cmd.env(key, value);
        }
        for arg in args {
            cmd.arg(arg);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|error| format!("failed to spawn {}: {error}", program.display()))?;
        // Drop our copy of the slave fd so the child's own copy is the only
        // one left; otherwise the reader thread below never sees EOF.
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("failed to clone the pty reader: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("failed to take the pty writer: {error}"))?;

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => return,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
        });

        Ok(Terminal {
            master: pair.master,
            writer,
            child,
            output: rx,
            parser: vt100::Parser::new(nz(rows), nz(cols), 0),
            rows,
            cols,
        })
    }

    /// Feeds whatever the child has written since the last call into the vt100
    /// screen. Never blocks: the child's output is read on its own thread.
    pub(crate) fn pump(&mut self) {
        while let Ok(bytes) = self.output.try_recv() {
            self.parser.process(&bytes);
        }
    }

    pub(crate) fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    /// Writes to the child's stdin.
    pub(crate) fn send(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// Resizes the pty and the vt100 screen to `rows`×`cols`.
    pub(crate) fn resize(&mut self, rows: u16, cols: u16) {
        if (rows, cols) == (self.rows, self.cols) || rows == 0 || cols == 0 {
            return;
        }

        self.rows = rows;
        self.cols = cols;
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        self.parser.screen_mut().set_size(nz(rows), nz(cols));
    }

    /// Whether the child has exited.
    pub(crate) fn has_ended(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Terminal {
    /// Kills, then reaps, only the child: killing first means the wait
    /// returns almost immediately instead of blocking on a child that might
    /// otherwise run forever, so dropping a terminal never blocks the UI
    /// thread.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `vt100::Parser::new` needs a non-zero size; clamps to at least 1 cell.
fn nz(n: u16) -> NonZeroU16 {
    NonZeroU16::new(n.max(1)).expect("n.max(1) is never zero")
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use super::*;

    /// Every pseudo-terminal test polls rather than sleeps a fixed amount,
    /// and gives up after this long - the risk design.md names: these tests
    /// depend on timing and on `/bin/sh`, `cat` and `stty`.
    const DEADLINE: Duration = Duration::from_secs(5);

    /// Pumps `terminal` and checks `done` until it is satisfied or
    /// `DEADLINE` passes; `true` only if `done` was ever satisfied.
    fn wait_for(terminal: &mut Terminal, mut done: impl FnMut(&Terminal) -> bool) -> bool {
        let started = Instant::now();
        loop {
            terminal.pump();
            if done(terminal) {
                return true;
            }
            if started.elapsed() > DEADLINE {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// A terminal shows what its child prints, and follows its size
    #[test]
    fn a_terminal_shows_what_its_child_prints_and_follows_its_size() {
        // The loop keeps reporting `stty size` every 100ms, so the test can
        // observe the pty's size change after `resize` without writing to
        // the child's stdin (which `cat` alone could not answer from).
        let mut terminal = Terminal::spawn(
            Path::new("/bin/sh"),
            &[
                "-c",
                "echo hello-from-the-child; while true; do stty size; sleep 0.1; done",
            ],
            24,
            80,
        )
        .expect("spawning /bin/sh succeeds");

        let printed = wait_for(&mut terminal, |t| {
            t.screen().contents().contains("hello-from-the-child")
        });
        assert!(printed, "the child's own output reaches the screen");

        let sized = wait_for(&mut terminal, |t| t.screen().contents().contains("24 80"));
        assert!(sized, "the pty starts at the size it was spawned with");

        terminal.resize(30, 100);

        let resized = wait_for(&mut terminal, |t| t.screen().contents().contains("30 100"));
        assert!(
            resized,
            "the pty's actual size followed resize, as the child's own `stty size` reports it"
        );
    }

    /// The child gets only the clean environment
    #[test]
    fn the_child_gets_only_the_clean_environment() {
        // Rust 2024 requires `unsafe` around `set_var`; kept to the minimum
        // this assertion needs, and undone once the dump has been read.
        unsafe {
            std::env::set_var("CELLWORKS_TEST_LEAK", "should-not-reach-the-child");
        }

        // `/usr/bin/env` directly, not a shell, so no shell-local variable
        // (`PWD`, `SHLVL`, ...) can be mistaken for something the clean
        // environment let through.
        let mut terminal = Terminal::spawn(Path::new("/usr/bin/env"), &[], 24, 80)
            .expect("spawning /usr/bin/env succeeds");

        let dumped = wait_for(&mut terminal, |t| t.screen().contents().contains("TERM="));
        assert!(dumped, "the child printed its environment");

        let dump = terminal.screen().contents();
        unsafe {
            std::env::remove_var("CELLWORKS_TEST_LEAK");
        }

        assert!(
            !dump.contains("CELLWORKS_TEST_LEAK"),
            "an unrelated variable set in the test process never reaches the child: {dump:?}"
        );
        let names: std::collections::BTreeSet<&str> = dump
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(name, _)| name)
            .collect();
        // portable-pty always adds SHELL itself when it spawns a child.
        let allowed: std::collections::BTreeSet<&str> =
            ["HOME", "PATH", "USER", "LANG", "TERM", "SHELL"]
                .into_iter()
                .collect();
        assert!(
            names.is_subset(&allowed),
            "only HOME, PATH, USER, LANG and TERM (and portable-pty's SHELL) are visible to the child: {names:?}"
        );
        assert!(
            dump.lines().any(|line| line == "TERM=xterm-256color"),
            "TERM is fixed to xterm-256color regardless of the test's own TERM: {dump:?}"
        );
    }

    /// Dropping a terminal ends its child
    #[test]
    fn dropping_a_terminal_ends_its_child() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let pidfile = dir.path().join("pid");

        // `exec cat` replaces the shell with `cat` in the same process, so
        // the pid the shell wrote to `pidfile` with `$$` is `cat`'s own pid -
        // the crate exposes no pid of its own to check directly.
        let mut terminal = Terminal::spawn(
            Path::new("/bin/sh"),
            &["-c", &format!("echo $$ > {}; exec cat", pidfile.display())],
            24,
            80,
        )
        .expect("spawning /bin/sh succeeds");

        let wrote_pid = wait_for(&mut terminal, |_| {
            pidfile.is_file() && !pid_of(&pidfile).is_empty()
        });
        assert!(wrote_pid, "the child reported its own pid");
        let pid = pid_of(&pidfile);
        assert!(
            is_running(&pid),
            "the child is running before the terminal is dropped"
        );

        let started = Instant::now();
        drop(terminal);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "dropping a terminal never blocks the caller waiting on pty I/O"
        );

        let gone = {
            let started = Instant::now();
            loop {
                if !is_running(&pid) {
                    break true;
                }
                if started.elapsed() > DEADLINE {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        };
        assert!(
            gone,
            "the child no longer exists after the terminal is dropped"
        );
    }

    /// Whether process `pid` exists (`kill -0`, which works on macOS and Linux).
    fn is_running(pid: &str) -> bool {
        std::process::Command::new("kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    fn pid_of(pidfile: &Path) -> String {
        std::fs::read_to_string(pidfile)
            .unwrap_or_default()
            .trim()
            .to_owned()
    }
}
