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
    /// The short id `claude attach`/`claude stop` take; empty when the entry
    /// carries no usable id, as for an interactive session.
    pub id: String,
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
        let mut cmd = std::process::Command::new("claude");
        cmd.args(["agents", "--json"]).env_clear();
        for (key, value) in clean_env() {
            cmd.env(key, value);
        }

        let output = cmd
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
                id: as_str(entry, "id"),
                name,
                cwd: as_str(entry, "cwd"),
                status: as_str(entry, "status"),
                state: as_str(entry, "state"),
            })
        })
        .collect()
}

/// The environment every `claude` child process Cellworks spawns gets
/// (ADR-0010): cleared of everything else, with `TERM` fixed so the embedded
/// terminal gets predictable capabilities regardless of the user's own.
pub(crate) fn clean_env() -> [(&'static str, String); 5] {
    let env_var = |name: &str| std::env::var(name).unwrap_or_default();

    [
        ("HOME", env_var("HOME")),
        ("PATH", env_var("PATH")),
        ("USER", env_var("USER")),
        ("LANG", env_var("LANG")),
        ("TERM", "xterm-256color".to_owned()),
    ]
}

/// Stops the agent `id` names, via `claude stop <id>`.
pub(crate) fn stop(claude: &std::path::Path, id: &str) -> Result<(), String> {
    let mut cmd = std::process::Command::new(claude);
    cmd.arg("stop").arg(id).env_clear();
    for (key, value) in clean_env() {
        cmd.env(key, value);
    }

    let output = cmd
        .output()
        .map_err(|error| format!("failed to run claude stop: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "claude stop exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

/// Resumes `session_id` as a new background session named `name`, via
/// `claude --resume <session_id> --bg -n <name>`, returning the new session's
/// copy id parsed out of its combined stdout and stderr.
pub(crate) fn continue_in_background(
    claude: &std::path::Path,
    session_id: &str,
    name: &str,
) -> Result<String, String> {
    let mut cmd = std::process::Command::new(claude);
    cmd.args(["--resume", session_id, "--bg", "-n", name])
        .env_clear();
    for (key, value) in clean_env() {
        cmd.env(key, value);
    }

    let output = cmd
        .output()
        .map_err(|error| format!("failed to run claude --resume: {error}"))?;

    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    parse_bg_id(&text).ok_or_else(|| text.trim().chars().take(200).collect())
}

/// The token (alphanumerics, `-` and `_`; the acceptance test's copy id is
/// hyphenated) right after the literal "backgrounded" in `output`
/// (design.md's edge case): `claude --resume ... --bg` prints "backgrounded
/// · <id> · <name>", punctuation and all; `None` if "backgrounded" never
/// appears or nothing alphanumeric follows it.
pub(crate) fn parse_bg_id(output: &str) -> Option<String> {
    let after = output.split("backgrounded").nth(1)?;
    let id: String = after
        .trim_start_matches(|c: char| !c.is_ascii_alphanumeric())
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();

    if id.is_empty() { None } else { Some(id) }
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
                id: "short-1".to_owned(),
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

    /// Pins [`parse_bg_id`] (design.md's edge case): `claude --resume ... --bg`
    /// prints "started a copy as <id>" then "backgrounded · <id> · <name>";
    /// the copy's id is the alphanumeric token right after the literal
    /// "backgrounded", wherever it falls in the output, punctuation and all;
    /// output that never says "backgrounded" at all yields no id.
    #[test]
    fn the_copy_id_is_the_token_after_backgrounded() {
        assert_eq!(
            parse_bg_id("started a copy as ab12cd34\nbackgrounded · ab12cd34 · pricing-fix\n"),
            Some("ab12cd34".to_owned())
        );
        assert_eq!(
            parse_bg_id("...backgrounded · ab12cd34\n"),
            Some("ab12cd34".to_owned())
        );
        assert_eq!(
            parse_bg_id("error: no job matching docs-run"),
            None,
            "no \"backgrounded\" anywhere in the output means no id"
        );
    }
}
