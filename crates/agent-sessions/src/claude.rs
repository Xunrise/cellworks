//! Reading `claude agents --json`: the roster of sessions Claude Code itself
//! currently knows about, read tolerantly since neither the command nor its
//! output shape is a published contract (docs/arc42/03-context.md).

/// Why `claude agents --json` could not be read.
pub struct ClaudeUnavailable(pub String);

/// Lists the sessions `claude` currently knows about, behind a trait so a
/// test never runs the real CLI.
pub trait AgentList: Send + Sync {
    fn list(&self) -> Result<Vec<Agent>, ClaudeUnavailable>;
}

/// One entry of `claude agents --json`.
#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub session_id: String,
    pub name: String,
    pub cwd: String,
    pub status: String,
    pub state: String,
}

/// Runs the real `claude agents --json`, with a clean environment
/// (ADR-0010).
pub struct ClaudeCli;

impl AgentList for ClaudeCli {
    fn list(&self) -> Result<Vec<Agent>, ClaudeUnavailable> {
        let env_var = |name: &str| std::env::var(name).unwrap_or_default();

        let output = std::process::Command::new("claude")
            .args(["agents", "--json"])
            .env_clear()
            .env("HOME", env_var("HOME"))
            .env("PATH", env_var("PATH"))
            .env("USER", env_var("USER"))
            .env("TERM", env_var("TERM"))
            .output()
            .map_err(|error| ClaudeUnavailable(format!("failed to run claude: {error}")))?;

        if !output.status.success() {
            return Err(ClaudeUnavailable(format!(
                "claude agents --json exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(parse_agents(&output.stdout))
    }
}

/// Parses `claude agents --json`'s stdout tolerantly: unknown fields are
/// ignored, a missing or empty `name` falls back to `id`, an entry with no
/// or an empty `sessionId` is skipped entirely, and a non-array top level
/// value, or JSON that does not parse at all, reads as no agents.
pub fn parse_agents(json: &[u8]) -> Vec<Agent> {
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_slice::<serde_json::Value>(json)
    else {
        return Vec::new();
    };

    let as_str = |value: &serde_json::Value, key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned()
    };

    entries
        .iter()
        .filter_map(|entry| {
            let session_id = as_str(entry, "sessionId");
            if session_id.is_empty() {
                return None;
            }

            let name = entry
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| as_str(entry, "id"));

            Some(Agent {
                session_id,
                name,
                cwd: as_str(entry, "cwd"),
                status: as_str(entry, "status"),
                state: as_str(entry, "state"),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins [`parse_agents`]'s tolerant reading of `claude agents --json`:
    /// a well-formed entry, one with no `name` (falls back to `id`), one
    /// with no `state`, one with no `sessionId` and one with an empty
    /// `sessionId` (both skipped), and one carrying an unrelated extra field
    /// (ignored, the entry itself kept).
    #[test]
    fn agents_are_read_tolerantly() {
        let json = br#"[
            {"sessionId": "sess-1", "id": "short-1", "name": "pricing-fix", "cwd": "/repo", "status": "busy", "state": "working"},
            {"sessionId": "sess-2", "id": "short-2", "cwd": "/repo", "status": "idle", "state": "done"},
            {"sessionId": "sess-3", "id": "short-3", "name": "no-state", "cwd": "/repo", "status": "busy"},
            {"id": "short-4", "name": "no-session-id", "cwd": "/repo", "status": "busy", "state": "working"},
            {"sessionId": "", "id": "short-5", "name": "empty-session-id", "cwd": "/repo"},
            {"sessionId": "sess-6", "id": "short-6", "name": "with-extra", "cwd": "/repo", "status": "idle", "state": "done", "somethingNew": {"a": 1}}
        ]"#;

        let agents = parse_agents(json);

        assert_eq!(
            agents.len(),
            4,
            "two entries with no usable sessionId are skipped: {agents:?}"
        );
        assert_eq!(
            agents[0],
            Agent {
                session_id: "sess-1".to_owned(),
                name: "pricing-fix".to_owned(),
                cwd: "/repo".to_owned(),
                status: "busy".to_owned(),
                state: "working".to_owned(),
            }
        );
        assert_eq!(
            agents[1].name, "short-2",
            "a missing name falls back to id: {agents:?}"
        );
        assert_eq!(
            agents[2].state, "",
            "an absent state reads as empty, not an error: {agents:?}"
        );
        assert_eq!(
            agents[3].session_id, "sess-6",
            "an unrelated extra field does not stop the entry being read: {agents:?}"
        );
        assert!(
            agents.iter().all(|agent| !agent.session_id.is_empty()),
            "every entry with no usable sessionId was skipped: {agents:?}"
        );
    }

    #[test]
    fn a_non_array_top_level_value_reads_as_no_agents() {
        assert_eq!(parse_agents(br#"{"not": "an array"}"#), Vec::new());
    }

    #[test]
    fn json_that_does_not_parse_reads_as_no_agents() {
        assert_eq!(parse_agents(b"not json at all"), Vec::new());
    }
}
