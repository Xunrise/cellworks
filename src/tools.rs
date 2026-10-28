//! The tools Claude uses to read a project and build its lenses, run from
//! the command line without a window.

use std::path::Path;

use cellworks_code_analysis::{CodeAnalysisEvent, GitCli, load};
use cellworks_map::{ProjectMap, default_prefix_rule, project};

/// The folders map of the project at `repo`, as the app reads it.
fn read(repo: &str) -> Result<ProjectMap, String> {
    let root = Path::new(repo)
        .canonicalize()
        .map_err(|e| format!("{repo}: {e}"))?;
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    load(&root, &name, default_prefix_rule(), &GitCli, false)
        .into_iter()
        .find_map(|event| match event {
            CodeAnalysisEvent::ProjectMapped { map, .. } => Some(*map),
            _ => None,
        })
        .ok_or_else(|| format!("{repo}: could not be read"))
}

/// What Cellworks knows about the project at `repo`, as pretty JSON.
pub fn facts(repo: &str, kind: &str) -> Result<String, String> {
    let map = read(repo)?;
    serde_json::to_string_pretty(&project::facts(&map, kind)).map_err(|e| e.to_string())
}

/// Builds lens `name` of the project at `repo`, writes its data, and says
/// what was made and what is left unmatched.
pub fn lens_build(repo: &str, name: &str) -> Result<String, String> {
    let map = read(repo)?;
    let def = project::load(&map.root, name)?.def;
    let data = project::build(&map, &def);
    let text = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(project::data_path(&map.root, name), text + "\n").map_err(|e| e.to_string())?;
    let mut districts: Vec<&str> = data.entities.iter().map(|e| e.district.as_str()).collect();
    districts.sort();
    districts.dedup();
    let mut kinds = std::collections::BTreeMap::<&str, usize>::new();
    for l in &data.relations {
        *kinds.entry(l.kind.as_str()).or_default() += 1;
    }
    let report = serde_json::json!({
        "files": data.entities.len(),
        "districts": districts,
        "underground": data.entities.iter().filter(|e| e.underground).count(),
        "relations": kinds,
        "unmatched": data.unmatched,
    });
    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
}
