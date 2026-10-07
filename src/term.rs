// Detachable terminal sessions, like dtach/tmux: each session runs in its own host process (`cellworks --host <id>`)
// that owns the pty and listens on a Unix socket. The app attaches and detaches; sessions outlive the app.
//
// Wire format, client to host: [kind u8][len u32 BE][payload]; kind 0 = input bytes, 1 = resize (cols u16, rows u16).
// Host to client: the raw output buffer since start (bounded), then the live output stream.
use eframe::egui::{self, Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, pos2, vec2};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

const BUFFER: usize = 2_000_000;

#[derive(Serialize, Deserialize, Clone)]
pub struct Meta {
    /// the Claude session id the host runs
    pub id: String,
    pub cwd: String,
    pub cmd: Vec<String>,
    pub pid: u32,
}

pub fn dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache/cellworks/sessions")
}

fn sock(id: &str) -> PathBuf {
    dir().join(format!("{id}.sock"))
}

fn meta_path(id: &str) -> PathBuf {
    dir().join(format!("{id}.json"))
}

/// Sessions with a live host.
pub fn list() -> Vec<Meta> {
    let Ok(rd) = std::fs::read_dir(dir()) else { return vec![] };
    rd.flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_str::<Meta>(&std::fs::read_to_string(e.path()).ok()?).ok())
        .filter(|m| UnixStream::connect(sock(&m.id)).is_ok())
        .collect()
}

/// Starts a detached host running `cmd` in `cwd` for session `id`.
pub fn spawn(id: &str, cwd: &str, cmd: &[String]) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    std::fs::create_dir_all(dir())?;
    let exe = std::env::current_exe()?;
    std::process::Command::new(exe)
        .arg("--host").arg(id).arg(cwd).args(cmd)
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()?;
    // wait briefly for the socket so the caller can attach right away
    for _ in 0..50 {
        if UnixStream::connect(sock(id)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    Err(std::io::Error::other("session host did not start"))
}

/// Host process body: runs until the program in the pty exits.
pub fn run_host(id: &str, cwd: &str, cmd: &[String]) -> anyhow::Result<()> {
    let pair = native_pty_system().openpty(PtySize { rows: 40, cols: 120, pixel_width: 0, pixel_height: 0 })?;
    let mut cb = CommandBuilder::new(&cmd[0]);
    cb.args(&cmd[1..]);
    cb.cwd(cwd);
    cb.env("TERM", "xterm-256color");
    cb.env("COLORTERM", "truecolor");
    let mut child = pair.slave.spawn_command(cb)?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader()?;
    let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
    let master = Arc::new(Mutex::new(pair.master));
    let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
    let clients: Arc<Mutex<Vec<UnixStream>>> = Arc::new(Mutex::new(vec![]));
    let _ = std::fs::remove_file(sock(id));
    let listener = UnixListener::bind(sock(id))?;
    std::fs::write(meta_path(id), serde_json::to_string(&Meta { id: id.into(), cwd: cwd.into(), cmd: cmd.to_vec(), pid: std::process::id() })?)?;

    // pty output: keep a bounded buffer for late attachers and fan out to every attached client
    {
        let (buffer, clients) = (buffer.clone(), clients.clone());
        std::thread::spawn(move || {
            let mut chunk = [0u8; 65536];
            while let Ok(n) = reader.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                let mut b = buffer.lock().unwrap();
                b.extend_from_slice(&chunk[..n]);
                if b.len() > BUFFER {
                    let cut = b.len() - BUFFER;
                    b.drain(..cut);
                }
                clients.lock().unwrap().retain_mut(|c| c.write_all(&chunk[..n]).is_ok());
            }
        });
    }
    // attaching clients: replay, then read their input and resize frames
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let mut out = match conn.try_clone() {
                Ok(c) => c,
                Err(_) => continue,
            };
            {
                let b = buffer.lock().unwrap();
                if out.write_all(&b).is_err() {
                    continue;
                }
                clients.lock().unwrap().push(out);
            }
            let (writer, master) = (writer.clone(), master.clone());
            let mut conn = conn;
            std::thread::spawn(move || {
                let mut head = [0u8; 5];
                while conn.read_exact(&mut head).is_ok() {
                    let len = u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize;
                    let mut body = vec![0u8; len];
                    if conn.read_exact(&mut body).is_err() {
                        break;
                    }
                    match head[0] {
                        0 => {
                            let _ = writer.lock().unwrap().write_all(&body);
                        }
                        1 if len == 4 => {
                            let cols = u16::from_be_bytes([body[0], body[1]]);
                            let rows = u16::from_be_bytes([body[2], body[3]]);
                            let _ = master.lock().unwrap().resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
                        }
                        _ => {}
                    }
                }
            });
        }
    });
    let _ = child.wait();
    let _ = std::fs::remove_file(sock(id));
    let _ = std::fs::remove_file(meta_path(id));
    Ok(())
}

