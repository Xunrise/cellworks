//! A Neovim embedded with `nvim --embed`: a msgpack-RPC UI client
//! (`ext_linegrid`) whose screen grid the editor paints.

use rmpv::Value;
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};

/// A highlight Neovim defined for its grid.
#[derive(Clone, Copy, Default)]
pub struct Highlight {
    pub fg: Option<u32>,
    pub bg: Option<u32>,
    pub reverse: bool,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// A running Neovim and its screen as last redrawn.
pub struct Nvim {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    redraws: Receiver<Value>,
    request: u64,
    pub path: PathBuf,
    pub cols: usize,
    pub rows: usize,
    /// The grid's rows of (text, highlight id).
    pub cells: Vec<Vec<(String, u64)>>,
    pub highlights: HashMap<u64, Highlight>,
    pub fg: u32,
    pub bg: u32,
    /// The cursor's (row, column).
    pub cursor: (usize, usize),
    /// The mode's name: "normal", "insert", "visual", ...
    pub mode: String,
}

fn number(value: &Value) -> u64 {
    value.as_u64().unwrap_or(0)
}

/// Whether `nvim` can be started.
pub fn available() -> bool {
    Command::new("nvim")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

impl Nvim {
    /// Starts Neovim on `path` at `line`, `cols` by `rows` cells, with the
    /// user's own configuration unless `config` names another; `repaint` is
    /// called whenever Neovim has redrawn.
    pub fn spawn(
        path: &Path,
        config: Option<&Path>,
        line: usize,
        cols: usize,
        rows: usize,
        repaint: impl Fn() + Send + 'static,
    ) -> std::io::Result<Self> {
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut command = Command::new("nvim");
        command.args(["--embed", "-n"]);
        if let Some(config) = config {
            command.arg("-u").arg(config);
        }
        let mut child = command
            .arg(format!("+{}", line.max(1)))
            .arg(path)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = BufWriter::new(child.stdin.take().expect("piped stdin"));
        let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let (sender, redraws) = channel();
        std::thread::spawn(move || {
            while let Ok(value) = rmpv::decode::read_value(&mut stdout) {
                if sender.send(value).is_err() {
                    break;
                }
                repaint();
            }
        });
        let mut nvim = Nvim {
            child,
            stdin,
            redraws,
            request: 0,
            path: path.to_path_buf(),
            cols,
            rows,
            cells: Vec::new(),
            highlights: HashMap::new(),
            fg: 0xd8dee9,
            bg: 0x0b1016,
            cursor: (0, 0),
            mode: "normal".into(),
        };
        let options = Value::Map(vec![
            ("ext_linegrid".into(), true.into()),
            ("rgb".into(), true.into()),
        ]);
        nvim.call(
            "nvim_ui_attach",
            vec![(cols as u64).into(), (rows as u64).into(), options],
        );
        // Edits land on disk right away, so git and the map see them.
        nvim.call(
            "nvim_command",
            vec![
                "set hidden autowriteall | autocmd InsertLeave,TextChanged,BufLeave,FocusLost * silent! update"
                    .into(),
            ],
        );
        Ok(nvim)
    }

    fn call(&mut self, method: &str, args: Vec<Value>) {
        self.request += 1;
        let message = Value::Array(vec![
            0.into(),
            self.request.into(),
            method.into(),
            Value::Array(args),
        ]);
        let _ = rmpv::encode::write_value(&mut self.stdin, &message);
        let _ = self.stdin.flush();
    }

    /// Whether the process still runs.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Keys in Neovim's notation, e.g. `i`, `<Esc>`, `<C-w>`.
    pub fn input(&mut self, keys: &str) {
        self.call("nvim_input", vec![keys.into()]);
    }

    /// A mouse `button` ("left", "wheel") `action` ("press", "up") at a cell.
    pub fn mouse(&mut self, button: &str, action: &str, row: usize, col: usize) {
        self.call(
            "nvim_input_mouse",
            vec![
                button.into(),
                action.into(),
                "".into(),
                0u64.into(),
                (row as u64).into(),
                (col as u64).into(),
            ],
        );
    }

    /// Resizes the grid to `cols` by `rows`.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        if (cols, rows) != (self.cols, self.rows) && cols > 0 && rows > 0 {
            self.cols = cols;
            self.rows = rows;
            self.call(
                "nvim_ui_try_resize",
                vec![(cols as u64).into(), (rows as u64).into()],
            );
        }
    }

    /// Applies the redraws that arrived since the last call.
    pub fn pump(&mut self) {
        while let Ok(message) = self.redraws.try_recv() {
            let Some(message) = message.as_array() else {
                continue;
            };
            if message.len() != 3 || number(&message[0]) != 2 {
                continue;
            }
            if message[1].as_str() != Some("redraw") {
                continue;
            }
            for event in message[2].as_array().into_iter().flatten() {
                let Some(event) = event.as_array() else {
                    continue;
                };
                let Some(name) = event.first().and_then(Value::as_str) else {
                    continue;
                };
                for args in &event[1..] {
                    if let Some(args) = args.as_array() {
                        self.apply(name, args);
                    }
                }
            }
        }
    }

