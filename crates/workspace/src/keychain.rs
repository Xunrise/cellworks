//! Where the Linear API key lives: the macOS Keychain, through the `security`
//! command line tool as a child process (ADR-0003). The key is never written to
//! the settings file and never logged; errors name what failed, not the key.

use std::process::{Command, Stdio};
use std::sync::Mutex;

const SERVICE: &str = "cellworks-linear";
const ACCOUNT: &str = "default";

/// A place to keep one secret.
pub trait Keychain: Send + Sync {
    fn get(&self) -> Option<String>;
    fn set(&self, key: &str) -> Result<(), String>;
}

/// The login keychain, through `security`.
pub struct SecurityCli;

impl Keychain for SecurityCli {
    fn get(&self) -> Option<String> {
        let out = Command::new("security")
            .args(["find-generic-password", "-s", SERVICE, "-a", ACCOUNT, "-w"])
            .stderr(Stdio::null())
            .output()
            .ok()?;
        let key = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        (out.status.success() && !key.is_empty()).then_some(key)
    }

    fn set(&self, key: &str) -> Result<(), String> {
        // ponytail: the key is on `security`'s argument vector while it runs,
        // as `security add-generic-password -w` requires; no stdin form exists.
        let status = Command::new("security")
            .args([
                "add-generic-password",
                "-U",
                "-s",
                SERVICE,
                "-a",
                ACCOUNT,
                "-w",
            ])
            .arg(key)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| "the `security` tool could not be started".to_owned())?;
        status
            .success()
            .then_some(())
            .ok_or_else(|| "the keychain refused the key".to_owned())
    }
}

/// A keychain in memory, for tests and for builds without a real one.
#[derive(Default)]
pub struct MemoryKeychain(Mutex<Option<String>>);

impl Keychain for MemoryKeychain {
    fn get(&self) -> Option<String> {
        self.0.lock().ok()?.clone()
    }

    fn set(&self, key: &str) -> Result<(), String> {
        *self.0.lock().map_err(|_| "poisoned".to_owned())? = Some(key.to_owned());
        Ok(())
    }
}
