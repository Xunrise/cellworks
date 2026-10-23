//! Why a tower was built: the bounded context it belongs to, the pull
//! requests that last wrote its floors with each one's share of them, the
//! stories those pull requests name, and the decisions that bind it
//! (STORY-019).
//!
//! The join is pure: [`why_of`] reads only what is already on the
//! [`ProjectMap`] — the per-line history of STORY-018 and the bounded
//! contexts DomainKnowledge read — so the shares are worked out and tested
//! without a panel, a thread or git.

use std::path::{Path, PathBuf};

use crate::contexts::{Decision, StoryText};
use crate::graph::{Item, ProjectMap};
use crate::history::Commit;

/// Which group of floors a pull request share stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrKey {
    /// One pull request, by its number.
    Number(u32),
    /// The floors whose commits name no pull request, as one group.
    NoPullRequest,
    /// The floors no commit has written yet, as one group.
    NotCommitted,
}

impl PrKey {
    /// How the panel names the group.
    pub fn label(&self) -> String {
        match self {
            PrKey::Number(number) => format!("#{number}"),
            PrKey::NoPullRequest => "Commits without a pull request".to_owned(),
            PrKey::NotCommitted => "Not committed yet".to_owned(),
        }
    }

    /// Where a group stands among others of the same size: pull requests
    /// first, by number ascending, then the commits without one, then the
    /// floors not committed yet.
    fn rank(&self) -> (u8, u32) {
        match self {
            PrKey::Number(number) => (0, *number),
            PrKey::NoPullRequest => (1, 0),
            PrKey::NotCommitted => (2, 0),
        }
    }
}

/// One story a pull request's commits name, with the text known for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Story {
    /// The normalised id, as the commit message names it (`STORY-012`).
    pub id: String,
    /// Its title and "so that", where a local `sdlc/*/issue.md` knows them.
    pub text: Option<StoryText>,
}

impl Story {
    /// How the panel names it: the id with its title where known, the id
    /// alone otherwise.
    pub fn label(&self) -> String {
        match &self.text {
            Some(text) if !text.title.is_empty() => format!("{}: {}", self.id, text.title),
            _ => self.id.clone(),
        }
    }
}

/// What one pull request built of a tower.
#[derive(Clone, Debug, PartialEq)]
pub struct PrShare {
    pub key: PrKey,
    /// How many floors of the tower its commits last wrote.
    pub floors: usize,
    /// That count as a share of the tower's floors, 0 to 1.
    pub share: f32,
    /// Every story its commits name, in order of first mention, each with
    /// the text known for it. Stories carry no share of their own.
    pub stories: Vec<Story>,
    /// The subjects of its commits, in order of first mention; empty for
    /// the floors not committed yet.
    pub commits: Vec<String>,
}

/// Why one tower was built, as the why panel says it.
#[derive(Clone, Debug, PartialEq)]
pub struct Why {
    /// The tower explained.
    pub item: PathBuf,
    /// Its bounded context's name and description; `None` in no context.
    pub context: Option<(String, String)>,
    /// One group per pull request that last wrote floors of it, the largest
    /// share first, ties by pull request number ascending.
    pub built_by: Vec<PrShare>,
    /// For each floor of the tower, in source order, the index into
    /// [`Why::built_by`] of the group that last wrote it. The tint colours
    /// the floors by that index.
    pub floors: Vec<usize>,
    /// The Accepted decisions that bind it: those scoped to its bounded
    /// context, and the cross-cutting ones.
    pub decisions: Vec<Decision>,
}

impl Why {
    /// Whether every floor comes from a commit naming no story: the panel
    /// then says so and lists those commits' subjects. False for a tower
    /// with no commit behind any floor at all, which has nothing to list.
    pub fn without_story(&self) -> bool {
        self.built_by.iter().any(|share| !share.commits.is_empty())
            && self.built_by.iter().all(|share| share.stories.is_empty())
    }

