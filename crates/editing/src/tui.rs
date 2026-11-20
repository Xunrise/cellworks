//! Another terminal editor than Neovim (Helix, Vim, Micro, ...), run behind a
//! pseudo-terminal and kept as a `vt100` screen the editor paints.

use egui::{Align2, Color32, FontId, Rect, Stroke, StrokeKind, pos2, vec2};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

/// A terminal editor running on one file.
pub struct Tui {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    output: Receiver<Vec<u8>>,
    parser: vt100::Parser,
    pub path: PathBuf,
    rows: u16,
    cols: u16,
}

/// The arguments that open `path` at `line` in `command`: `file:line` for
/// Helix, `+line file` for the editors that follow vi.
pub fn arguments(command: &str, path: &Path, line: usize) -> Vec<String> {
    let program = command.split_whitespace().next().unwrap_or("");
    let name = Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let file = path.display().to_string();
    match name {
        "hx" | "helix" => vec![format!("{file}:{line}")],
        _ => vec![format!("+{line}"), file],
    }
}

impl Tui {
    /// Starts `command` (a program and its flags, as a shell reads them) on
    /// `path` at `line` in a terminal `cols` by `rows` cells.
    pub fn spawn(
        command: &str,
        path: &Path,
        line: usize,
        cols: usize,
        rows: usize,
        repaint: impl Fn() + Send + 'static,
    ) -> Result<Tui, String> {
        let (rows, cols) = (rows.clamp(1, 500) as u16, cols.clamp(1, 500) as u16);
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("no pseudo-terminal: {error}"))?;
        // Through the shell, so the command may carry its own flags.
        let mut builder = CommandBuilder::new("/bin/sh");
        builder.arg("-c");
        builder.arg(format!("{command} \"$@\""));
        builder.arg("sh");
        for argument in arguments(command, path, line) {
            builder.arg(argument);
        }
        builder.env("TERM", "xterm-256color");
        builder.env("COLORTERM", "truecolor");
        if let Some(dir) = path.parent() {
            builder.cwd(dir);
        }
        let child = pair
            .slave
            .spawn_command(builder)
            .map_err(|error| format!("{command} would not start: {error}"))?;
        drop(pair.slave);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| error.to_string())?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| error.to_string())?;
        let (sender, output) = channel();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buffer) {
                if n == 0 || sender.send(buffer[..n].to_vec()).is_err() {
                    return;
                }
                repaint();
            }
        });
        Ok(Tui {
            master: pair.master,
            writer,
            child,
            output,
            parser: vt100::Parser::new(nz(rows), nz(cols), 0),
            path: path.to_path_buf(),
            rows,
            cols,
        })
    }

    /// Takes what the editor wrote since the last call.
    pub fn pump(&mut self) {
        while let Ok(bytes) = self.output.try_recv() {
            self.parser.process(&bytes);
        }
    }

    /// Whether the editor still runs.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn send(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// Resizes the terminal to `cols` by `rows`.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let (rows, cols) = (rows.clamp(1, 500) as u16, cols.clamp(1, 500) as u16);
        if (rows, cols) != (self.rows, self.cols) {
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
    }

    /// Paints the screen into `area`, `cw` by `lh` points a cell.
    pub fn paint(&self, ui: &egui::Ui, area: Rect, font: &FontId, cw: f32, lh: f32, focused: bool) {
        const INK: Color32 = Color32::from_rgb(0xd8, 0xde, 0xe9);
        const GROUND: Color32 = Color32::from_rgb(0x0b, 0x10, 0x16);
        let painter = ui.painter().with_clip_rect(area);
        painter.rect_filled(area, 0.0, GROUND);
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        let (rows, cols) = (rows.get(), cols.get());
        for row in 0..rows {
            let y = area.min.y + row as f32 * lh;
            for col in 0..cols {
                let Some(cell) = screen.cell(row, col) else {
                    continue;
                };
                let (mut fg, mut bg) =
                    (colour(cell.fgcolor(), INK), colour(cell.bgcolor(), GROUND));
                if cell.inverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }
                let at = pos2(area.min.x + col as f32 * cw, y);
                if bg != GROUND {
                    painter.rect_filled(Rect::from_min_size(at, vec2(cw, lh)), 0.0, bg);
                }
                let text = cell.contents();
                if !text.trim().is_empty() {
                    painter.text(at, Align2::LEFT_TOP, text, font.clone(), fg);
                }
            }
        }
        if !screen.hide_cursor() {
            let (row, col) = screen.cursor_position();
            let cursor = Rect::from_min_size(
                area.min + vec2(col as f32 * cw, row as f32 * lh),
                vec2(cw, lh),
            );
            if focused {
                painter.rect_filled(cursor, 0.0, INK.gamma_multiply(0.5));
            } else {
                painter.rect_stroke(cursor, 0.0, Stroke::new(1.0, INK), StrokeKind::Inside);
            }
        }
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn nz(n: u16) -> std::num::NonZeroU16 {
    std::num::NonZeroU16::new(n.max(1)).expect("at least one")
}

