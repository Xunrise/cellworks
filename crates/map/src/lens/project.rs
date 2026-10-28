//! Project lenses: a lens a project defines for itself in
//! `.cellworks/lenses/<name>.lens.json`, built into
//! `.cellworks/lenses/<name>.data.json`, and drawn from that data.
//!
//! The definition holds the rules (which district and block a file stands in,
//! what lies underground, which relations hold between files), the one-off
//! overrides, and how each relation is drawn. The data is what the rules made
//! of the project's files; the map reads only that, plus the drawing choices.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use egui::Rect;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::cluster::{Weights, link};
use super::shape;
use crate::graph::{self, District, DistrictKind, Item, ItemKind, Node, ProjectMap, SubDistrict};
use crate::layout;

/// A lens definition, as Claude writes it and the engineer tweaks it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LensDef {
    pub name: String,
    /// The question the lens answers.
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub cut: Cut,
    /// Regexes over the relative path: those files are left off the map.
    #[serde(default)]
    pub hide: Vec<String>,
    /// First match wins; a file no rule places stays in its folder district.
    #[serde(default)]
    pub districts: Vec<Place>,
    /// Blocks inside a district; first match wins, none means no block.
    #[serde(default)]
    pub blocks: Vec<Place>,
    /// Regexes over the relative path: those files lie underground.
    #[serde(default)]
    pub underground: Vec<String>,
    #[serde(default)]
    pub relations: Vec<Relation>,
    #[serde(default)]
    pub overrides: Overrides,
}

/// How the city is cut into buildings.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cut {
    /// One tower per function and type (true) or one per file (false).
    #[serde(default = "yes")]
    pub split_files: bool,
}

impl Default for Cut {
    fn default() -> Self {
        Cut { split_files: true }
    }
}

fn yes() -> bool {
    true
}

/// A path rule: files whose relative path matches `match` go to the district
/// or block `as`, with `$1` or `$name` replaced by the captures.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Place {
    #[serde(rename = "match")]
    pub pattern: String,
    #[serde(rename = "as")]
    pub template: String,
}

/// A kind of relation between files, how it is found, and how it is drawn.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    pub kind: String,
    /// Path pairs: a file matching `from` relates to the file `to` names.
    #[serde(default)]
    pub pairs: Vec<Pair>,
    /// A fact Cellworks knows: `calls`, `imports` or `cochange`.
    #[serde(default)]
    pub source: Option<String>,
    /// Keeps only the fact links whose `from` file matches this regex.
    #[serde(default)]
    pub only_from: Option<String>,
    /// Links lighter than this are dropped.
    #[serde(default)]
    pub min_weight: f32,
    pub viz: Viz,
}

/// A file matching `from` relates to the file `to` names, `$1`/`$name`
/// replaced by the captures.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    pub from: String,
    pub to: String,
}

/// How a relation is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Viz {
    /// The `from` file hangs underground beneath its `to` file.
    Cellar,
    /// Districts linked by it pull together.
    Cluster,
    /// The district most linked by it in the middle, the others round it.
    Orbit,
    /// A road between the two districts, labelled with the kind.
    Roads,
    /// A lane between the two files.
    Lanes,
    /// Not drawn.
    #[default]
    None,
}

impl Viz {
    pub const ALL: [Viz; 6] = [
        Viz::Cellar,
        Viz::Cluster,
        Viz::Orbit,
        Viz::Roads,
        Viz::Lanes,
        Viz::None,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Viz::Cellar => "cellar",
            Viz::Cluster => "cluster",
            Viz::Orbit => "orbit",
            Viz::Roads => "roads",
            Viz::Lanes => "lanes",
            Viz::None => "none",
        }
    }
}

/// The edge cases the rules do not settle, settled once by hand.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overrides {
    /// Relative file path to where it stands.
    #[serde(default)]
    pub place: BTreeMap<String, Placed>,
    #[serde(default)]
    pub links: Vec<Link>,
    /// Relative paths that lie underground, or explicitly above ground.
    #[serde(default)]
    pub underground: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placed {
    pub district: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
}

