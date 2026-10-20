//! Reading one session's transcript: `.jsonl` lines tailed from a byte
//! offset, each holding a tool call that becomes a file event, tolerantly -
//! neither the transcript format nor `claude agents --json` is a published
//! contract (docs/arc42/03-context.md), so unknown tools, blocks and
//! unparsable lines are ignored rather than treated as errors.

use std::path::{Path, PathBuf};

/// One file event read off a tool call in a transcript line.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ToolCall {
    pub write: bool,
    /// Absolute: resolved against the line's own `cwd` when the tool named
    /// a relative path.
    pub path: PathBuf,
    pub line: Option<usize>,
    /// Lines added, for a write; 0 for a read.
    pub added: usize,
    /// Copied verbatim from the transcript line.
    pub timestamp: String,
}

/// A transcript file's read position: the byte offset up to which every line
/// is known to be complete.
#[derive(Default)]
pub(crate) struct Tail {
    offset: u64,
}

impl Tail {
    pub(crate) fn new() -> Tail {
        Tail::default()
    }

    /// Seeds the starting offset at `max_bytes` from the end of `path`, so
    /// the next [`Tail::read`] call reads only lines completed within
    /// roughly its last `max_bytes` - a freshly opened read-only pane does
    /// not reparse a long-running session's whole history just to show its
    /// last few entries (design.md requirement 6). Falls back to the whole
    /// file when it is shorter than `max_bytes`, or to offset 0 when its
    /// length cannot be read.
    pub(crate) fn from_end(path: &Path, max_bytes: u64) -> Tail {
        let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        Tail {
            offset: len.saturating_sub(max_bytes),
        }
    }

    /// The complete new lines of `path` since the last call; a shorter file
    /// than last time is read again from the start. Never blocks on
    /// growth - just reads what's there now.
    pub(crate) fn read(&mut self, path: &Path) -> Vec<String> {
        use std::fs::File;
        use std::io::{Read, Seek, SeekFrom};

        let Ok(mut file) = File::open(path) else {
            return Vec::new();
        };
        let Ok(len) = file.metadata().map(|m| m.len()) else {
            return Vec::new();
        };
        if len < self.offset {
            self.offset = 0;
        }
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut buf = Vec::new();
        if file.read_to_end(&mut buf).is_err() {
            return Vec::new();
        }

        let mut lines = Vec::new();
        let mut consumed: u64 = 0;
        let mut start = 0usize;
        for (i, &byte) in buf.iter().enumerate() {
            if byte == b'\n' {
                let line = String::from_utf8_lossy(&buf[start..i]).into_owned();
                lines.push(line);
                start = i + 1;
                consumed = start as u64;
            }
        }

        self.offset += consumed;
        lines
    }
}

/// Lines in `s`, with an empty string counting as 0 lines (unlike
/// `str::lines`, which agrees everywhere else).
fn line_count(s: &str) -> usize {
    if s.is_empty() { 0 } else { s.lines().count() }
}

/// Resolves `path` against `cwd` when it is relative.
fn resolve(cwd: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_owned()
    } else {
        cwd.join(path)
    }
}

/// The 1-based line on which `needle` sits in `content`, if `content` was
/// read successfully and `needle` is non-empty and found.
fn line_of(content: &Option<String>, needle: &str) -> Option<usize> {
    let content = content.as_ref()?;
    if needle.is_empty() {
        return None;
    }
    let idx = content.find(needle)?;
    Some(content[..idx].matches('\n').count() + 1)
}

