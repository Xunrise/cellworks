//! The custom arrangement of a project (ADR-0006): how far the engineer
//! moved each district and block in the custom lens, kept in Cellworks' own
//! data folder, never in the project and never in the settings file.
//!
//! One file per project, named after the project's path; a line per moved
//! element: how far along ground x and y, then its path relative to the
//! project. Reading is tolerant: a line that does not parse is left out.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::ProjectId;

/// How far each element of one project was moved, by its path relative to
/// the project, in the map's unturned ground units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Arrangement {
    pub moves: BTreeMap<PathBuf, (f32, f32)>,
}

impl Arrangement {
    /// Adds a move of `by` to the element at `path`, relative to the project.
    pub fn moved(&mut self, path: PathBuf, by: (f32, f32)) {
        let at = self.moves.entry(path).or_insert((0.0, 0.0));
        *at = (at.0 + by.0, at.1 + by.1);
    }
}

/// Where arrangements are kept: a folder of their own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrangementStore {
    folder: PathBuf,
}

impl ArrangementStore {
    /// The store in `folder`, which need not exist yet.
    pub fn at(folder: PathBuf) -> ArrangementStore {
        ArrangementStore { folder }
    }

    /// The store beside the settings file `settings_file`.
    pub fn beside(settings_file: &Path) -> ArrangementStore {
        let folder = settings_file
            .parent()
            .unwrap_or(Path::new("."))
            .join("arrangements");
        ArrangementStore::at(folder)
    }

    /// The arrangement of `project`: empty when none was kept or the file
    /// cannot be read.
    pub fn load(&self, project: &ProjectId) -> Arrangement {
        let Ok(text) = fs::read_to_string(self.file(project)) else {
            return Arrangement::default();
        };
        let moves = text
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(3, '\t');
                let x = parts.next()?.parse().ok()?;
                let y = parts.next()?.parse().ok()?;
                let path = parts.next().filter(|path| !path.is_empty())?;
                Some((PathBuf::from(path), (x, y)))
            })
            .collect();
        Arrangement { moves }
    }

    /// Keeps `arrangement` as `project`'s.
    pub fn save(&self, project: &ProjectId, arrangement: &Arrangement) -> io::Result<()> {
        fs::create_dir_all(&self.folder)?;
        let text: String = arrangement
            .moves
            .iter()
            .map(|(path, (x, y))| format!("{x}\t{y}\t{}\n", path.display()))
            .collect();
        fs::write(self.file(project), text)
    }

    /// The file of `project`: its path with every character that is not a
    /// letter or a digit written as `_`.
    fn file(&self, project: &ProjectId) -> PathBuf {
        let name: String = project
            .path()
            .to_string_lossy()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        self.folder.join(format!("{name}.tsv"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Project;

    #[test]
    fn an_arrangement_kept_is_the_one_loaded() {
        let folder = tempfile::tempdir().expect("a temporary directory");
        let store = ArrangementStore::at(folder.path().join("arrangements"));
        let shop = Project::at(PathBuf::from("/p/shop")).id;
        let mut arrangement = Arrangement::default();
        arrangement.moved(PathBuf::from("catalog/pricing.rs"), (12.5, -3.0));
        arrangement.moved(PathBuf::from("catalog/pricing.rs"), (2.5, 1.0));
        arrangement.moved(PathBuf::from("orders"), (-40.0, 0.0));

        store.save(&shop, &arrangement).expect("kept");

        assert_eq!(store.load(&shop), arrangement);
        assert_eq!(
            arrangement.moves[Path::new("catalog/pricing.rs")],
            (15.0, -2.0)
        );
        assert_eq!(
            store.load(&Project::at(PathBuf::from("/p/other")).id),
            Arrangement::default()
        );
    }

    #[test]
    fn a_line_that_does_not_parse_is_left_out() {
        let folder = tempfile::tempdir().expect("a temporary directory");
        let store = ArrangementStore::at(folder.path().to_path_buf());
        let shop = Project::at(PathBuf::from("/p/shop")).id;
        fs::write(store.file(&shop), "1\t2\ta.rs\nnot a move\n3\t\tb.rs\n").expect("write");

        assert_eq!(store.load(&shop).moves.len(), 1);
    }
}