/// Ends a session by stopping its host's program.
pub fn kill(id: &str) {
    if let Some(m) = std::fs::read_to_string(meta_path(id)).ok().and_then(|t| serde_json::from_str::<Meta>(&t).ok()) {
        let _ = std::process::Command::new("pkill").arg("-TERM").arg("-P").arg(m.pid.to_string()).status();
    }
}

/// An attached session: output parsed into a screen, input and resizes sent to the host.
pub struct Term {
    pub cwd: String,
    stream: UnixStream,
    rx: Receiver<Vec<u8>>,
    pub parser: vt100::Parser,
    pub alive: bool,
    size: (u16, u16),
    /// rows scrolled back into history
    pub scroll: usize,
}

impl Term {
    pub fn attach(id: &str, cwd: &str, repaint: egui::Context) -> std::io::Result<Self> {
        let stream = UnixStream::connect(sock(id))?;
        let mut read = stream.try_clone()?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 65536];
            while let Ok(n) = read.read(&mut chunk) {
                if n == 0 || tx.send(chunk[..n].to_vec()).is_err() {
                    break;
                }
                repaint.request_repaint();
            }
        });
        Ok(Term { cwd: cwd.into(), stream, rx, parser: vt100::Parser::new(40, 120, 5000), alive: true, size: (0, 0), scroll: 0 })
    }

    pub fn pump(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(b) => self.parser.process(&b),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(_) => {
                    self.alive = false;
                    break;
                }
            }
        }
    }

    fn frame(&mut self, kind: u8, body: &[u8]) {
        let mut f = vec![kind];
        f.extend_from_slice(&(body.len() as u32).to_be_bytes());
        f.extend_from_slice(body);
        if self.stream.write_all(&f).is_err() {
            self.alive = false;
        }
    }

    pub fn input(&mut self, bytes: &[u8]) {
        self.scroll = 0;
        self.frame(0, bytes);
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        if (cols, rows) != self.size && cols > 10 && rows > 3 {
            self.size = (cols, rows);
            self.parser.screen_mut().set_size(rows, cols);
            let mut b = cols.to_be_bytes().to_vec();
            b.extend_from_slice(&rows.to_be_bytes());
            self.frame(1, &b);
        }
    }

    /// The permission question on screen, when the CLI is waiting for an answer.
    pub fn waiting_question(&self) -> Option<String> {
        let text = self.parser.screen().contents();
        let lines: Vec<&str> = text.lines().collect();
        let i = lines.iter().rposition(|l| l.contains("Do you want to") || l.contains("Would you like to"))?;
        // only when the options are still showing below it
        lines[i..].iter().any(|l| l.contains("1.") || l.contains("Yes")).then(|| lines[i].trim().trim_start_matches(['│', '╭', '|']).trim().to_string())
    }
}

/// Bytes a key press sends to a terminal program.
pub fn key_bytes(key: egui::Key, m: egui::Modifiers, app_cursor: bool) -> Option<Vec<u8>> {
    use egui::Key::*;
    let arrow = |c: char| if app_cursor { format!("\x1bO{c}") } else { format!("\x1b[{c}") };
    let s = match key {
        Enter => "\r".to_string(),
        Tab if m.shift => "\x1b[Z".into(),
        Tab => "\t".into(),
        Backspace => "\x7f".into(),
        Escape => "\x1b".into(),
        ArrowUp => arrow('A'),
        ArrowDown => arrow('B'),
        ArrowRight => arrow('C'),
        ArrowLeft => arrow('D'),
        Home => "\x1b[H".into(),
        End => "\x1b[F".into(),
        Delete => "\x1b[3~".into(),
        PageUp => "\x1b[5~".into(),
        PageDown => "\x1b[6~".into(),
        _ if m.ctrl => {
            let name = key.name();
            let c = name.chars().next()?;
            if name.len() == 1 && c.is_ascii_alphabetic() {
                return Some(vec![(c.to_ascii_lowercase() as u8) & 0x1f]);
            }
            return None;
        }
        _ => return None,
    };
    Some(s.into_bytes())
}