    /// The stories `commit` names, as the tower already knows them; a story
    /// no group of this tower holds is left out, which cannot happen for a
    /// commit of its own floors.
    pub fn stories_of(&self, commit: &Commit) -> Vec<&Story> {
        commit
            .stories
            .iter()
            .filter_map(|id| {
                self.built_by
                    .iter()
                    .flat_map(|share| &share.stories)
                    .find(|story| &story.id == id)
            })
            .collect()
    }
}

/// Why the tower at `item` was built; `None` when the map holds no item
/// there — a district, a block or nothing selected has no why panel.
pub fn why_of(map: &ProjectMap, item: &Path) -> Option<Why> {
    let item = map.item(item)?;
    let known = map.contexts.as_ref();
    let context = item
        .standing_file()
        .zip(known)
        .and_then(|(file, known)| known.context_of_file(file))
        .map(|context| (context.name.clone(), context.description.clone()));
    let decisions = known
        .map(|known| {
            known
                .decisions
                .iter()
                .filter(|decision| decision.binds(context.as_ref().map(|(name, _)| name.as_str())))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let (built_by, floors) = shares(map, item);
    Some(Why {
        item: item.path.clone(),
        context,
        built_by,
        floors,
        decisions,
    })
}

/// The commit that last wrote floor `index` of `item`, as the map's history
/// tells it; `None` for an uncommitted floor, a floor the history does not
/// know, or an index past the tower's floors.
pub fn floor_commit<'m>(map: &'m ProjectMap, item: &Item, index: usize) -> Option<&'m Commit> {
    let file = item.standing_file()?;
    map.history.of_line(file, item.floors.get(index)?.number)
}

