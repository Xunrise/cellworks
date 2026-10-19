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
use std::sync::mpsc;

use crate::{ProjectId, WorkspaceEvent};

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
        // Written whole beside it, then put in its place: a write cut short
        // never leaves half an arrangement.
        let file = self.file(project);
        let written = file.with_extension("tsv.new");
        fs::write(&written, text)?;
        fs::rename(written, file)
    }

    /// The file of `project`: its path with every character that is not a
    /// letter or a digit written as `_`, and a hash of the path, so
    /// `my-shop` and `my_shop` keep arrangements of their own.
    fn file(&self, project: &ProjectId) -> PathBuf {
        let path = project.path().to_string_lossy();
        let name: String = path
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let hash = path.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        self.folder.join(format!("{name}-{hash:016x}.tsv"))
    }

    /// Reads `project`'s arrangement on a thread of its own, reported as
    /// [`WorkspaceEvent::ArrangementRead`].
    pub fn start_read(&self, project: ProjectId) -> mpsc::Receiver<WorkspaceEvent> {
        let (sender, receiver) = mpsc::channel();
        let store = self.clone();
        std::thread::spawn(move || {
            let arrangement = store.load(&project);
            // A send that fails is the UI having gone away.
            let _ = sender.send(WorkspaceEvent::ArrangementRead {
                project,
                arrangement,
            });
        });
        receiver
    }

    /// A thread that keeps every arrangement handed to it, one after the
    /// other in the order handed: the last one kept is the last one handed.
    pub fn start_keeping(&self) -> ArrangementKeeper {
        let (sender, receiver) = mpsc::channel::<(ProjectId, Arrangement)>();
        let store = self.clone();
        std::thread::spawn(move || {
            for (project, arrangement) in receiver {
                // A failed write only loses the move.
                let _ = store.save(&project, &arrangement);
            }
        });
        ArrangementKeeper { sender }
    }
}

/// Where arrangements go to be kept, off the frame loop and in order.
#[derive(Clone, Debug)]
pub struct ArrangementKeeper {
    sender: mpsc::Sender<(ProjectId, Arrangement)>,
}

impl ArrangementKeeper {
    /// Keeps `arrangement` as `project`'s.
    pub fn keep(&self, project: ProjectId, arrangement: Arrangement) {
        let _ = self.sender.send((project, arrangement));
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
    fn projects_whose_paths_differ_only_in_punctuation_keep_their_own() {
        let folder = tempfile::tempdir().expect("a temporary directory");
        let store = ArrangementStore::at(folder.path().to_path_buf());
        let (dash, underscore) = (
            Project::at(PathBuf::from("/p/my-shop")).id,
            Project::at(PathBuf::from("/p/my_shop")).id,
        );
        let mut moved = Arrangement::default();
        moved.moved(PathBuf::from("a.rs"), (1.0, 0.0));

        store.save(&dash, &moved).expect("kept");

        assert_eq!(store.load(&dash), moved);
        assert_eq!(store.load(&underscore), Arrangement::default());
    }

    #[test]
    fn arrangements_handed_one_after_the_other_are_kept_in_that_order() {
        let folder = tempfile::tempdir().expect("a temporary directory");
        let store = ArrangementStore::at(folder.path().join("arrangements"));
        let shop = Project::at(PathBuf::from("/p/shop")).id;
        let keeper = store.start_keeping();
        let mut last = Arrangement::default();
        for n in 0..20 {
            last.moved(PathBuf::from("a.rs"), (n as f32, 0.0));
            keeper.keep(shop.clone(), last.clone());
        }
        drop(keeper);

        let started = std::time::Instant::now();
        while store.load(&shop) != last && started.elapsed() < std::time::Duration::from_secs(5) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(store.load(&shop), last);
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
