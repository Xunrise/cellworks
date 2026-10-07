// Embedded Neovim: a msgpack-RPC UI client (ext_linegrid) whose screen grid the app paints inside a machine.
use eframe::egui;
use rmpv::Value;
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};

#[derive(Clone, Copy, Default)]
pub struct Hl {
    pub fg: Option<u32>,
    pub bg: Option<u32>,
    pub reverse: bool,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

pub struct Nvim {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    rx: Receiver<Value>,
    msgid: u64,
    pub path: String,
    pub cols: usize,
    pub rows: usize,
    /// rows of (text, highlight id)
    pub cells: Vec<Vec<(String, u64)>>,
    pub hl: HashMap<u64, Hl>,
    pub fg: u32,
    pub bg: u32,
    pub cursor: (usize, usize),
    /// current mode name ("normal", "insert", "visual", ...)
    pub mode: String,
}

fn u(v: &Value) -> u64 {
    v.as_u64().unwrap_or(0)
}

impl Nvim {
    pub fn spawn(path: &str, line: Option<u32>, cols: usize, rows: usize, repaint: egui::Context) -> std::io::Result<Self> {
        let dir = std::path::Path::new(path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let mut cmd = Command::new("nvim");
        cmd.args(["--embed", "-n"]);
        if let Some(l) = line {
            cmd.arg(format!("+{l}"));
        }
        let mut child = cmd.arg(path).current_dir(dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
        let stdin = BufWriter::new(child.stdin.take().unwrap());
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            while let Ok(v) = rmpv::decode::read_value(&mut stdout) {
                if tx.send(v).is_err() {
                    break;
                }
                repaint.request_repaint();
            }
        });
        let mut n = Nvim {
            child, stdin, rx, msgid: 0, path: path.into(), cols, rows, cells: vec![], hl: HashMap::new(), fg: 0xe8dcc0, bg: 0x14120f, cursor: (0, 0), mode: "normal".into(),
        };
        let opts = Value::Map(vec![("ext_linegrid".into(), true.into()), ("rgb".into(), true.into())]);
        n.request("nvim_ui_attach", vec![(cols as u64).into(), (rows as u64).into(), opts]);
        // edits land on disk right away, so git (and the map's git overlay) sees them; `hidden` lets another file open over unsaved work
        n.request("nvim_command", vec!["set hidden autowriteall | autocmd InsertLeave,TextChanged,BufLeave,FocusLost * silent! update".into()]);
        Ok(n)
    }

    fn request(&mut self, method: &str, args: Vec<Value>) {
        self.msgid += 1;
        let msg = Value::Array(vec![0.into(), self.msgid.into(), method.into(), Value::Array(args)]);
        let _ = rmpv::encode::write_value(&mut self.stdin, &msg);
        let _ = self.stdin.flush();
    }

    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Keys in Neovim notation, e.g. `i`, `<Esc>`, `<C-w>`.
    pub fn input(&mut self, keys: &str) {
        self.request("nvim_input", vec![keys.into()]);
    }

    pub fn mouse(&mut self, button: &str, action: &str, row: usize, col: usize) {
        self.request("nvim_input_mouse", vec![button.into(), action.into(), "".into(), 0u64.into(), (row as u64).into(), (col as u64).into()]);
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        if (cols, rows) != (self.cols, self.rows) && cols > 0 && rows > 0 {
            self.cols = cols;
            self.rows = rows;
            self.request("nvim_ui_try_resize", vec![(cols as u64).into(), (rows as u64).into()]);
        }
    }

    pub fn open(&mut self, path: &str, line: Option<u32>) {
        self.path = path.into();
        self.request("nvim_command", vec![format!("execute 'edit ' . fnameescape('{}')", path.replace('\'', "''")).into()]);
        if let Some(l) = line {
            self.request("nvim_command", vec![format!("{l}").into()]);
        }
    }