fn ansi(i: u8) -> Color32 {
    const BASE: [(u8, u8, u8); 16] = [
        (0x1d, 0x1b, 0x18), (0xd0, 0x5a, 0x4a), (0x8a, 0xc0, 0x6a), (0xe0, 0xb0, 0x50), (0x6a, 0x9a, 0xd8), (0xc0, 0x8a, 0xd0), (0x5a, 0xb8, 0xb0), (0xd8, 0xcc, 0xb4),
        (0x5a, 0x55, 0x4c), (0xf0, 0x7a, 0x6a), (0xa8, 0xe0, 0x8a), (0xf8, 0xd0, 0x70), (0x8a, 0xba, 0xf0), (0xe0, 0xaa, 0xf0), (0x7a, 0xd8, 0xd0), (0xf4, 0xec, 0xdc),
    ];
    match i {
        0..=15 => {
            let (r, g, b) = BASE[i as usize];
            Color32::from_rgb(r, g, b)
        }
        16..=231 => {
            let i = i - 16;
            let lvl = |x: u8| if x == 0 { 0 } else { 55 + x * 40 };
            Color32::from_rgb(lvl(i / 36), lvl((i / 6) % 6), lvl(i % 6))
        }
        _ => {
            let g = 8 + (i - 232) * 10;
            Color32::from_gray(g)
        }
    }
}

fn color(c: vt100::Color, default: Color32) -> Color32 {
    match c {
        vt100::Color::Default => default,
        vt100::Color::Idx(i) => ansi(i),
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    }
}

pub const FG: Color32 = Color32::from_rgb(0xe8, 0xdc, 0xc0);
pub const BG: Color32 = Color32::from_rgb(0x12, 0x10, 0x0d);

/// Paints the screen into `area`, one text run per stretch of equal style.
pub fn paint(p: &Painter, t: &mut Term, area: Rect, font: &FontId, cw: f32, lh: f32, focused: bool) {
    p.rect_filled(area, 6.0, BG);
    let p = p.with_clip_rect(area);
    t.parser.screen_mut().set_scrollback(t.scroll);
    let screen = t.parser.screen();
    let (rows, cols) = screen.size();
    for r in 0..rows {
        let y = area.min.y + 4.0 + r as f32 * lh;
        let mut c = 0;
        while c < cols {
            let Some(cell) = screen.cell(r, c) else { break };
            let style = (cell.fgcolor(), cell.bgcolor(), cell.bold(), cell.inverse());
            let start = c;
            let mut text = String::new();
            while c < cols {
                let Some(x) = screen.cell(r, c) else { break };
                if (x.fgcolor(), x.bgcolor(), x.bold(), x.inverse()) != style {
                    break;
                }
                if !x.is_wide_continuation() {
                    text.push_str(if x.has_contents() { x.contents() } else { " " });
                }
                c += 1;
            }
            let (mut fg, mut bg) = (color(style.0, FG), color(style.1, BG));
            if style.3 {
                std::mem::swap(&mut fg, &mut bg);
            }
            let x0 = area.min.x + 4.0 + start as f32 * cw;
            if bg != BG {
                p.rect_filled(Rect::from_min_size(pos2(x0, y), vec2((c - start) as f32 * cw, lh)), 0.0, bg);
            }
            if !text.trim().is_empty() {
                p.text(pos2(x0, y + 1.0), Align2::LEFT_TOP, text, font.clone(), if style.2 { fg } else { fg.gamma_multiply(0.92) });
            }
        }
    }
    if !screen.hide_cursor() && t.scroll == 0 {
        let (cr, cc) = screen.cursor_position();
        let cur = Rect::from_min_size(area.min + vec2(4.0 + cc as f32 * cw, 4.0 + cr as f32 * lh), vec2(cw, lh));
        if focused {
            p.rect_filled(cur, 0.0, FG.gamma_multiply(0.6));
        } else {
            p.rect_stroke(cur, 0.0, Stroke::new(1.0, FG.gamma_multiply(0.6)), StrokeKind::Inside);
        }
    }
    if t.scroll > 0 {
        p.text(area.right_top() + vec2(-8.0, 6.0), Align2::RIGHT_TOP, format!("history -{} rows", t.scroll), FontId::proportional(11.0), FG.gamma_multiply(0.6));
    }
}
