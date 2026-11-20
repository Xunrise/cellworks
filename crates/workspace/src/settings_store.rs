//! The settings the app itself edits, and the store that writes them.
//!
//! [`Settings`] is the typed view of the keys the settings window owns. The
//! file is loaded as a `toml::Table`, only those keys are replaced and the
//! table is written back through a temporary file and a rename, so every key
//! this module does not know (`added_folders`, `[projects."<path>"]`,
//! `[hosts."<name>"]`, anything newer) survives. The `toml` crate drops
//! comments, so the first write keeps the previous file as `settings.toml.bak`.
//!
//! Environment variables keep overriding the file ([`Env`]): the demo and the
//! tests rely on them, and the window shows which field one has taken over.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Which view a project opens in when it has no saved choice of its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DefaultView {
    #[default]
    City,
    Flat,
}

impl DefaultView {
    pub fn name(self) -> &'static str {
        match self {
            DefaultView::City => "city",
            DefaultView::Flat => "flat",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct JiraSettings {
    pub enabled: bool,
    /// Part of the site's address to pick when a sign-in reaches several.
    pub site: String,
    pub offline: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CockpitSettings {
    pub enabled: bool,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GitHubSettings {
    pub enabled: bool,
    /// `owner/repo` to list open issues of.
    pub repos: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapDefaults {
    pub default_view: DefaultView,
    /// A lens by name (`folders`, `imports`, `calls`, `co-change`, `cells`).
    pub default_lens: String,
    pub show_ignored: bool,
    pub import_lanes: bool,
    pub call_lanes: bool,
    pub underground: bool,
}

/// Everything the settings window edits.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Folders walked for git repositories; a leading `~/` is the home folder.
    pub project_folders: Vec<String>,
    pub theme_words: bool,
    pub jira: JiraSettings,
    pub cockpit: CockpitSettings,
    pub github: GitHubSettings,
    /// The Linear API key is not here: it lives in the keychain.
    pub linear_enabled: bool,
    pub map: MapDefaults,
    /// Wins over `$VISUAL` and `$EDITOR`; empty falls back to them, else
    /// the system's text editor.
    pub editor: String,
    /// Empty: `~/.cellworks/cases`.
    pub cases_folder: String,
    /// Empty: the skills of this source tree.
    pub skills_folder: String,
    /// The terminal editor files open in inside Cellworks, with its flags;
    /// empty: the embedded Neovim.
    pub embedded_editor: String,
    /// The configuration the embedded Neovim reads instead of the user's own;
    /// empty: the user's own, colour scheme and all.
    pub nvim_config: String,
    pub auto_check: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            project_folders: vec!["~/Projects".to_owned()],
            theme_words: true,
            jira: JiraSettings {
                enabled: true,
                site: String::new(),
                offline: false,
            },
            cockpit: CockpitSettings {
                enabled: true,
                url: "http://127.0.0.1:4747".to_owned(),
            },
            github: GitHubSettings {
                enabled: true,
                repos: Vec::new(),
            },
            linear_enabled: true,
            map: MapDefaults {
                default_view: DefaultView::City,
                default_lens: "folders".to_owned(),
                show_ignored: false,
                import_lanes: false,
                call_lanes: false,
                underground: false,
            },
            editor: String::new(),
            cases_folder: String::new(),
            skills_folder: String::new(),
            embedded_editor: String::new(),
            nvim_config: String::new(),
            auto_check: true,
        }
    }
}

// ------------------------------------------------------------------ env

/// The environment variables that override a settings value.
const ENV_VARS: [&str; 7] = [
    "CELLWORKS_JIRA_SITE",
    "CELLWORKS_JIRA_OFFLINE",
    "CELLWORKS_COCKPIT_URL",
    "CELLWORKS_GITHUB_REPOS",
    "CELLWORKS_LINEAR_API_KEY",
    "CELLWORKS_CASES",
    "CELLWORKS_SKILLS",
];

/// The overriding variables that are set. Passed in rather than read where it
/// matters, so a test never depends on (or changes) the process environment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Env(BTreeMap<String, String>);

impl Env {
    /// The variables of this process.
    pub fn process() -> Env {
        let vars = ENV_VARS.iter().chain(["EDITOR"].iter());
        Env(vars
            .filter_map(|name| Some(((*name).to_owned(), std::env::var(name).ok()?)))
            .collect())
    }

    pub fn of<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Env {
        Env(pairs
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect())
    }

    /// The value, when set and not blank.
    fn value(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(|v| v.trim()).filter(|v| !v.is_empty())
    }

    /// The variable that takes `key` (a dotted settings key such as
    /// `tickets.jira.site`) over, if one is set.
    pub fn overriding(&self, key: &str) -> Option<&'static str> {
        let names: &[&'static str] = match key {
            "tickets.jira.site" => &["CELLWORKS_JIRA_SITE"],
            "tickets.cockpit.url" => &["CELLWORKS_COCKPIT_URL"],
            "tickets.github.repos" => &["CELLWORKS_GITHUB_REPOS"],
            "tickets.linear.key" => &["CELLWORKS_LINEAR_API_KEY"],
            "cases.folder" => &["CELLWORKS_CASES"],
            "skills.folder" => &["CELLWORKS_SKILLS"],
            "tickets.jira.offline" => {
                // Any value, even empty, as the ticket facade has always read it.
                return self
                    .0
                    .contains_key("CELLWORKS_JIRA_OFFLINE")
                    .then_some("CELLWORKS_JIRA_OFFLINE");
            }
            _ => return None,
        };
        names.iter().copied().find(|n| self.value(n).is_some())
    }

    /// The Linear key from the environment, if set.
    pub fn linear_key(&self) -> Option<String> {
        self.value("CELLWORKS_LINEAR_API_KEY").map(str::to_owned)
    }
}

impl Settings {
    /// These settings with every environment override applied: what the app
    /// behaves by, and what the window shows in a disabled field.
    pub fn effective(&self, env: &Env) -> Settings {
        let mut out = self.clone();
        if let Some(v) = env.value("CELLWORKS_JIRA_SITE") {
            out.jira.site = v.to_owned();
        }
        if env.overriding("tickets.jira.offline").is_some() {
            out.jira.offline = true;
        }
        if let Some(v) = env.value("CELLWORKS_COCKPIT_URL") {
            out.cockpit.url = v.to_owned();
        }
        if let Some(v) = env.value("CELLWORKS_GITHUB_REPOS") {
            out.github.repos = v.split(',').map(|r| r.trim().to_owned()).collect();
        }
        if let Some(v) = env.value("CELLWORKS_CASES") {
            out.cases_folder = v.to_owned();
        }
        if let Some(v) = env.value("CELLWORKS_SKILLS") {
            out.skills_folder = v.to_owned();
        }
        out
    }