    fn apply(&mut self, name: &str, a: &[Value]) {
        match name {
            "mode_change" if !a.is_empty() => self.mode = a[0].as_str().unwrap_or("").into(),
            "grid_resize" if a.len() >= 3 => {
                let (width, height) = (number(&a[1]) as usize, number(&a[2]) as usize);
                self.cells.resize(height, Vec::new());
                for row in &mut self.cells {
                    row.resize(width, (" ".into(), 0));
                }
            }
            "default_colors_set" if a.len() >= 2 => {
                if let Some(fg) = a[0].as_i64().filter(|&c| c >= 0) {
                    self.fg = fg as u32;
                }
                if let Some(bg) = a[1].as_i64().filter(|&c| c >= 0) {
                    self.bg = bg as u32;
                }
            }
            "hl_attr_define" if a.len() >= 2 => {
                let mut highlight = Highlight::default();
                for (key, value) in a[1].as_map().into_iter().flatten() {
                    match key.as_str() {
                        Some("foreground") => highlight.fg = value.as_u64().map(|c| c as u32),
                        Some("background") => highlight.bg = value.as_u64().map(|c| c as u32),
                        Some("reverse") => highlight.reverse = value.as_bool().unwrap_or(false),
                        Some("bold") => highlight.bold = value.as_bool().unwrap_or(false),
                        Some("italic") => highlight.italic = value.as_bool().unwrap_or(false),
                        Some("underline") | Some("undercurl") => {
                            highlight.underline = value.as_bool().unwrap_or(false)
                        }
                        _ => {}
                    }
                }
                self.highlights.insert(number(&a[0]), highlight);
            }
            "grid_line" if a.len() >= 4 => {
                let (row, mut col) = (number(&a[1]) as usize, number(&a[2]) as usize);
                let mut highlight = 0;
                for cell in a[3].as_array().into_iter().flatten() {
                    let Some(cell) = cell.as_array() else {
                        continue;
                    };
                    let text = cell.first().and_then(Value::as_str).unwrap_or(" ");
                    if let Some(id) = cell.get(1) {
                        highlight = number(id);
                    }
                    for _ in 0..cell.get(2).map_or(1, number) {
                        if let Some(slot) = self.cells.get_mut(row).and_then(|r| r.get_mut(col)) {
                            *slot = (text.to_owned(), highlight);
                        }
                        col += 1;
                    }
                }
            }
            "grid_clear" => {
                for row in &mut self.cells {
                    row.fill((" ".into(), 0));
                }
            }
            "grid_cursor_goto" if a.len() >= 3 => {
                self.cursor = (number(&a[1]) as usize, number(&a[2]) as usize)
            }
            "grid_scroll" if a.len() >= 7 => {
                let (top, bottom) = (number(&a[1]) as usize, number(&a[2]) as usize);
                let (left, right) = (number(&a[3]) as usize, number(&a[4]) as usize);
                let rows = a[5].as_i64().unwrap_or(0);
                let copy = |cells: &mut Vec<Vec<(String, u64)>>, to: usize, from: usize| {
                    if from < cells.len() && to < cells.len() {
                        let right = right.min(cells[from].len()).min(cells[to].len());
                        if left < right {
                            let part = cells[from][left..right].to_vec();
                            cells[to][left..right].clone_from_slice(&part);
                        }
                    }
                };
                if rows > 0 {
                    for row in top..bottom.saturating_sub(rows as usize) {
                        copy(&mut self.cells, row, row + rows as usize);
                    }
                } else if rows < 0 {
                    let n = (-rows) as usize;
                    for row in (top + n..bottom).rev() {
                        copy(&mut self.cells, row, row - n);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Drop for Nvim {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// An egui key press in Neovim's key notation, for keys that send no text.
pub fn key_notation(key: egui::Key, modifiers: egui::Modifiers) -> Option<String> {
    use egui::Key::*;
    let special = match key {
        Enter => Some("CR"),
        Escape => Some("Esc"),
        Backspace => Some("BS"),
        Tab => Some("Tab"),
        Delete => Some("Del"),
        ArrowUp => Some("Up"),
        ArrowDown => Some("Down"),
        ArrowLeft => Some("Left"),
        ArrowRight => Some("Right"),
        Home => Some("Home"),
        End => Some("End"),
        PageUp => Some("PageUp"),
        PageDown => Some("PageDown"),
        _ => None,
    };
    let ctrl = if modifiers.ctrl { "C-" } else { "" };
    if let Some(special) = special {
        let shift = if modifiers.shift { "S-" } else { "" };
        return Some(format!("<{ctrl}{shift}{special}>"));
    }
    // Ctrl with a letter sends no text event, so the key carries it.
    let name = key.name();
    (modifiers.ctrl && name.len() == 1).then(|| format!("<C-{}>", name.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_without_text_are_written_in_neovim_notation() {
        let none = egui::Modifiers::NONE;
        assert_eq!(
            key_notation(egui::Key::Escape, none).as_deref(),
            Some("<Esc>")
        );
        assert_eq!(
            key_notation(egui::Key::W, egui::Modifiers::CTRL).as_deref(),
            Some("<C-w>")
        );
        assert_eq!(key_notation(egui::Key::W, none), None);
    }

    /// Needs `nvim` on the PATH; skipped where there is none.
    #[test]
    fn an_embedded_neovim_draws_the_file_it_opened() {
        if !available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "first\nsecond line\n").unwrap();
        let mut nvim = Nvim::spawn(&file, None, 2, 40, 6, || {}).unwrap();
        let started = std::time::Instant::now();
        let text = |nvim: &Nvim| -> String {
            nvim.cells
                .iter()
                .map(|row| row.iter().map(|c| c.0.as_str()).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n")
        };
        while !(text(&nvim).contains("second line") && nvim.cursor.0 == 1)
            && started.elapsed().as_secs() < 10
        {
            nvim.pump();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(text(&nvim).contains("second line"), "{}", text(&nvim));
        assert_eq!(nvim.cursor.0, 1, "the cursor stands on the line asked for");
    }
}