/// One relation between two files, by relative path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub kind: String,
    pub from: String,
    pub to: String,
    #[serde(default = "one")]
    pub weight: f32,
}

fn one() -> f32 {
    1.0
}

/// What a definition made of a project's files.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LensData {
    pub lens: String,
    /// A hash of the definition's rules and overrides: the data is stale
    /// when it no longer matches.
    pub rules: String,
    pub entities: Vec<Entity>,
    pub relations: Vec<Link>,
    pub unmatched: Unmatched,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub file: String,
    pub district: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    #[serde(default)]
    pub underground: bool,
}

/// The queue of edge cases for Claude.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Unmatched {
    /// Files no district rule or override placed: they stay in their folder
    /// district.
    pub unplaced: Vec<String>,
    /// For each relation kind, files a pair matched whose target is not a
    /// file of the project, with the target it named.
    pub missing_targets: BTreeMap<String, Vec<(String, String)>>,
    /// Underground files no cellar relation hangs under another file.
    pub unhung: Vec<String>,
    /// Rules that do not compile, with the error.
    pub bad_rules: Vec<String>,
}

/// A project lens, defined and built.
#[derive(Clone, Debug, Default)]
pub struct ProjectLens {
    pub def: LensDef,
    pub data: LensData,
}

impl ProjectLens {
    /// Whether the data was built from other rules than the definition's.
    pub fn stale(&self) -> bool {
        self.data.rules != self.def.rules_key()
    }
}