    /// What is wrong with these settings, by dotted key. Nothing here touches
    /// the network; folders are looked up on disk.
    pub fn problems(&self) -> BTreeMap<&'static str, String> {
        let mut out = BTreeMap::new();
        for folder in &self.project_folders {
            if !expand_home(folder).is_dir() {
                out.insert("projects.folders", format!("{folder} is not a folder"));
                break;
            }
        }
        let url = self.cockpit.url.trim();
        if !(url.starts_with("http://") || url.starts_with("https://")) || url.contains(' ') {
            out.insert(
                "tickets.cockpit.url",
                "needs an http:// or https:// address".to_owned(),
            );
        }
        if let Some(bad) = self.github.repos.iter().find(|r| {
            let mut parts = r.split('/');
            let ok = |p: Option<&str>| {
                p.is_some_and(|p| !p.is_empty() && !p.contains(char::is_whitespace))
            };
            !(ok(parts.next()) && ok(parts.next()) && parts.next().is_none())
        }) {
            out.insert("tickets.github.repos", format!("{bad} is not owner/repo"));
        }
        if self.jira.site.contains(char::is_whitespace) {
            out.insert("tickets.jira.site", "no spaces".to_owned());
        }
        for (key, value) in [
            ("cases.folder", &self.cases_folder),
            ("skills.folder", &self.skills_folder),
            ("editor.nvim_config", &self.nvim_config),
        ] {
            let v = value.trim();
            if !(v.is_empty() || v.starts_with('/') || v.starts_with("~/")) {
                out.insert(key, "needs an absolute path".to_owned());
            }
        }
        if !LENSES.contains(&self.map.default_lens.as_str()) {
            out.insert("map.default_lens", "unknown lens".to_owned());
        }
        out
    }

