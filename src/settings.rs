// User settings, persisted as JSON in ~/.config/cellworks/settings.json.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// zoom after opening a project, relative to fitting the whole project (1.0)
    pub default_zoom: f32,
    pub scroll_zoom_speed: f32,
    pub left_panel_open: bool,
    pub right_panel_open: bool,
    /// other projects around the current one, and flying between them
    pub show_universe: bool,
    pub background_scan: bool,
    pub physics: bool,
    pub physics_speed: f32,
    pub editor_enabled: bool,
    pub editor_font_size: f32,
    pub editor_full_screen_by_default: bool,
    pub show_log: bool,
    pub show_agent_list: bool,
    pub show_import_belts: bool,
    pub show_context_pipes: bool,
    /// index into the permission modes offered for new servitors
    pub default_servitor_mode: usize,
    pub stale_after_days: i64,
    /// folder names skipped when scanning a project (tool caches, agent state)
    pub ignored_folders: Vec<String>,
    /// connections drawn between: 0 files, 1 folders, 2 districts
    pub conn_level: u8,
    pub conn_calls: bool,
    pub conn_cell_refs: bool,
    /// hide connections carrying fewer imports/calls than this
    pub conn_min_weight: u32,
    /// only connections touching the selection
    pub conn_selected_only: bool,
    pub git_overlay: bool,
    /// layout grouping: 0 folders, 1 connections, 2 git changes
    pub organize: u8,
    /// folders added by hand to the project list (anything, git repository or not)
    pub extra_projects: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            default_zoom: 1.0, scroll_zoom_speed: 1.0, left_panel_open: false, right_panel_open: false, show_universe: true,
            background_scan: true, physics: true, physics_speed: 1.0, editor_enabled: true, editor_font_size: 13.0,
            editor_full_screen_by_default: false, show_log: false, show_agent_list: true, show_import_belts: true, show_context_pipes: true,
            default_servitor_mode: 0, stale_after_days: 14,
            ignored_folders: DEFAULT_IGNORED.iter().map(|s| s.to_string()).collect(),
            conn_level: 1, conn_calls: false, conn_cell_refs: true, conn_min_weight: 1, conn_selected_only: false, git_overlay: false, organize: 0, extra_projects: vec![],
        }
    }
}

pub const DEFAULT_IGNORED: &[&str] = &[
    ".omc", ".pytest_cache", ".mypy_cache", ".ruff_cache", ".cache", ".turbo", ".parcel-cache", ".nyc_output", "coverage", ".idea", ".vscode", ".gradle", ".terraform",
];

fn path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/cellworks/settings.json")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let p = path();
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(p, serde_json::to_string_pretty(self).unwrap())
    }
}