    /// Applies pending redraw events.
    pub fn pump(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            let Some(a) = msg.as_array() else { continue };
            if a.len() == 3 && u(&a[0]) == 2 && a[1].as_str() == Some("redraw") {
                for ev in a[2].as_array().into_iter().flatten() {
                    let Some(ev) = ev.as_array() else { continue };
                    let Some(name) = ev.first().and_then(Value::as_str) else { continue };
                    for args in &ev[1..] {
                        if let Some(args) = args.as_array() {
                            self.apply(name, args);
                        }
                    }
                }
            }
        }
    }

    fn apply(&mut self, name: &str, a: &[Value]) {
        match name {
            "mode_change" if !a.is_empty() => self.mode = a[0].as_str().unwrap_or("").into(),
            "grid_resize" if a.len() >= 3 => {
                let (w, h) = (u(&a[1]) as usize, u(&a[2]) as usize);
                self.cells.resize(h, vec![]);
                for row in &mut self.cells {
                    row.resize(w, (" ".into(), 0));
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
                let mut hl = Hl::default();
                for (k, v) in a[1].as_map().into_iter().flatten() {
                    match k.as_str() {
                        Some("foreground") => hl.fg = v.as_u64().map(|c| c as u32),
                        Some("background") => hl.bg = v.as_u64().map(|c| c as u32),
                        Some("reverse") => hl.reverse = v.as_bool().unwrap_or(false),
                        Some("bold") => hl.bold = v.as_bool().unwrap_or(false),
                        Some("italic") => hl.italic = v.as_bool().unwrap_or(false),
                        Some("underline") | Some("undercurl") => hl.underline = v.as_bool().unwrap_or(false),
                        _ => {}
                    }
                }
                self.hl.insert(u(&a[0]), hl);
            }
            "grid_line" if a.len() >= 4 => {
                let (row, mut col) = (u(&a[1]) as usize, u(&a[2]) as usize);
                let mut hl = 0;
                for cell in a[3].as_array().into_iter().flatten() {
                    let Some(c) = cell.as_array() else { continue };
                    let text = c.first().and_then(Value::as_str).unwrap_or(" ").to_string();
                    if let Some(h) = c.get(1) {
                        hl = u(h);
                    }
                    let repeat = c.get(2).map_or(1, u);
                    for _ in 0..repeat {
                        if let Some(slot) = self.cells.get_mut(row).and_then(|r| r.get_mut(col)) {
                            *slot = (text.clone(), hl);
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
            "grid_cursor_goto" if a.len() >= 3 => self.cursor = (u(&a[1]) as usize, u(&a[2]) as usize),
            "grid_scroll" if a.len() >= 7 => {
                let (top, bot, left, right) = (u(&a[1]) as usize, u(&a[2]) as usize, u(&a[3]) as usize, u(&a[4]) as usize);
                let rows = a[5].as_i64().unwrap_or(0);
                let copy = |cells: &mut Vec<Vec<(String, u64)>>, dst: usize, src: usize| {
                    if src < cells.len() && dst < cells.len() {
                        let right = right.min(cells[src].len());
                        let seg: Vec<_> = cells[src][left..right].to_vec();
                        cells[dst][left..right].clone_from_slice(&seg);
                    }
                };
                if rows > 0 {
                    for r in top..bot.saturating_sub(rows as usize) {
                        copy(&mut self.cells, r, r + rows as usize);
                    }
                } else if rows < 0 {
                    let n = (-rows) as usize;
                    for r in (top + n..bot).rev() {
                        copy(&mut self.cells, r, r - n);
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
    }
}

/// Translates an egui key press into Neovim key notation.
pub fn key_notation(key: egui::Key, m: egui::Modifiers) -> Option<String> {
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
    let prefix = format!("{}{}", if m.ctrl { "C-" } else { "" }, if m.shift && special.is_some() { "S-" } else { "" });
    if let Some(s) = special {
        return Some(format!("<{prefix}{s}>"));
    }
    // ctrl+letter produces no text event, so it is sent from the key
    let name = key.name();
    (m.ctrl && name.len() == 1).then(|| format!("<C-{}>", name.to_lowercase()))
}