/// The groups of `item`'s floors by the pull request that last wrote them,
/// the largest first, and for each floor the index of its group.
fn shares(map: &ProjectMap, item: &Item) -> (Vec<PrShare>, Vec<usize>) {
    let stories_known = map.contexts.as_ref().map(|known| &known.stories);
    let mut groups: Vec<PrShare> = Vec::new();
    // The group each floor landed in before the groups were ordered.
    let mut gathered: Vec<usize> = Vec::with_capacity(item.floors.len());
    for index in 0..item.floors.len() {
        let commit = floor_commit(map, item, index);
        let key = match commit {
            Some(commit) => match commit.pull_request {
                Some(number) => PrKey::Number(number),
                None => PrKey::NoPullRequest,
            },
            None => PrKey::NotCommitted,
        };
        let at = match groups.iter().position(|group| group.key == key) {
            Some(at) => at,
            None => {
                groups.push(PrShare {
                    key,
                    floors: 0,
                    share: 0.0,
                    stories: Vec::new(),
                    commits: Vec::new(),
                });
                groups.len() - 1
            }
        };
        let group = &mut groups[at];
        group.floors += 1;
        if let Some(commit) = commit {
            if !group.commits.contains(&commit.summary) {
                group.commits.push(commit.summary.clone());
            }
            for id in &commit.stories {
                if !group.stories.iter().any(|story| &story.id == id) {
                    group.stories.push(Story {
                        id: id.clone(),
                        text: stories_known.and_then(|known| known.get(id)).cloned(),
                    });
                }
            }
        }
        gathered.push(at);
    }
    let total = item.floors.len().max(1) as f32;
    for group in &mut groups {
        group.share = group.floors as f32 / total;
    }
    // Largest share first; of two of the same size, the lower pull request
    // number first, and the groups without a pull request last.
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by_key(|&at| {
        (
            std::cmp::Reverse(groups[at].floors),
            groups[at].key.rank(),
            at,
        )
    });
    // Where each gathered group ended up once ordered.
    let mut moved_to = vec![0; groups.len()];
    for (place, &at) in order.iter().enumerate() {
        moved_to[at] = place;
    }
    let ordered = order.into_iter().map(|at| groups[at].clone()).collect();
    let floors = gathered.into_iter().map(|at| moved_to[at]).collect();
    (ordered, floors)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use egui::Rect;

    use super::*;
    use crate::contexts::{BoundedContext, BoundedContexts, Scope};
    use crate::graph::{Declaration, Floor, ItemKind};
    use crate::history::LineHistory;
    use crate::{Directories, default_prefix_rule};

    /// The project "shop", whose crate "catalog" holds "discount.rs" with a
    /// tower of `lines` floors; `blame` names, for each line, the commit of
    /// `commits` that last wrote it, `None` for an uncommitted line.
    fn shop(lines: usize, commits: Vec<Commit>, blame: Vec<Option<usize>>) -> ProjectMap {
        let file = PathBuf::from("catalog/discount.rs");
        let root = Path::new("/p");
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let item = Item {
            name: "discount".to_owned(),
            path: root.join(&file).join("discount"),
            kind: ItemKind::Function,
            tests: false,
            floors: (1..=lines)
                .map(|number| Floor {
                    number,
                    text: format!("line {number}"),
                    indent: 0,
                    width: 7,
                    tokens: Vec::new(),
                    calls: Vec::new(),
                    imports: Vec::new(),
                })
                .collect(),
            area: Rect::NOTHING,
            base: 0.0,
            declaration: Declaration::default(),
        };
        let mut items = BTreeMap::new();
        items.insert(file.clone(), vec![item]);
        let mut map = ProjectMap::of_code(
            "shop",
            root,
            std::slice::from_ref(&file),
            &strategy,
            items,
            &BTreeMap::new(),
        );
        let mut by_file = BTreeMap::new();
        by_file.insert(root.join(&file), blame);
        map.history = LineHistory {
            commits,
            lines: by_file,
        };
        map
    }

    fn commit(summary: &str, pull_request: Option<u32>, stories: &[&str]) -> Commit {
        Commit {
            sha: summary.to_owned(),
            time: 0,
            summary: summary.to_owned(),
            pull_request,
            stories: stories.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    /// The tower of the project built by [`shop`].
    fn tower() -> PathBuf {
        PathBuf::from("/p/catalog/discount.rs/discount")
    }

    #[test]
    fn shares_are_floors_per_pull_request_largest_first() {
        // Nine floors: the first two from #80, the other seven from #68.
        let blame: Vec<Option<usize>> = (0..9)
            .map(|index| if index < 2 { Some(1) } else { Some(0) })
            .collect();
        let map = shop(
            9,
            vec![
                commit("feat: a thing (#68)", Some(68), &["STORY-068"]),
                commit("feat: another (#80)", Some(80), &["STORY-080"]),
            ],
            blame,
        );

        let why = why_of(&map, &tower()).expect("a why for the tower");

        let shares: Vec<(String, usize)> = why
            .built_by
            .iter()
            .map(|share| (share.key.label(), share.floors))
            .collect();
        assert_eq!(
            shares,
            [("#68".to_owned(), 7), ("#80".to_owned(), 2)],
            "the larger share first"
        );
        assert!((why.built_by[0].share - 7.0 / 9.0).abs() < 1e-6);
        assert!((why.built_by[1].share - 2.0 / 9.0).abs() < 1e-6);
        // Every floor is tinted by the group that wrote it.
        assert_eq!(why.floors, [1, 1, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn a_pull_request_lists_every_story_its_commits_name() {
        let mut map = shop(
            2,
            vec![commit(
                "feat: two stories (#80)",
                Some(80),
                &["STORY-015", "STORY-016"],
            )],
            vec![Some(0), Some(0)],
        );
        map.contexts = Some(BoundedContexts {
            stories: BTreeMap::from([(
                "STORY-015".to_owned(),
                StoryText {
                    title: "Relationship lenses".to_owned(),
                    so_that: "I see what connects".to_owned(),
                },
            )]),
            ..Default::default()
        });

        let why = why_of(&map, &tower()).expect("a why for the tower");

        let listed: Vec<String> = why.built_by[0]
            .stories
            .iter()
            .map(Story::label)
            .collect::<Vec<_>>();
        assert_eq!(
            listed,
            [
                "STORY-015: Relationship lenses".to_owned(),
                // Known to no local issue.md: the id alone.
                "STORY-016".to_owned(),
            ]
        );
        assert!(!why.without_story());
    }

    #[test]
    fn floors_of_commits_without_a_pull_request_are_one_group() {
        let map = shop(
            3,
            vec![
                commit("initial import", None, &[]),
                commit("tidy up", None, &[]),
                commit("feat: a thing (#12)", Some(12), &["STORY-012"]),
            ],
            vec![Some(0), Some(1), Some(2)],
        );

        let why = why_of(&map, &tower()).expect("a why for the tower");

        let groups: Vec<(String, usize)> = why
            .built_by
            .iter()
            .map(|share| (share.key.label(), share.floors))
            .collect();
        assert_eq!(
            groups,
            [
                ("Commits without a pull request".to_owned(), 2),
                ("#12".to_owned(), 1),
            ]
        );
        assert_eq!(
            why.built_by[0].commits,
            ["initial import".to_owned(), "tidy up".to_owned()],
            "both subjects, in order of first mention"
        );
        assert!(!why.without_story(), "#12 names a story");
    }

    #[test]
    fn uncommitted_floors_are_counted_as_not_committed_yet() {
        let map = shop(
            3,
            vec![commit("chore: old code", None, &[])],
            vec![Some(0), None, None],
        );

        let why = why_of(&map, &tower()).expect("a why for the tower");

        let groups: Vec<(String, usize)> = why
            .built_by
            .iter()
            .map(|share| (share.key.label(), share.floors))
            .collect();
        assert_eq!(
            groups,
            [
                ("Not committed yet".to_owned(), 2),
                ("Commits without a pull request".to_owned(), 1),
            ]
        );
        assert!(why.built_by[0].commits.is_empty(), "nothing to list");
        assert!(
            why.without_story(),
            "the one commit behind it names no story"
        );
    }

    #[test]
    fn a_tower_in_no_context_has_none() {
        let mut map = shop(1, Vec::new(), vec![None]);
        map.contexts = Some(BoundedContexts {
            contexts: vec![BoundedContext {
                name: "Pricing".to_owned(),
                code: vec![PathBuf::from("/p/pricing")],
                description: "How much a customer pays.".to_owned(),
            }],
            ..Default::default()
        });

        let why = why_of(&map, &tower()).expect("a why for the tower");

        assert_eq!(why.context, None);
    }

    #[test]
    fn decisions_follow_the_context_or_are_cross_cutting() {
        let mut map = shop(1, Vec::new(), vec![None]);
        let decision = |id: &str, scope: Scope| Decision {
            id: id.to_owned(),
            title: format!("{id} title"),
            line: format!("{id} decides."),
            scope,
        };
        map.contexts = Some(BoundedContexts {
            contexts: vec![BoundedContext {
                name: "Pricing".to_owned(),
                code: vec![PathBuf::from("/p/catalog")],
                description: "How much a customer pays.".to_owned(),
            }],
            decisions: vec![
                decision("ADR-0001", Scope::CrossCutting),
                decision("ADR-0003", Scope::Contexts(vec!["Pricing".to_owned()])),
                decision("ADR-0006", Scope::Contexts(vec!["Ordering".to_owned()])),
            ],
            ..Default::default()
        });

        let why = why_of(&map, &tower()).expect("a why for the tower");

        assert_eq!(
            why.context,
            Some(("Pricing".to_owned(), "How much a customer pays.".to_owned()))
        );
        let bound: Vec<&str> = why.decisions.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(bound, ["ADR-0001", "ADR-0003"], "not the Ordering one");
    }

    #[test]
    fn nothing_is_explained_for_an_element_that_is_not_a_tower() {
        let map = shop(1, Vec::new(), vec![None]);

        assert_eq!(why_of(&map, Path::new("/p/catalog")), None);
        assert_eq!(why_of(&map, Path::new("/p/catalog/discount.rs")), None);
    }
}