    /// `self` with the fields `problems` names put back to `previous`'s, so a
    /// half-typed value is shown but never taken up or saved.
    pub fn accepted(&self, previous: &Settings) -> Settings {
        let mut out = self.clone();
        for key in self.problems().keys() {
            match *key {
                "projects.folders" => out.project_folders = previous.project_folders.clone(),
                "tickets.cockpit.url" => out.cockpit.url = previous.cockpit.url.clone(),
                "tickets.github.repos" => out.github.repos = previous.github.repos.clone(),
                "tickets.jira.site" => out.jira.site = previous.jira.site.clone(),
                "cases.folder" => out.cases_folder = previous.cases_folder.clone(),
                "skills.folder" => out.skills_folder = previous.skills_folder.clone(),
                "editor.nvim_config" => out.nvim_config = previous.nvim_config.clone(),
                "map.default_lens" => out.map.default_lens = previous.map.default_lens.clone(),
                _ => {}
            }
        }
        out
    }
}

/// The lenses that can be a default: those every project offers or needs
/// nothing of its own for.
pub const LENSES: [&str; 5] = ["folders", "imports", "calls", "co-change", "cells"];

/// `~/rest` as `$HOME/rest`; anything else as written.
pub fn expand_home(path: &str) -> PathBuf {
    let path = path.trim();
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ if path == "~" => std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default(),
        _ => PathBuf::from(path),
    }
}

// ---------------------------------------------------------------- table

fn get<'a>(table: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Value> {
    let (last, parents) = path.split_last()?;
    let mut table = table;
    for key in parents {
        table = table.get(*key)?.as_table()?;
    }
    table.get(*last)
}

fn text(table: &toml::Table, path: &[&str], default: &str) -> String {
    get(table, path)
        .and_then(toml::Value::as_str)
        .unwrap_or(default)
        .to_owned()
}

fn flag(table: &toml::Table, path: &[&str], default: bool) -> bool {
    get(table, path)
        .and_then(toml::Value::as_bool)
        .unwrap_or(default)
}

fn list(table: &toml::Table, path: &[&str]) -> Option<Vec<String>> {
    Some(
        get(table, path)?
            .as_array()?
            .iter()
            .filter_map(toml::Value::as_str)
            .map(str::to_owned)
            .collect(),
    )
}

fn put(table: &mut toml::Table, path: &[&str], value: toml::Value) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut table = table;
    for key in parents {
        let entry = table
            .entry((*key).to_owned())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if !entry.is_table() {
            *entry = toml::Value::Table(toml::Table::new());
        }
        table = entry.as_table_mut().expect("just made a table");
    }
    table.insert((*last).to_owned(), value);
}