/// Tool calls become file events, tolerantly: unknown tools, unparsable
/// lines, and blocks that are not `tool_use` are ignored. See
/// `design.md`'s requirement 3 for the per-tool rules this pins.
pub(crate) fn tool_events(lines: &[String]) -> Vec<ToolCall> {
    let mut events = Vec::new();

    for line in lines {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        let cwd_str = value.get("cwd").and_then(|v| v.as_str()).unwrap_or("/");
        let cwd = Path::new(cwd_str);
        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();

        let Some(content) = value["message"]["content"].as_array() else {
            continue;
        };

        for block in content {
            if block["type"].as_str() != Some("tool_use") {
                continue;
            }
            let Some(name) = block["name"].as_str() else {
                continue;
            };

            match name {
                "Read" => {
                    let Some(file_path) = block["input"]["file_path"].as_str() else {
                        continue;
                    };
                    let path = resolve(cwd, file_path);
                    let line_no = block["input"]["offset"].as_u64().unwrap_or(1) as usize;
                    events.push(ToolCall {
                        write: false,
                        path,
                        line: Some(line_no),
                        added: 0,
                        timestamp: timestamp.clone(),
                    });
                }
                "Edit" => {
                    let Some(file_path) = block["input"]["file_path"].as_str() else {
                        continue;
                    };
                    let path = resolve(cwd, file_path);
                    let old_string = block["input"]["old_string"].as_str().unwrap_or("");
                    let new_string = block["input"]["new_string"].as_str().unwrap_or("");
                    let current = std::fs::read_to_string(&path).ok();
                    let line_no = line_of(&current, new_string);
                    let added = line_count(new_string).saturating_sub(line_count(old_string));
                    events.push(ToolCall {
                        write: true,
                        path,
                        line: line_no,
                        added,
                        timestamp: timestamp.clone(),
                    });
                }
                "MultiEdit" => {
                    let Some(file_path) = block["input"]["file_path"].as_str() else {
                        continue;
                    };
                    let Some(edits) = block["input"]["edits"].as_array() else {
                        continue;
                    };
                    if edits.is_empty() {
                        continue;
                    }
                    let path = resolve(cwd, file_path);
                    let added: usize = edits
                        .iter()
                        .map(|edit| {
                            let old_string = edit["old_string"].as_str().unwrap_or("");
                            let new_string = edit["new_string"].as_str().unwrap_or("");
                            line_count(new_string).saturating_sub(line_count(old_string))
                        })
                        .sum();
                    let current = std::fs::read_to_string(&path).ok();
                    let first_new = edits[0]["new_string"].as_str().unwrap_or("");
                    let line_no = line_of(&current, first_new);
                    events.push(ToolCall {
                        write: true,
                        path,
                        line: line_no,
                        added,
                        timestamp: timestamp.clone(),
                    });
                }
                "Write" => {
                    let Some(file_path) = block["input"]["file_path"].as_str() else {
                        continue;
                    };
                    let path = resolve(cwd, file_path);
                    events.push(ToolCall {
                        write: true,
                        path,
                        line: Some(1),
                        added: 0,
                        timestamp: timestamp.clone(),
                    });
                }
                "Bash" => {
                    let Some(command) = block["input"]["command"].as_str() else {
                        continue;
                    };
                    for token in
                        command.split(|c: char| !(c.is_ascii_alphanumeric() || "_./-".contains(c)))
                    {
                        if token.is_empty() {
                            continue;
                        }
                        let Some((_, last)) = token.rsplit_once('/') else {
                            continue;
                        };
                        let Some((stem, ext)) = last.rsplit_once('.') else {
                            continue;
                        };
                        if stem.is_empty()
                            || ext.is_empty()
                            || !ext.chars().all(|c| c.is_ascii_alphanumeric())
                        {
                            continue;
                        }
                        let path = resolve(cwd, token);
                        if path.is_file() {
                            events.push(ToolCall {
                                write: false,
                                path,
                                line: None,
                                added: 0,
                                timestamp: timestamp.clone(),
                            });
                        }
                    }
                }
                "Grep" | "Glob" => {
                    let Some(path_str) = block["input"]["path"].as_str() else {
                        continue;
                    };
                    let path = resolve(cwd, path_str);
                    if path.is_file() {
                        events.push(ToolCall {
                            write: false,
                            path,
                            line: None,
                            added: 0,
                            timestamp: timestamp.clone(),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    events
}

/// `(first ~110 characters of the last assistant message that carries text,
/// flattened to single spaces; newest timestamp as epoch seconds)` across
/// `lines`. A line with `isSidechain: true` counts for neither. The text is
/// `None` when no line here carries assistant text at all - an assistant
/// message of nothing but `tool_use` blocks is the normal shape of a working
/// session, and must not blank the last thing it said - so a caller folding
/// over successive batches of new lines keeps the text it already has.
pub(crate) fn summary(lines: &[String]) -> (Option<String>, Option<u64>) {
    let mut last: Option<String> = None;
    let mut newest: Option<u64> = None;

    for line in lines {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        if value["isSidechain"] == serde_json::Value::Bool(true) {
            continue;
        }

        let this_epoch = epoch(value["timestamp"].as_str().unwrap_or(""));
        newest = newest.max(this_epoch);

        let is_assistant = value["type"] == "assistant" || value["message"]["role"] == "assistant";
        if !is_assistant {
            continue;
        }

        if let Some(content) = value["message"]["content"].as_array() {
            let text = content
                .iter()
                .filter(|block| block["type"] == "text")
                .filter_map(|block| block["text"].as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let flattened = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !flattened.is_empty() {
                last = Some(flattened.chars().take(110).collect());
            }
        }
    }

    (last, newest)
}

/// What one [`Entry`] of a read-only conversation is.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum EntryKind {
    Prompt,
    Text,
    Tool,
}

/// One line of a read-only conversation (design.md requirement 6): a
/// prompt, a piece of assistant text, or a tool call already formatted to
/// one line.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub kind: EntryKind,
    pub text: String,
}

/// The first of `input.file_path`, `input.notebook_path`, `input.command`,
/// `input.pattern` or `input.path` present on a `tool_use` block, formatted
/// as one line: whitespace-flattened and cut to 80 characters, as
/// `"{name} {arg}"` for `Read`/`Edit`/`Write`/`MultiEdit`, else
/// `"{name}: {arg}"`.
fn tool_entry(name: &str, input: &serde_json::Value) -> Entry {
    let raw = ["file_path", "notebook_path", "command", "pattern", "path"]
        .into_iter()
        .find_map(|key| input[key].as_str())
        .unwrap_or("");
    let flattened = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let arg: String = flattened.chars().take(80).collect();

    let text = if matches!(name, "Read" | "Edit" | "Write" | "MultiEdit") {
        format!("{name} {arg}")
    } else {
        format!("{name}: {arg}")
    };
    Entry {
        kind: EntryKind::Tool,
        text,
    }
}

/// A prompt or text candidate, kept only when, trimmed, it is non-empty and
/// does not start with `'<'` (system reminders and command output).
fn candidate(kind: EntryKind, text: &str) -> Option<Entry> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.starts_with('<') {
        return None;
    }
    Some(Entry {
        kind,
        text: trimmed.to_owned(),
    })
}

/// The read-only conversation `lines` carry (design.md requirement 6):
/// prompts (`"> text"` is the UI's concern, not this function's), assistant
/// text, and tool calls formatted to one line each, oldest first. A
/// sidechain line is skipped entirely; an unparsable line is skipped too -
/// neither the transcript format nor `claude agents --json` is a published
/// contract.
pub(crate) fn entries(lines: &[String]) -> Vec<Entry> {
    let mut out = Vec::new();

    for line in lines {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        if value["isSidechain"] == serde_json::Value::Bool(true) {
            continue;
        }

        let role = value["message"]["role"].as_str().unwrap_or("");
        let content = &value["message"]["content"];

        if let Some(text) = content.as_str() {
            if let Some(entry) = candidate(EntryKind::Prompt, text) {
                out.push(entry);
            }
            continue;
        }

        let Some(blocks) = content.as_array() else {
            continue;
        };

        for block in blocks {
            match block["type"].as_str() {
                Some("text") => {
                    let text = block["text"].as_str().unwrap_or("");
                    let kind = if role == "user" {
                        EntryKind::Prompt
                    } else {
                        EntryKind::Text
                    };
                    if let Some(entry) = candidate(kind, text) {
                        out.push(entry);
                    }
                }
                Some("tool_use") => {
                    let Some(name) = block["name"].as_str() else {
                        continue;
                    };
                    out.push(tool_entry(name, &block["input"]));
                }
                _ => {}
            }
        }
    }

    out
}

/// An RFC3339-ish UTC timestamp ("2026-10-19T10:00:00.123Z") to epoch
/// seconds. `None` if it doesn't parse.
pub(crate) fn epoch(ts: &str) -> Option<u64> {
    let part = |range: std::ops::Range<usize>| ts.get(range).and_then(|s| s.parse::<i64>().ok());

    let y = part(0..4)?;
    let m = part(5..7)?;
    let d = part(8..10)?;
    let h = part(11..13)?;
    let mi = part(14..16)?;
    let sec = part(17..19)?;

    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    u64::try_from(days * 86400 + h * 3600 + mi * 60 + sec).ok()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    use serde_json::json;

    use super::*;

    /// One transcript line of an assistant message holding a single
    /// `tool_use` block.
    fn tool_use_line(ts: &str, cwd: &Path, tool: &str, input: serde_json::Value) -> String {
        json!({
            "timestamp": ts,
            "cwd": cwd.display().to_string(),
            "isSidechain": false,
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "tool_use", "name": tool, "input": input}]
            }
        })
        .to_string()
    }

    /// One transcript line of a plain text message, of `role`
    /// ("assistant" or "user"), optionally on a sidechain.
    fn text_line(ts: &str, cwd: &Path, role: &str, sidechain: bool, text: &str) -> String {
        json!({
            "timestamp": ts,
            "cwd": cwd.display().to_string(),
            "isSidechain": sidechain,
            "type": role,
            "message": {
                "role": role,
                "content": [{"type": "text", "text": text}]
            }
        })
        .to_string()
    }

    /// One transcript line of a user message whose `content` is a plain
    /// string rather than an array of blocks - the shape a plain user
    /// message takes, distinct from [`text_line`]'s block form.
    fn user_string_line(ts: &str, cwd: &Path, sidechain: bool, text: &str) -> String {
        json!({
            "timestamp": ts,
            "cwd": cwd.display().to_string(),
            "isSidechain": sidechain,
            "type": "user",
            "message": {
                "role": "user",
                "content": text
            }
        })
        .to_string()
    }

    /// Pins design.md requirement 3's per-tool rules: `Read` (with and
    /// without `offset`), `Edit` (line and `added` from the file's current
    /// content on disk), `MultiEdit` (summed `added`, the first edit's
    /// line), `Write` (line 1, nothing added), `Bash` (only the token that
    /// names an existing file, among others that do not), `Grep`/`Glob`
    /// (their `path` field), an unknown tool name, and an unparsable line -
    /// none of the last three panics or produces an event.
    #[test]
    fn tool_calls_become_file_events() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let cwd = dir.path();
        fs::create_dir_all(cwd.join("catalog")).expect("create catalog/");
        fs::write(cwd.join("catalog/pricing.rs"), "stub").expect("write pricing.rs");
        fs::write(cwd.join("foo.rs"), "unrelated\nbefore\na\nb\nc\nafter\n").expect("write foo.rs");
        fs::write(cwd.join("bar.rs"), "top\nFIRST_EDIT_TARGET\nbottom\n").expect("write bar.rs");

        let mut lines = vec![
            tool_use_line("t1", cwd, "Read", json!({"file_path": "foo.rs"})),
            tool_use_line(
                "t2",
                cwd,
                "Read",
                json!({"file_path": "foo.rs", "offset": 42}),
            ),
            tool_use_line(
                "t3",
                cwd,
                "Edit",
                json!({"file_path": "foo.rs", "old_string": "x", "new_string": "a\nb\nc"}),
            ),
            tool_use_line(
                "t4",
                cwd,
                "MultiEdit",
                json!({
                    "file_path": "bar.rs",
                    "edits": [
                        {"old_string": "zzz\nyyy", "new_string": "FIRST_EDIT_TARGET"},
                        {"old_string": "", "new_string": "extra1\nextra2\nextra3"}
                    ]
                }),
            ),
            tool_use_line("t5", cwd, "Write", json!({"file_path": "baz.rs"})),
            tool_use_line(
                "t6",
                cwd,
                "Bash",
                json!({
                    "command": "grep -rn pattern catalog/pricing.rs missingfile/nothere.rs -rf"
                }),
            ),
            tool_use_line("t7", cwd, "Grep", json!({"path": "catalog/pricing.rs"})),
            tool_use_line("t8", cwd, "Glob", json!({"path": "missing/nope.rs"})),
            tool_use_line("t9", cwd, "Frobnicate", json!({"file_path": "foo.rs"})),
            text_line(
                "t10",
                cwd,
                "assistant",
                false,
                "just talking, no tool call here",
            ),
        ];
        lines.push("not json at all, must be skipped without panicking".to_owned());

        let events = tool_events(&lines);
        let find = |ts: &str| events.iter().find(|event| event.timestamp == ts);

        let read_default = find("t1").expect("a plain Read is kept");
        assert!(!read_default.write);
        assert_eq!(read_default.path, cwd.join("foo.rs"));
        assert_eq!(read_default.line, Some(1), "offset defaults to 1");
        assert_eq!(read_default.added, 0);

        let read_offset = find("t2").expect("a Read with an offset is kept");
        assert_eq!(read_offset.line, Some(42));

        let edit = find("t3").expect("the Edit is kept");
        assert!(edit.write);
        assert_eq!(edit.path, cwd.join("foo.rs"));
        assert_eq!(
            edit.line,
            Some(3),
            "new_string sits on line 3 of the file's current content"
        );
        assert_eq!(edit.added, 2, "3 lines of new_string less 1 of old_string");

        let multi = find("t4").expect("the MultiEdit is kept");
        assert!(multi.write);
        assert_eq!(multi.path, cwd.join("bar.rs"));
        assert_eq!(
            multi.line,
            Some(2),
            "the first edit's new_string sits on line 2"
        );
        assert_eq!(
            multi.added, 3,
            "0 from the first edit (floored, it removes a line) plus 3 from the second"
        );

        let write = find("t5").expect("the Write is kept");
        assert!(write.write);
        assert_eq!(write.path, cwd.join("baz.rs"));
        assert_eq!(write.line, Some(1));
        assert_eq!(write.added, 0);

        let bash_events: Vec<_> = events
            .iter()
            .filter(|event| event.timestamp == "t6")
            .collect();
        assert_eq!(
            bash_events.len(),
            1,
            "only the one token that names an existing file becomes an event: {bash_events:?}"
        );
        assert!(!bash_events[0].write);
        assert_eq!(bash_events[0].path, cwd.join("catalog/pricing.rs"));
        assert_eq!(bash_events[0].line, None);

        let grep = find("t7").expect("Grep's path names an existing file");
        assert!(!grep.write);
        assert_eq!(grep.path, cwd.join("catalog/pricing.rs"));
        assert_eq!(grep.line, None);

        assert!(
            find("t8").is_none(),
            "Glob's path does not resolve to an existing file"
        );
        assert!(find("t9").is_none(), "an unknown tool name is ignored");
        assert!(find("t10").is_none(), "a text block is not a tool call");
        assert_eq!(events.len(), 7, "{events:?}");
    }

    /// Pins the tailing rule: an incomplete (no trailing newline) line is
    /// never returned, only the lines completed since the last read are,
    /// and a file that shrank below the tracked offset is read again from
    /// the start.
    #[test]
    fn a_tail_reads_only_new_complete_lines() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("transcript.jsonl");
        fs::write(&path, "incomplete line without newline").expect("write the fixture");
        let mut tail = Tail::new();

        assert!(
            tail.read(&path).is_empty(),
            "an incomplete line is not returned yet"
        );

        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open for appending");
        write!(file, "\nmore text without a newline").expect("append more content");
        drop(file);

        assert_eq!(
            tail.read(&path),
            vec!["incomplete line without newline".to_owned()],
            "exactly the line that was completed"
        );

        fs::write(&path, "short\n").expect("truncate and write fresh content");

        assert_eq!(
            tail.read(&path),
            vec!["short".to_owned()],
            "a file shorter than the tracked offset is read again from the start"
        );
    }

    /// Pins [`summary`]: the text half is the last assistant message that
    /// carries text (not the first, never a sidechain one, and never blanked
    /// by the tool-only assistant messages a working session is mostly made
    /// of), trimmed to about 110 characters with its newlines flattened to
    /// single spaces; the time half is the newest timestamp of any line
    /// (sidechains excluded), whether or not that line is itself an
    /// assistant message.
    #[test]
    fn the_summary_is_the_last_assistant_text_and_newest_time() {
        let cwd = Path::new("/repo");
        let lines = vec![
            text_line(
                "2026-10-19T10:00:00.000Z",
                cwd,
                "assistant",
                false,
                "FIRST RESPONSE MARKER, must not be the summary",
            ),
            text_line(
                "2026-10-19T10:05:00.000Z",
                cwd,
                "user",
                false,
                "A question from the engineer, never the summary",
            ),
            text_line(
                "2026-10-19T12:00:00.000Z",
                cwd,
                "assistant",
                true,
                "SIDECHAIN MARKER must never surface, and its late timestamp must not count",
            ),
            text_line(
                "2026-10-19T10:10:00.000Z",
                cwd,
                "assistant",
                false,
                "Line one of the answer.\nLine two continues here with more detail, well past \
                 the hundred and ten character mark the summary trims to, to prove the trimming \
                 really happens here.",
            ),
            tool_use_line(
                "2026-10-19T10:15:00.000Z",
                cwd,
                "Read",
                json!({"file_path": "foo.rs"}),
            ),
            text_line(
                "2026-10-19T10:20:00.000Z",
                cwd,
                "user",
                false,
                "A later question: the newest timestamp, but not the summary's text",
            ),
        ];

        let (text, latest) = summary(&lines);
        let text = text.expect("these lines carry assistant text");

        assert!(
            !text.contains('\n'),
            "newlines are flattened to single spaces: {text:?}"
        );
        assert!(
            text.starts_with("Line one of the answer. Line two continues"),
            "the last assistant message that carries text, not the first, and not \
             blanked by the later tool-only assistant message: {text:?}"
        );
        assert!(
            !text.contains("FIRST RESPONSE MARKER"),
            "not the first assistant message: {text:?}"
        );
        assert!(
            !text.contains("SIDECHAIN MARKER"),
            "a sidechain message is ignored even though it is the latest assistant one: {text:?}"
        );
        assert!(
            text.chars().count() <= 115,
            "trimmed to about 110 characters: {text:?}"
        );
        assert_eq!(
            latest,
            epoch("2026-10-19T10:20:00.000Z"),
            "the newest timestamp of any line, not only assistant ones, sidechains excluded"
        );

        let only_a_tool_call = vec![tool_use_line(
            "2026-10-19T10:15:00.000Z",
            cwd,
            "Read",
            json!({"file_path": "foo.rs"}),
        )];
        let (none, still_timed) = summary(&only_a_tool_call);
        assert_eq!(
            none, None,
            "a batch of nothing but tool-only assistant messages carries no text, \
             so a caller folding over new lines keeps the text it already has"
        );
        assert_eq!(
            still_timed,
            epoch("2026-10-19T10:15:00.000Z"),
            "it still counts for the time half"
        );
    }

    /// Pins [`epoch`]: it parses the transcript's timestamp shape, orders
    /// timestamps the way their text does, and reports `None` for anything
    /// else.
    #[test]
    fn epoch_parses_timestamps_and_orders_them() {
        let earlier = epoch("2026-10-19T10:00:00.123Z").expect("a valid timestamp parses");
        let later = epoch("2026-10-19T10:00:01.000Z").expect("a valid timestamp parses");

        assert!(later > earlier, "{earlier} vs {later}");
        assert_eq!(epoch("not a timestamp"), None);
    }

    /// Pins [`entries`] (design.md requirement 6 and the Proof section): a
    /// plain-string user message and a user `text` block both become a
    /// prompt, an assistant `text` block becomes text, a `Read` tool call
    /// becomes one line "Read <path>", another tool (`Bash`) becomes
    /// "Bash: <command>" cut to 80 characters, a sidechain line is skipped
    /// entirely whatever it carries, and an assistant `text` block starting
    /// with "<" or carrying no text is skipped too (system reminders and
    /// command output, and the tool-only assistant messages a working
    /// session is mostly made of).
    #[test]
    fn the_conversation_is_prompts_text_and_one_line_tool_calls() {
        let cwd = Path::new("/repo");
        let long_command = "x".repeat(200);
        let lines = vec![
            user_string_line("t1", cwd, false, "Plain string content becomes a prompt"),
            text_line(
                "t2",
                cwd,
                "user",
                false,
                "A user text block is also a prompt",
            ),
            text_line(
                "t3",
                cwd,
                "assistant",
                false,
                "Assistant text becomes a Text entry",
            ),
            tool_use_line("t4", cwd, "Read", json!({"file_path": "foo.rs"})),
            tool_use_line("t5", cwd, "Bash", json!({"command": long_command})),
            text_line(
                "t6",
                cwd,
                "assistant",
                true,
                "SIDECHAIN MARKER must never surface",
            ),
            text_line(
                "t7",
                cwd,
                "assistant",
                false,
                "<system-reminder>skipped, starts with '<'</system-reminder>",
            ),
            text_line("t8", cwd, "assistant", false, ""),
        ];

        let entries = entries(&lines);

        let prompts: Vec<&Entry> = entries
            .iter()
            .filter(|e| e.kind == EntryKind::Prompt)
            .collect();
        assert_eq!(prompts.len(), 2, "{entries:?}");
        assert_eq!(prompts[0].text, "Plain string content becomes a prompt");
        assert_eq!(prompts[1].text, "A user text block is also a prompt");

        let texts: Vec<&Entry> = entries
            .iter()
            .filter(|e| e.kind == EntryKind::Text)
            .collect();
        assert_eq!(
            texts.len(),
            1,
            "the sidechain line, the '<'-prefixed line and the empty line are all \
             skipped: {entries:?}"
        );
        assert_eq!(texts[0].text, "Assistant text becomes a Text entry");

        let tools: Vec<&Entry> = entries
            .iter()
            .filter(|e| e.kind == EntryKind::Tool)
            .collect();
        assert_eq!(tools.len(), 2, "{entries:?}");
        assert_eq!(
            tools[0].text, "Read foo.rs",
            "Read/Edit/Write/MultiEdit format as \"{{name}} {{arg}}\""
        );
        assert_eq!(
            tools[1].text,
            format!("Bash: {}", "x".repeat(80)),
            "another tool formats as \"{{name}}: {{arg}}\", its argument cut to 80 characters"
        );

        assert!(
            !entries.iter().any(|e| e.text.contains("SIDECHAIN")),
            "a sidechain line is skipped whatever kind it would otherwise be: {entries:?}"
        );
    }

    /// Pins [`Tail::from_end`]: seeded this way, the next `read` returns only
    /// the lines completed within the last `max_bytes` of the file - so a
    /// freshly-opened pane does not reparse a whole transcript's history
    /// just to show its last ~80 entries - and falls back to the whole file
    /// when it is shorter than `max_bytes`.
    #[test]
    fn from_end_seeds_reading_only_the_recent_tail_of_a_file() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("transcript.jsonl");
        let kept = "kept-line\n";
        // Far more history than `max_bytes` below, so the seeded offset
        // lands exactly at the start of `kept`, not inside the old content.
        let old = "stale ".repeat(1000);
        fs::write(&path, format!("{old}{kept}")).expect("write the fixture");

        let mut tail = Tail::from_end(&path, kept.len() as u64);
        assert_eq!(
            tail.read(&path),
            vec!["kept-line".to_owned()],
            "only the line completed within the last max_bytes is read"
        );

        let short_path = dir.path().join("short.jsonl");
        fs::write(&short_path, "only-line\n").expect("write the short fixture");
        let mut short_tail = Tail::from_end(&short_path, 1_000_000);
        assert_eq!(
            short_tail.read(&short_path),
            vec!["only-line".to_owned()],
            "a file shorter than max_bytes is read from the start"
        );
    }
}