impl LensDef {
    /// A hash of everything that decides the data, not how it is drawn.
    pub fn rules_key(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut rules = self.clone();
        rules.name.clear();
        rules.question.clear();
        rules.cut = Cut::default();
        for relation in &mut rules.relations {
            relation.viz = Viz::None;
        }
        let text = serde_json::to_string(&rules).unwrap_or_default();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// The viz of the relation of `kind`.
    pub fn viz_of(&self, kind: &str) -> Viz {
        self.relations
            .iter()
            .find(|r| r.kind == kind)
            .map_or(Viz::None, |r| r.viz)
    }
}

/// The folder a project's lenses live in.
pub fn lenses_dir(root: &Path) -> PathBuf {
    root.join(".cellworks").join("lenses")
}

/// The definition file of lens `name`.
pub fn def_path(root: &Path, name: &str) -> PathBuf {
    lenses_dir(root).join(format!("{name}.lens.json"))
}

/// The data file of lens `name`.
pub fn data_path(root: &Path, name: &str) -> PathBuf {
    lenses_dir(root).join(format!("{name}.data.json"))
}

/// The names of the lenses the project at `root` defines, sorted.
pub fn list(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(lenses_dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_suffix(".lens.json")
                .map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

/// Reads lens `name` of the project at `root`; data that is missing or
/// unreadable is empty.
pub fn load(root: &Path, name: &str) -> Result<ProjectLens, String> {
    let text = std::fs::read_to_string(def_path(root, name)).map_err(|e| e.to_string())?;
    let def: LensDef = serde_json::from_str(&text).map_err(|e| format!("{name}.lens.json: {e}"))?;
    let data = match std::fs::read_to_string(data_path(root, name)) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{name}.data.json: {e}"))?,
        Err(_) => LensData::default(),
    };
    Ok(ProjectLens { def, data })
}

/// Writes the definition back, as the engineer changed how it is drawn.
pub fn save_def(root: &Path, name: &str, def: &LensDef) -> Result<(), String> {
    let text = serde_json::to_string_pretty(def).map_err(|e| e.to_string())?;
    std::fs::write(def_path(root, name), text + "\n").map_err(|e| e.to_string())
}

/// The relative path of every file of `folders`, with its folder district.
fn files(folders: &ProjectMap) -> Vec<(String, String)> {
    folders
        .districts
        .iter()
        .flat_map(|d| {
            graph::nodes(d)
                .into_iter()
                .map(move |n| (relative(&folders.root, &n.path), d.name.clone()))
        })
        .collect()
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn compiled(pattern: &str, bad: &mut Vec<String>) -> Option<Regex> {
    match Regex::new(pattern) {
        Ok(re) => Some(re),
        Err(e) => {
            bad.push(format!("{pattern}: {e}"));
            None
        }
    }
}

fn place(rules: &[(Regex, &str)], file: &str) -> Option<String> {
    rules.iter().find_map(|(re, template)| {
        let caps = re.captures(file)?;
        let mut out = String::new();
        caps.expand(template, &mut out);
        Some(out)
    })
}

/// A target with `**/` names a file by its prefix and its last parts: the
/// shortest known path that has both.
fn found(known: &BTreeSet<&str>, target: &str) -> Option<String> {
    let (prefix, suffix) = target.split_once("**/")?;
    known
        .iter()
        .filter(|f| {
            f.starts_with(prefix)
                && (f.ends_with(&format!("/{suffix}")) || **f == format!("{prefix}{suffix}"))
        })
        .min_by_key(|f| f.len())
        .map(|f| (*f).to_owned())
}

/// Applies the rules of `def` to the files of `folders`.
pub fn build(folders: &ProjectMap, def: &LensDef) -> LensData {
    let mut unmatched = Unmatched::default();
    let bad = &mut unmatched.bad_rules;
    let districts: Vec<(Regex, &str)> = def
        .districts
        .iter()
        .filter_map(|p| Some((compiled(&p.pattern, bad)?, p.template.as_str())))
        .collect();
    let blocks: Vec<(Regex, &str)> = def
        .blocks
        .iter()
        .filter_map(|p| Some((compiled(&p.pattern, bad)?, p.template.as_str())))
        .collect();
    let underground: Vec<Regex> = def
        .underground
        .iter()
        .filter_map(|p| compiled(p, bad))
        .collect();

    let hide: Vec<Regex> = def
        .hide
        .iter()
        .filter_map(|p| compiled(p, &mut unmatched.bad_rules))
        .collect();
    let mut all = files(folders);
    all.retain(|(f, _)| !hide.iter().any(|re| re.is_match(f)));
    let known: BTreeSet<&str> = all.iter().map(|(f, _)| f.as_str()).collect();
    let mut entities = Vec::new();
    for (file, folder_district) in &all {
        let placed = def.overrides.place.get(file).cloned().or_else(|| {
            Some(Placed {
                district: place(&districts, file)?,
                block: place(&blocks, file),
            })
        });
        let placed = placed.unwrap_or_else(|| {
            if !districts.is_empty() {
                unmatched.unplaced.push(file.clone());
            }
            Placed {
                district: folder_district.clone(),
                block: place(&blocks, file),
            }
        });
        let below = def
            .overrides
            .underground
            .get(file)
            .copied()
            .unwrap_or_else(|| underground.iter().any(|re| re.is_match(file)));
        entities.push(Entity {
            file: file.clone(),
            district: placed.district,
            block: placed.block,
            underground: below,
        });
    }

    let mut relations: Vec<Link> = Vec::new();
    for relation in &def.relations {
        let pairs: Vec<(Regex, &str)> = relation
            .pairs
            .iter()
            .filter_map(|p| Some((compiled(&p.from, &mut unmatched.bad_rules)?, p.to.as_str())))
            .collect();
        let overridden: BTreeSet<&str> = def
            .overrides
            .links
            .iter()
            .filter(|l| l.kind == relation.kind)
            .map(|l| l.from.as_str())
            .collect();
        for (file, _) in &all {
            if overridden.contains(file.as_str()) {
                continue;
            }
            let Some(target) = place(&pairs, file) else {
                continue;
            };
            let target = found(&known, &target).unwrap_or(target);
            if known.contains(target.as_str()) {
                relations.push(Link {
                    kind: relation.kind.clone(),
                    from: file.clone(),
                    to: target,
                    weight: 1.0,
                });
            } else {
                unmatched
                    .missing_targets
                    .entry(relation.kind.clone())
                    .or_default()
                    .push((file.clone(), target));
            }
        }
        if let Some(source) = &relation.source {
            let only = relation
                .only_from
                .as_deref()
                .and_then(|p| compiled(p, &mut unmatched.bad_rules));
            for (from, to, weight) in fact_links(folders, source) {
                if only.as_ref().is_some_and(|re| !re.is_match(&from))
                    || overridden.contains(from.as_str())
                {
                    continue;
                }
                if weight >= relation.min_weight
                    && from != to
                    && known.contains(from.as_str())
                    && known.contains(to.as_str())
                {
                    relations.push(Link {
                        kind: relation.kind.clone(),
                        from,
                        to,
                        weight,
                    });
                }
            }
        }
    }
    relations.extend(def.overrides.links.iter().cloned());

    let hung: BTreeSet<&str> = relations
        .iter()
        .filter(|l| def.viz_of(&l.kind) == Viz::Cellar)
        .map(|l| l.from.as_str())
        .collect();
    unmatched.unhung = entities
        .iter()
        .filter(|e| e.underground && !hung.contains(e.file.as_str()))
        .map(|e| e.file.clone())
        .collect();

    LensData {
        lens: def.name.clone(),
        rules: def.rules_key(),
        entities,
        relations,
        unmatched,
    }
}

/// File-to-file links from what the loaders read: calls and imports summed
/// per pair of files, co-change as git weighs it.
fn fact_links(folders: &ProjectMap, source: &str) -> Vec<(String, String, f32)> {
    let root = &folders.root;
    if source == "cochange" {
        return folders
            .co_changes
            .iter()
            .map(|c| {
                (
                    relative(root, &c.files.0),
                    relative(root, &c.files.1),
                    c.weight,
                )
            })
            .collect();
    }
    let mut sums: BTreeMap<(String, String), f32> = BTreeMap::new();
    for district in &folders.districts {
        for item in graph::items(district) {
            let Some(from) = item.path.parent() else {
                continue;
            };
            for floor in &item.floors {
                let targets = match source {
                    "calls" => &floor.calls,
                    "imports" => &floor.imports,
                    _ => continue,
                };
                for target in targets {
                    if let Some(to) = target.parent()
                        && to != from
                    {
                        *sums
                            .entry((relative(root, from), relative(root, to)))
                            .or_default() += 1.0;
                    }
                }
            }
        }
    }
    sums.into_iter().map(|((a, b), w)| (a, b, w)).collect()
}

/// What Cellworks knows about the project, as JSON for Claude to query.
pub fn facts(folders: &ProjectMap, kind: &str) -> serde_json::Value {
    use serde_json::json;
    let root = &folders.root;
    match kind {
        "files" => json!(
            folders
                .districts
                .iter()
                .flat_map(|d| graph::nodes(d).into_iter().map(move |n| json!({
                    "file": relative(root, &n.path),
                    "folder_district": d.name,
                    "kind": format!("{:?}", n.kind),
                    "items": n.items.len(),
                    "tests": n.items.iter().filter(|i| i.tests).count(),
                })))
                .collect::<Vec<_>>()
        ),
        "items" => json!(
            folders
                .districts
                .iter()
                .flat_map(graph::items)
                .map(|i| json!({
                    "item": relative(root, &i.path),
                    "kind": format!("{:?}", i.kind),
                    "tests": i.tests,
                    "lines": i.floors.first().map(|f| f.number)
                        .zip(i.floors.last().map(|f| f.number)),
                }))
                .collect::<Vec<_>>()
        ),
        source => json!(
            fact_links(folders, source)
                .into_iter()
                .map(|(from, to, weight)| json!({"from": from, "to": to, "weight": weight}))
                .collect::<Vec<_>>()
        ),
    }
}

/// One tower for all of `items`: a file's code as one building.
fn whole(items: Vec<Item>, node: &Node, tests: bool) -> Option<Item> {
    let mut floors: Vec<_> = items.iter().flat_map(|i| i.floors.clone()).collect();
    if floors.is_empty() {
        return None;
    }
    floors.sort_by_key(|f| f.number);
    floors.dedup_by_key(|f| f.number);
    let mut item = items.into_iter().next()?;
    item.name = node.name.clone();
    item.path = node.path.join(if tests { "(tests)" } else { "(file)" });
    item.kind = if tests {
        ItemKind::Test
    } else {
        ItemKind::Text
    };
    item.tests = tests;
    item.floors = floors;
    item.declaration = Default::default();
    Some(item)
}

/// `folders` seen through the project lens `lens`.
pub(crate) fn seen(folders: &ProjectMap, lens: &ProjectLens) -> ProjectMap {
    let mut map = folders.clone();
    map.lens = crate::Lens::Project;
    map.homes = super::homes(folders);
    let root = folders.root.clone();
    let def = &lens.def;

    let mut nodes: HashMap<String, Node> = folders
        .districts
        .iter()
        .flat_map(graph::nodes)
        .map(|n| (relative(&root, &n.path), n.clone()))
        .collect();

    for entity in &lens.data.entities {
        if entity.underground
            && let Some(node) = nodes.get_mut(&entity.file)
        {
            for item in &mut node.items {
                item.tests = true;
            }
        }
    }
    if !def.cut.split_files {
        for node in nodes.values_mut() {
            let (below, above): (Vec<Item>, Vec<Item>) = std::mem::take(&mut node.items)
                .into_iter()
                .partition(|i| i.tests);
            node.items = [whole(above, node, false), whole(below, node, true)]
                .into_iter()
                .flatten()
                .collect();
        }
    }

    // Each underground file hangs beneath the file its heaviest cellar link
    // names.
    let mut hang: BTreeMap<&str, (&str, f32)> = BTreeMap::new();
    for l in &lens.data.relations {
        if def.viz_of(&l.kind) == Viz::Cellar && l.from != l.to {
            let best = hang.entry(&l.from).or_insert((&l.to, l.weight));
            if l.weight > best.1 {
                *best = (&l.to, l.weight);
            }
        }
    }
    let mut hung_under: HashMap<String, String> = HashMap::new();
    for (from, (to, _)) in &hang {
        if !nodes.contains_key(*to) || hang.contains_key(*to) {
            continue;
        }
        if let Some(mut test) = nodes.remove(*from) {
            for item in &mut test.items {
                item.tests = true;
            }
            if let Some(subject) = nodes.get_mut(*to) {
                subject.items.extend(test.items);
            }
            hung_under.insert((*from).to_owned(), (*to).to_owned());
        }
    }

    let base = root.join(".lens").join("project");
    let mut districts: Vec<District> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut district_of: HashMap<String, usize> = HashMap::new();
    let mut entities: Vec<&super::project::Entity> = lens.data.entities.iter().collect();
    entities
        .sort_by(|a, b| (&a.district, &a.block, &a.file).cmp(&(&b.district, &b.block, &b.file)));
    for entity in entities {
        let Some(node) = nodes.remove(&entity.file) else {
            continue;
        };
        let d = *index.entry(entity.district.clone()).or_insert_with(|| {
            districts.push(District {
                name: entity.district.clone(),
                path: base.join(&entity.district),
                sub_districts: Vec::new(),
                nodes: Vec::new(),
                area: Rect::NOTHING,
                kind: DistrictKind::Cluster,
                elevation: 0.0,
            });
            districts.len() - 1
        });
        district_of.insert(entity.file.clone(), d);
        let district = &mut districts[d];
        match &entity.block {
            Some(block) => {
                let path = district.path.join(block);
                let sub = match district.sub_districts.iter().position(|s| s.path == path) {
                    Some(s) => s,
                    None => {
                        district.sub_districts.push(SubDistrict {
                            name: block.clone(),
                            path,
                            groups: Vec::new(),
                            nodes: Vec::new(),
                            area: Rect::NOTHING,
                        });
                        district.sub_districts.len() - 1
                    }
                };
                district.sub_districts[sub].nodes.push(node);
            }
            None => district.nodes.push(node),
        }
    }
    for (from, to) in &hung_under {
        if let Some(&d) = district_of.get(to) {
            district_of.insert(from.clone(), d);
        }
    }

    layout::lay_out(&mut districts, &base);

    let pair_of = |l: &Link| -> Option<(usize, usize)> {
        let (a, b) = (*district_of.get(&l.from)?, *district_of.get(&l.to)?);
        (a != b).then_some((a, b))
    };
    let mut weights = Weights::new();
    let mut orbit = false;
    let mut roads: BTreeMap<(usize, usize, String), f32> = BTreeMap::new();
    let mut links: BTreeSet<(PathBuf, PathBuf, String)> = BTreeSet::new();
    for l in &lens.data.relations {
        match def.viz_of(&l.kind) {
            Viz::Cluster | Viz::Orbit => {
                orbit |= def.viz_of(&l.kind) == Viz::Orbit;
                if let Some((a, b)) = pair_of(l) {
                    link(&mut weights, a, b, l.weight);
                }
            }
            Viz::Roads => {
                if let Some((a, b)) = pair_of(l) {
                    *roads.entry((a, b, l.kind.clone())).or_default() += l.weight;
                }
            }
            Viz::Lanes => {
                let end = |f: &str| root.join(hung_under.get(f).map_or(f, String::as_str));
                if district_of.contains_key(&l.from) && district_of.contains_key(&l.to) {
                    links.insert((end(&l.from), end(&l.to), l.kind.clone()));
                }
            }
            Viz::Cellar | Viz::None => {}
        }
    }
    if orbit {
        map.hub = shape::orbit(&mut districts, &weights);
    } else {
        shape::network(&mut districts, &weights);
    }
    map.roads = roads.into_keys().collect();
    map.links = links.into_iter().filter(|(a, b, _)| a != b).collect();
    map.districts = districts;
    map.extent = map.ground();
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DistrictStrategy;

    fn folders(files: &[&str]) -> ProjectMap {
        let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        let strategy = crate::Directories {
            prefix: crate::default_prefix_rule(),
        };
        ProjectMap::of(
            "p",
            Path::new("/p"),
            &files,
            &strategy as &dyn DistrictStrategy,
        )
    }

    #[test]
    fn a_pair_rule_and_an_override_hang_tests_under_their_subjects() {
        let map = folders(&[
            "app/src/main/kotlin/a/Foo.kt",
            "app/src/test/kotlin/a/FooTest.kt",
            "web/src/cart.ts",
            "web/src/cart.spec.ts",
            "web/test/odd.ts",
        ]);
        let def: LensDef = serde_json::from_str(
            r#"{
              "name": "tests",
              "districts": [{"match": "^(\\w+)/", "as": "$1"}],
              "underground": ["/test/", "\\.spec\\.ts$"],
              "relations": [{"kind": "tests", "viz": "cellar", "pairs": [
                {"from": "^(.*)/src/test/(.*)Test\\.kt$", "to": "$1/src/main/${2}.kt"},
                {"from": "^(.*)\\.spec\\.ts$", "to": "$1.ts"}
              ]}],
              "overrides": {"links": [{"kind": "tests", "from": "web/test/odd.ts", "to": "web/src/cart.ts"}]}
            }"#,
        )
        .unwrap();
        let data = build(&map, &def);
        let tests: Vec<(&str, &str)> = data
            .relations
            .iter()
            .map(|l| (l.from.as_str(), l.to.as_str()))
            .collect();
        assert!(tests.contains(&(
            "app/src/test/kotlin/a/FooTest.kt",
            "app/src/main/kotlin/a/Foo.kt"
        )));
        assert!(tests.contains(&("web/src/cart.spec.ts", "web/src/cart.ts")));
        assert!(tests.contains(&("web/test/odd.ts", "web/src/cart.ts")));
        assert!(data.unmatched.unhung.is_empty(), "{:?}", data.unmatched);
        let seen = seen(&map, &ProjectLens { def, data });
        let names: Vec<&str> = seen.districts.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["app", "web"]);
        // The test files are no blocks of their own.
        assert_eq!(seen.districts[1].nodes.len(), 1);
    }
}