impl Settings {
    fn from_table(table: &toml::Table) -> Settings {
        let d = Settings::default();
        let view = match text(table, &["map", "default_view"], "").as_str() {
            "flat" => DefaultView::Flat,
            _ => DefaultView::City,
        };
        let lens = text(table, &["map", "default_lens"], "folders");
        Settings {
            project_folders: list(table, &["projects", "folders"]).unwrap_or(d.project_folders),
            theme_words: flag(table, &["launcher", "theme_words"], true),
            jira: JiraSettings {
                enabled: flag(table, &["tickets", "jira", "enabled"], true),
                site: text(table, &["tickets", "jira", "site"], ""),
                offline: flag(table, &["tickets", "jira", "offline"], false),
            },
            cockpit: CockpitSettings {
                enabled: flag(table, &["tickets", "cockpit", "enabled"], true),
                url: text(table, &["tickets", "cockpit", "url"], &d.cockpit.url),
            },
            github: GitHubSettings {
                enabled: flag(table, &["tickets", "github", "enabled"], true),
                repos: list(table, &["tickets", "github", "repos"]).unwrap_or_default(),
            },
            linear_enabled: flag(table, &["tickets", "linear", "enabled"], true),
            map: MapDefaults {
                default_view: view,
                default_lens: if LENSES.contains(&lens.as_str()) {
                    lens
                } else {
                    d.map.default_lens
                },
                show_ignored: flag(table, &["map", "show_ignored"], false),
                import_lanes: flag(table, &["map", "import_lanes"], false),
                call_lanes: flag(table, &["map", "call_lanes"], false),
                underground: flag(table, &["map", "underground"], false),
            },
            editor: text(table, &["editor", "command"], ""),
            cases_folder: text(table, &["cases", "folder"], ""),
            skills_folder: text(table, &["skills", "folder"], ""),
            embedded_editor: text(table, &["editor", "embedded"], ""),
            nvim_config: text(table, &["editor", "nvim_config"], ""),
            auto_check: flag(table, &["updates", "auto_check"], true),
        }
    }

    /// Replaces exactly the keys this struct owns in `table`.
    fn write_into(&self, table: &mut toml::Table) {
        use toml::Value::{Array, Boolean, String as Str};
        let strings = |v: &[String]| Array(v.iter().cloned().map(Str).collect());
        put(
            table,
            &["projects", "folders"],
            strings(&self.project_folders),
        );
        put(
            table,
            &["launcher", "theme_words"],
            Boolean(self.theme_words),
        );
        put(
            table,
            &["tickets", "jira", "enabled"],
            Boolean(self.jira.enabled),
        );
        put(
            table,
            &["tickets", "jira", "site"],
            Str(self.jira.site.clone()),
        );
        put(
            table,
            &["tickets", "jira", "offline"],
            Boolean(self.jira.offline),
        );
        put(
            table,
            &["tickets", "cockpit", "enabled"],
            Boolean(self.cockpit.enabled),
        );
        put(
            table,
            &["tickets", "cockpit", "url"],
            Str(self.cockpit.url.clone()),
        );
        put(
            table,
            &["tickets", "github", "enabled"],
            Boolean(self.github.enabled),
        );
        put(
            table,
            &["tickets", "github", "repos"],
            strings(&self.github.repos),
        );
        put(
            table,
            &["tickets", "linear", "enabled"],
            Boolean(self.linear_enabled),
        );
        put(
            table,
            &["map", "default_view"],
            Str(self.map.default_view.name().to_owned()),
        );
        put(
            table,
            &["map", "default_lens"],
            Str(self.map.default_lens.clone()),
        );
        put(
            table,
            &["map", "show_ignored"],
            Boolean(self.map.show_ignored),
        );
        put(
            table,
            &["map", "import_lanes"],
            Boolean(self.map.import_lanes),
        );
        put(table, &["map", "call_lanes"], Boolean(self.map.call_lanes));
        put(
            table,
            &["map", "underground"],
            Boolean(self.map.underground),
        );
        put(table, &["editor", "command"], Str(self.editor.clone()));
        put(
            table,
            &["editor", "embedded"],
            Str(self.embedded_editor.clone()),
        );
        put(
            table,
            &["editor", "nvim_config"],
            Str(self.nvim_config.clone()),
        );
        put(table, &["cases", "folder"], Str(self.cases_folder.clone()));
        put(
            table,
            &["skills", "folder"],
            Str(self.skills_folder.clone()),
        );
        put(table, &["updates", "auto_check"], Boolean(self.auto_check));
    }
}

// ---------------------------------------------------------------- store

/// What [`SettingsStore::load`] found.
#[derive(Debug)]
pub struct Loaded {
    pub settings: Settings,
    /// Why the file could not be used, when it could not: the defaults are
    /// shown and nothing is saved over it.
    pub problem: Option<String>,
}

