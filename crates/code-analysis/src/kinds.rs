//! What kind of file a file is, told by its name: the map stands each kind
//! as a building of its own.

use std::path::Path;

use cellworks_map::NodeKind;

/// The kind of the file at `file`, by its name and extension; code when
/// nothing else fits.
pub(crate) fn kind_of(file: &Path) -> NodeKind {
    let name = file
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or_default();
    let generated = name.ends_with(".lock")
        || matches!(name.as_str(), "package-lock.json" | "pnpm-lock.yaml")
        || extension == "snap";
    if generated {
        NodeKind::Generated
    } else if name.starts_with(".env") {
        let example = ["example", "sample", "template"]
            .iter()
            .any(|word| name.contains(word));
        if example {
            NodeKind::SecretsExample
        } else {
            NodeKind::Secrets
        }
    } else if name.starts_with("dockerfile")
        || matches!(name.as_str(), "makefile" | "justfile")
        || extension == "sh"
    {
        NodeKind::Build
    } else {
        match extension {
            "md" | "markdown" | "rst" => NodeKind::Document,
            "yaml" | "yml" | "json" | "toml" => NodeKind::Configuration,
            "feature" => NodeKind::Specification,
            "tf" | "tfvars" => NodeKind::Infrastructure,
            "proto" => NodeKind::Contract,
            "sql" => NodeKind::Schema,
            "png" | "svg" | "jpg" | "jpeg" | "gif" | "ico" | "webp" => NodeKind::Image,
            _ => NodeKind::Code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_file_is_told_by_its_name() {
        let table = [
            ("Cargo.lock", NodeKind::Generated),
            ("web/package-lock.json", NodeKind::Generated),
            ("pnpm-lock.yaml", NodeKind::Generated),
            ("tests/snapshots/a.snap", NodeKind::Generated),
            (".env", NodeKind::Secrets),
            (".env.production", NodeKind::Secrets),
            (".env.example", NodeKind::SecretsExample),
            (".env.sample", NodeKind::SecretsExample),
            (".env.template", NodeKind::SecretsExample),
            ("README.md", NodeKind::Document),
            ("docs/guide.markdown", NodeKind::Document),
            ("docs/index.rst", NodeKind::Document),
            ("config.yaml", NodeKind::Configuration),
            (".github/workflows/ci.yml", NodeKind::Configuration),
            ("package.json", NodeKind::Configuration),
            ("Cargo.toml", NodeKind::Configuration),
            ("checkout.feature", NodeKind::Specification),
            ("main.tf", NodeKind::Infrastructure),
            ("prod.tfvars", NodeKind::Infrastructure),
            ("Dockerfile", NodeKind::Build),
            ("Dockerfile.dev", NodeKind::Build),
            ("Makefile", NodeKind::Build),
            ("justfile", NodeKind::Build),
            ("scripts/release.sh", NodeKind::Build),
            ("api.proto", NodeKind::Contract),
            ("schema.sql", NodeKind::Schema),
            ("logo.png", NodeKind::Image),
            ("icon.svg", NodeKind::Image),
            ("photo.JPG", NodeKind::Image),
            ("favicon.ico", NodeKind::Image),
            ("src/main.rs", NodeKind::Code),
            ("web/app.ts", NodeKind::Code),
            ("LICENSE", NodeKind::Code),
        ];

        for (file, kind) in table {
            assert_eq!(kind_of(Path::new(file)), kind, "{file}");
        }
    }
}