fn colour(colour: vt100::Color, default: Color32) -> Color32 {
    match colour {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        vt100::Color::Idx(n) => xterm(n),
    }
}

/// The xterm 256-colour palette.
fn xterm(n: u8) -> Color32 {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 49, 49),
        (13, 188, 121),
        (229, 229, 16),
        (36, 114, 200),
        (188, 63, 188),
        (17, 168, 205),
        (229, 229, 229),
        (102, 102, 102),
        (241, 76, 76),
        (35, 209, 139),
        (245, 245, 67),
        (59, 142, 234),
        (214, 112, 214),
        (41, 184, 219),
        (255, 255, 255),
    ];
    match n {
        0..16 => {
            let (r, g, b) = BASE[n as usize];
            Color32::from_rgb(r, g, b)
        }
        16..232 => {
            let k = n - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            Color32::from_rgb(level(k / 36), level(k / 6 % 6), level(k % 6))
        }
        _ => Color32::from_gray(8 + 10 * (n - 232)),
    }
}

/// The bytes one egui input event sends a terminal editor.
pub fn key_bytes(event: &egui::Event) -> Vec<u8> {
    use egui::Key::*;
    match event {
        egui::Event::Text(text) | egui::Event::Paste(text) => text.as_bytes().to_vec(),
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } if !modifiers.mac_cmd => match key {
            Enter => b"\r".to_vec(),
            Backspace => vec![0x7f],
            Tab => b"\t".to_vec(),
            Escape => vec![0x1b],
            ArrowUp => b"\x1b[A".to_vec(),
            ArrowDown => b"\x1b[B".to_vec(),
            ArrowRight => b"\x1b[C".to_vec(),
            ArrowLeft => b"\x1b[D".to_vec(),
            Home => b"\x1b[H".to_vec(),
            End => b"\x1b[F".to_vec(),
            PageUp => b"\x1b[5~".to_vec(),
            PageDown => b"\x1b[6~".to_vec(),
            Delete => b"\x1b[3~".to_vec(),
            _ => {
                let name = key.name().as_bytes();
                if modifiers.ctrl && name.len() == 1 && name[0].is_ascii_uppercase() {
                    vec![name[0] - b'A' + 1]
                } else {
                    Vec::new()
                }
            }
        },
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_editor_is_told_the_line_its_own_way() {
        let file = Path::new("/p/a.rs");
        assert_eq!(arguments("hx", file, 7), ["/p/a.rs:7"]);
        assert_eq!(
            arguments("/usr/bin/vim -u NONE", file, 7),
            ["+7", "/p/a.rs"]
        );
    }

    #[test]
    fn a_terminal_editor_runs_in_its_own_terminal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "hello\n").unwrap();
        // `cat` stands in for an editor: it prints the file it was given.
        let mut tui = Tui::spawn("cat", &file, 1, 40, 5, || {}).unwrap();
        let started = std::time::Instant::now();
        while !tui.parser.screen().contents().contains("hello") && started.elapsed().as_secs() < 5 {
            tui.pump();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(tui.parser.screen().contents().contains("hello"));
    }
}
