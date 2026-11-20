//! HTTP through `curl` and the small process helpers the ticket sources
//! share (ADR-0003). No shell is involved; the URL, the headers and the body
//! reach curl through its config on stdin and a private file, so no token
//! shows in the process list.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(super) fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// What curl got back.
pub(super) struct Response {
    pub status: u32,
    pub head: String,
    pub body: String,
}

/// A value for a curl config line: quotes, backslashes and newlines escaped.
fn quoted(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// One request through curl. `body` makes it a POST of `content_type`;
/// without one it is a GET. A refused connection or a timeout is an `Err`.
pub(super) fn request(
    url: &str,
    headers: &[(&str, &str)],
    body: Option<(&str, &str)>,
    max_seconds: u32,
) -> Result<Response, String> {
    let mut config = format!(
        "url = \"{}\"\nsilent\ninclude\nconnect-timeout = 5\nmax-time = {max_seconds}\n",
        quoted(url)
    );
    config += &match body {
        Some(_) => "request = \"POST\"\n".to_owned(),
        None => "request = \"GET\"\n".to_owned(),
    };
    for (name, value) in headers {
        config += &format!("header = \"{}\"\n", quoted(&format!("{name}: {value}")));
    }
    let mut tmp = None;
    if let Some((content_type, body)) = body {
        config += &format!("header = \"Content-Type: {}\"\n", quoted(content_type));
        let dir = home().join(".cache/cellworks");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("req-{}.tmp", random_hex(8)));
        write_private(&path, body)?;
        config += &format!(
            "data-binary = \"@{}\"\n",
            quoted(&path.display().to_string())
        );
        tmp = Some(path);
    }
    let run = || -> Result<String, String> {
        let mut child = Command::new("curl")
            .args(["-K", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("curl: {e}"))?;
        child
            .stdin
            .take()
            .ok_or("curl: no stdin")?
            .write_all(config.as_bytes())
            .map_err(|e| e.to_string())?;
        let mut out = String::new();
        child
            .stdout
            .take()
            .ok_or("curl: no stdout")?
            .read_to_string(&mut out)
            .map_err(|e| e.to_string())?;
        child.wait().map_err(|e| e.to_string())?;
        Ok(out)
    };
    let out = run();
    if let Some(path) = tmp {
        let _ = std::fs::remove_file(path);
    }
    parse(&out?).ok_or_else(|| format!("no response from {url}"))
}

/// Splits curl's `--include` output into status, headers and body, skipping
/// "100 Continue": the last header block is the response's.
fn parse(out: &str) -> Option<Response> {
    let mut rest = out;
    loop {
        let (head, body) = rest.split_once("\r\n\r\n")?;
        let status: u32 = head
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if status == 100 {
            rest = body;
            continue;
        }
        return Some(Response {
            status,
            head: head.to_owned(),
            body: body.to_owned(),
        });
    }
}

/// POSTs `body` and returns (status, headers, body).
pub(super) fn post(
    url: &str,
    headers: &[(&str, &str)],
    content_type: &str,
    body: &str,
) -> Result<(u32, String, String), String> {
    let r = request(url, headers, Some((content_type, body)), 60)?;
    Ok((r.status, r.head, r.body))
}

pub(super) fn write_private(path: &Path, text: &str) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .map_err(|e| e.to_string())
}

pub(super) fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    if let Ok(mut file) = std::fs::File::open("/dev/urandom") {
        let _ = file.read_exact(&mut buf);
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Runs `program args` and answers its stdout; its stderr is the error when
/// it fails. The program is looked up on PATH, so a missing one reads as
/// "not installed".
pub(super) fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let message = String::from_utf8_lossy(&out.stderr);
        Err(format!(
            "{program}: {}",
            message.trim().chars().take(300).collect::<String>()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_header_block_is_the_response() {
        let r = parse("HTTP/1.1 100 Continue\r\n\r\nHTTP/2 200\r\nx: y\r\n\r\n{\"a\":1}").unwrap();
        assert_eq!((r.status, r.body.as_str()), (200, "{\"a\":1}"));
    }

    #[test]
    fn config_values_cannot_break_out_of_their_quotes() {
        assert_eq!(quoted("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
    }
}