/// The settings file, read and written by the app.
#[derive(Clone, Debug)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn at(path: impl Into<PathBuf>) -> SettingsStore {
        SettingsStore { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn table(&self) -> Result<toml::Table, String> {
        match fs::read_to_string(&self.path) {
            Ok(text) => text.parse::<toml::Table>().map_err(|e| e.to_string()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(toml::Table::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Absent file: the defaults. Unreadable or garbled file: the defaults
    /// and the reason.
    pub fn load(&self) -> Loaded {
        match self.table() {
            Ok(table) => Loaded {
                settings: Settings::from_table(&table),
                problem: None,
            },
            Err(reason) => Loaded {
                settings: Settings::default(),
                problem: Some(reason),
            },
        }
    }

    /// Writes `settings` into the file, keeping every other key. A file that
    /// cannot be parsed is refused rather than overwritten.
    pub fn save(&self, settings: &Settings) -> io::Result<()> {
        let mut table = self
            .table()
            .map_err(|reason| io::Error::new(io::ErrorKind::InvalidData, reason))?;
        settings.write_into(&mut table);
        let text = toml::to_string_pretty(&table)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        // The toml crate drops comments: keep the file as the user wrote it
        // the first time this app writes over it.
        let backup = with_suffix(&self.path, ".bak");
        if self.path.exists() && !backup.exists() {
            fs::copy(&self.path, &backup)?;
        }
        let temporary = with_suffix(&self.path, ".tmp");
        fs::write(&temporary, text)?;
        fs::rename(temporary, &self.path)
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

// -------------------------------------------------------------- runtime

/// The folders and editor other crates read, through the functions below.
struct Runtime {
    cases: Option<PathBuf>,
    skills: Option<PathBuf>,
    editor: Option<String>,
}

static RUNTIME: RwLock<Runtime> = RwLock::new(Runtime {
    cases: None,
    skills: None,
    editor: None,
});

/// Takes the folders and editor of `effective` (see [`Settings::effective`])
/// as what [`cases_folder`], [`skills_folder`] and [`editor_command`] answer.
pub fn apply_runtime(effective: &Settings) {
    let folder = |v: &str| Some(v.trim()).filter(|v| !v.is_empty()).map(expand_home);
    if let Ok(mut runtime) = RUNTIME.write() {
        runtime.cases = folder(&effective.cases_folder);
        runtime.skills = folder(&effective.skills_folder);
        runtime.editor = Some(effective.editor.trim().to_owned()).filter(|v| !v.is_empty());
    }
}

/// Where case files live: `CELLWORKS_CASES`, else the setting, else
/// `~/.cellworks/cases`.
pub fn cases_folder() -> PathBuf {
    let env = std::env::var_os("CELLWORKS_CASES").filter(|v| !v.is_empty());
    let set = RUNTIME.read().ok().and_then(|r| r.cases.clone());
    env.map(PathBuf::from)
        .or(set)
        .unwrap_or_else(|| expand_home("~/.cellworks/cases"))
}

/// Where ticket skills live: `CELLWORKS_SKILLS`, else the setting, else
/// `None` (the caller's own default).
pub fn skills_folder() -> Option<PathBuf> {
    let env = std::env::var_os("CELLWORKS_SKILLS").filter(|v| !v.is_empty());
    env.map(PathBuf::from)
        .or_else(|| RUNTIME.read().ok().and_then(|r| r.skills.clone()))
}

/// The editor command: the setting, else `$VISUAL`, else `$EDITOR`. Most
/// shells set `$EDITOR`, so it is only the fallback; otherwise nobody could
/// choose an editor in the window.
pub fn editor_command() -> Option<String> {
    RUNTIME
        .read()
        .ok()
        .and_then(|r| r.editor.clone())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            ["VISUAL", "EDITOR"]
                .into_iter()
                .filter_map(|name| std::env::var(name).ok())
                .find(|value| !value.trim().is_empty())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, SettingsStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = SettingsStore::at(dir.path().join("Cellworks/settings.toml"));
        (dir, store)
    }

    #[test]
    fn a_missing_file_gives_the_defaults() {
        let (_dir, store) = store();
        let loaded = store.load();
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.problem.is_none());
    }

    #[test]
    fn a_garbled_file_gives_the_defaults_and_is_never_overwritten() {
        let (_dir, store) = store();
        fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        fs::write(store.path(), "this is [not toml").unwrap();

        let loaded = store.load();
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.problem.is_some());
        assert!(store.save(&Settings::default()).is_err());
        assert_eq!(
            fs::read_to_string(store.path()).unwrap(),
            "this is [not toml"
        );
    }

    #[test]
    fn settings_round_trip_and_unknown_keys_survive() {
        let (_dir, store) = store();
        fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        fs::write(
            store.path(),
            "# mine\nadded_folders = [\"/x\"]\nfuture = 3\n[projects.\"/p\"]\ndistrict_prefix = \"crate\"\n[hosts.box]\nprojects = [\"/srv\"]\n",
        )
        .unwrap();

        let mut settings = store.load().settings;
        settings.map.default_view = DefaultView::Flat;
        settings.map.default_lens = "imports".into();
        settings.github.repos = vec!["a/b".into()];
        settings.auto_check = false;
        store.save(&settings).unwrap();

        assert_eq!(store.load().settings, settings);
        let table: toml::Table = fs::read_to_string(store.path()).unwrap().parse().unwrap();
        assert_eq!(table["future"].as_integer(), Some(3));
        assert_eq!(table["added_folders"].as_array().unwrap().len(), 1);
        assert_eq!(
            table["projects"]["/p"]["district_prefix"].as_str(),
            Some("crate")
        );
        assert_eq!(
            table["hosts"]["box"]["projects"].as_array().unwrap().len(),
            1
        );
        // The comment is gone from the file but kept in the backup.
        let backup = fs::read_to_string(with_suffix(store.path(), ".bak")).unwrap();
        assert!(backup.starts_with("# mine"));
    }

    #[test]
    fn the_backup_is_made_once() {
        let (_dir, store) = store();
        fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        fs::write(store.path(), "# first\n").unwrap();
        store.save(&Settings::default()).unwrap();
        store.save(&Settings::default()).unwrap();
        let backup = fs::read_to_string(with_suffix(store.path(), ".bak")).unwrap();
        assert_eq!(backup, "# first\n");
    }

    #[test]
    fn the_environment_overrides_the_file() {
        let mut settings = Settings::default();
        settings.cockpit.url = "http://file:1".into();
        settings.github.repos = vec!["f/f".into()];
        let env = Env::of([
            ("CELLWORKS_COCKPIT_URL", "http://env:2"),
            ("CELLWORKS_GITHUB_REPOS", "a/b, c/d"),
            ("CELLWORKS_JIRA_OFFLINE", ""),
            ("EDITOR", "vim"),
        ]);

        let effective = settings.effective(&env);

        assert_eq!(effective.cockpit.url, "http://env:2");
        assert_eq!(effective.github.repos, vec!["a/b", "c/d"]);
        assert!(effective.jira.offline);
        // $EDITOR is only a fallback; the setting (empty here) stays.
        assert_eq!(effective.editor, "");
        assert_eq!(env.overriding("editor.command"), None);
        assert_eq!(effective.embedded_editor, "");
        assert_eq!(effective.nvim_config, "");
        assert_eq!(
            env.overriding("tickets.cockpit.url"),
            Some("CELLWORKS_COCKPIT_URL")
        );
        assert_eq!(env.overriding("cases.folder"), None);
        assert_eq!(settings.effective(&Env::default()), settings);
    }

    #[test]
    fn a_bad_value_is_reported_and_not_accepted() {
        let good = Settings::default();
        let mut typed = good.clone();
        typed.cockpit.url = "localhost".into();
        typed.project_folders = vec!["/definitely/not/here".into()];
        typed.map.underground = true;

        let problems = typed.problems();
        assert!(problems.contains_key("tickets.cockpit.url"));
        assert!(problems.contains_key("projects.folders"));

        let accepted = typed.accepted(&good);
        assert_eq!(accepted.cockpit.url, good.cockpit.url);
        assert_eq!(accepted.project_folders, good.project_folders);
        assert!(accepted.map.underground, "valid fields still go through");
    }
}
