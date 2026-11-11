//! STORY-067, files that are not code stand as buildings of their kind: a
//! document is a library hall with a bay per section, a configuration file a
//! switchboard of cabinets, a feature file a courthouse with a column per
//! scenario, Terraform a plant, a build file a depot, a contract a customs
//! house, a lockfile a ghost, a schema a slab sunk below the ground, an image
//! a billboard, and secrets a sealed vault whose lines are never shown.
//!
//! One test per scenario of issue #94, named after it:
//! - A document is a library hall with a bay per section
//! - A configuration file is a switchboard of cabinets
//! - A feature file is a courthouse with a column per scenario
//! - Other kinds of file stand as their building (one test looping the
//!   examples, each in a project of its own)
//! - Secrets are a sealed vault whose values are never shown
//!
//! The project "shop" is a real folder tree in a temporary directory. No
//! test runs git, reads `$HOME` or opens a window.

mod common;

use std::path::{Path, PathBuf};

use cellworks::app::CellworksApp;
use cellworks_map::ui::DrawnBuilding;
use cellworks_map::{Form, ItemKind, city};
use egui::{Event, Key, Modifiers};
use egui_kittest::Harness;

use common::{Recorded, Tree, labels, map, open, tower};

const README: &str = "\
# Shop
A shop.

## Usage
Run it.
Then use it.
Then stop it.

## Licence
MIT, see the licence file, which says what may be done with the shop and
what may not,
and who to ask.
";

const CONFIG: &str = "\
server:
  port: 8080
database:
  host: db
  pool:
    size: 5
";

const CHECKOUT: &str = "\
Feature: Checkout
  Background:
    Given a cart

  Scenario: Pay by card
    When I pay by card
    Then the order is placed

  Scenario: Pay by invoice
    When I pay by invoice
    Then the order waits for the payment
";

const MAIN_TF: &str = "\
terraform {
  required_version = \">= 1.5\"
}

provider \"aws\" {
  region = \"eu-central-1\"
}

resource \"aws_s3_bucket\" \"logs\" {
  bucket = \"shop-logs\"
  tags = {
    team = \"shop\"
  }
}
";

const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0xff];

/// The background: the project "shop", holding `files`, is open as a city.
fn shop(files: &[(&str, &str)]) -> (Tree, Harness<'static, CellworksApp>) {
    let tree = Tree::sources("shop", files);
    let mut harness = open(&tree, None, Recorded::listing(&[]));
    // The Explorer, open beside the map by default (STORY-089), is not what
    // this story's buildings are about, and zooming in to face one keeps
    // asking for frames well past the harness's step budget while it is
    // open.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::B);
    harness.run();
    (tree, harness)
}

/// The building drawn with its base at `path`.
fn building(harness: &Harness<'_, CellworksApp>, path: &Path) -> DrawnBuilding {
    harness
        .state()
        .buildings()
        .iter()
        .find(|building| building.path == path)
        .cloned()
        .unwrap_or_else(|| panic!("no building on {}", path.display()))
}

/// A document is a library hall with a bay per section
#[test]
fn a_document_is_a_library_hall_with_a_bay_per_section() {
    let (tree, harness) = shop(&[("README.md", README)]);
    let readme = tree.root.join("README.md");

    let library = building(&harness, &readme.join("Shop"));

    assert_eq!(library.form, Form::Library, "a hall with a colonnade");
    let map = map(&harness);
    let bays = map.items_of(&readme).expect("a block for README.md");
    let names: Vec<&str> = bays.iter().map(|bay| bay.name.as_str()).collect();
    assert_eq!(names, ["Shop", "Usage", "Licence"], "three bays");
    let mut tiers = vec![bays[0].path.clone()];
    tiers.extend(library.tiers.iter().cloned());
    let paths: Vec<PathBuf> = bays.iter().map(|bay| bay.path.clone()).collect();
    assert_eq!(tiers, paths, "every section a bay of the one library");
    for bay in bays {
        assert_eq!(bay.kind, ItemKind::Section);
        assert!(bay.base > 0.0, "{} stands on the hall's roof", bay.name);
        assert_eq!(bay.base, bays[0].base, "all on the same roof");
        assert_eq!(
            city::reach(bay),
            bay.floors.len() as f32 * city::FLOOR,
            "{} is as tall as its section",
            bay.name
        );
    }
    assert_eq!(
        bays.iter().map(|bay| bay.floors.len()).collect::<Vec<_>>(),
        [3, 5, 4]
    );
    for pair in bays.windows(2) {
        assert!(
            pair[0].area.max.x < pair[1].area.min.x,
            "{} stands beside {}",
            pair[1].name,
            pair[0].name
        );
    }
}

/// A configuration file is a switchboard of cabinets
#[test]
fn a_configuration_file_is_a_switchboard_of_cabinets() {
    let (tree, harness) = shop(&[("config.yaml", CONFIG)]);
    let config = tree.root.join("config.yaml");

    let cabinets: Vec<DrawnBuilding> = harness
        .state()
        .buildings()
        .iter()
        .filter(|building| building.path.starts_with(&config))
        .cloned()
        .collect();

    let mut paths: Vec<&Path> = cabinets.iter().map(|c| c.path.as_path()).collect();
    paths.sort();
    assert_eq!(
        paths,
        [config.join("database"), config.join("server")],
        "two cabinets"
    );
    assert!(cabinets.iter().all(|c| c.form == Form::Switchboard));
    let database = building(&harness, &config.join("database"));
    assert_eq!(
        database.tiers,
        [config.join("database.1"), config.join("database.2")],
        "its two nested levels"
    );
    let map = map(&harness);
    let mut below = map.item(&config.join("database")).expect("the cabinet");
    for level in &database.tiers {
        let level = map.item(level).expect("the level is on the map");
        assert!(
            below.area.contains_rect(level.area) && level.area.width() < below.area.width(),
            "{} is stacked on {}, smaller",
            level.name,
            below.name
        );
        assert!((level.base - city::height(below)).abs() < 1e-4);
        below = level;
    }
}

/// A feature file is a courthouse with a column per scenario
#[test]
fn a_feature_file_is_a_courthouse_with_a_column_per_scenario() {
    let (tree, harness) = shop(&[("checkout.feature", CHECKOUT)]);
    let checkout = tree.root.join("checkout.feature");
    let header = checkout.join("Feature: Checkout");

    let courthouse = building(&harness, &header);

    assert_eq!(courthouse.form, Form::Courthouse);
    assert_eq!(
        courthouse.tiers,
        [
            checkout.join("Scenario: Pay by card"),
            checkout.join("Scenario: Pay by invoice")
        ],
        "two columns"
    );
    let map = map(&harness);
    let hall = map.item(&header).expect("the hall");
    assert!(hall.base > 0.0, "the hall stands on a plinth");
    for column in &courthouse.tiers {
        let column = map.item(column).expect("the column");
        assert!(
            column.area.min.y > hall.area.max.y,
            "{} stands in front of the hall",
            column.name
        );
        assert_eq!(column.base, hall.base, "on the same plinth");
    }
}

/// Other kinds of file stand as their building
#[test]
fn other_kinds_of_file_stand_as_their_building() {
    let examples: [(&str, &[u8], &str, Form); 6] = [
        (
            "main.tf",
            MAIN_TF.as_bytes(),
            "resource \"aws_s3_bucket\" \"logs\"",
            Form::Plant,
        ),
        (
            "Dockerfile",
            b"FROM rust:1.90\nRUN cargo build --release\n",
            "Dockerfile",
            Form::Depot,
        ),
        (
            "api.proto",
            b"syntax = \"proto3\";\nmessage Order {\n  string id = 1;\n}\n",
            "api.proto",
            Form::CustomsHouse,
        ),
        (
            "Cargo.lock",
            b"version = 4\n\n[[package]]\nname = \"shop\"\n",
            "Cargo.lock",
            Form::Ghost,
        ),
        (
            "schema.sql",
            b"CREATE TABLE orders (\n  id INT PRIMARY KEY\n);\n",
            "schema.sql",
            Form::Slab,
        ),
        ("logo.png", PNG, "", Form::Billboard),
    ];

    for (file, bytes, base, form) in examples {
        let tree = Tree::sources("shop", &[]);
        std::fs::write(tree.root.join(file), bytes).expect("write the file");
        let harness = open(&tree, None, Recorded::listing(&[]));
        let node = tree.root.join(file);
        let path = if base.is_empty() {
            node.clone()
        } else {
            node.join(base)
        };

        let drawn = building(&harness, &path);

        assert_eq!(drawn.form, form, "{file}");
        let map = map(&harness);
        match form {
            Form::Plant => {
                // The largest block is the body.
                let blocks = map.items_of(&node).expect("a block").len();
                assert_eq!(blocks, 3);
                assert_eq!(drawn.tiers.len(), blocks - 1, "a chimney per further block");
                let body = map.item(&path).expect("the body");
                for chimney in &drawn.tiers {
                    let chimney = map.item(chimney).expect("a chimney");
                    assert!(body.area.contains_rect(chimney.area));
                    assert_eq!(chimney.base, city::height(body), "on the body's roof");
                }
            }
            Form::Slab => {
                let slab = map.item(&path).expect("the slab");
                assert!(slab.base < 0.0, "sunk below the ground");
                assert!(city::height(slab).abs() < 1e-4, "its top at the ground");
            }
            Form::Billboard => {
                assert_eq!(map.items_of(&node).map(<[_]>::len), Some(0));
            }
            _ => {}
        }
    }
}

/// Secrets are a sealed vault whose values are never shown
#[test]
fn secrets_are_a_sealed_vault_whose_values_are_never_shown() {
    let (tree, mut harness) = shop(&[
        (".env", "API_KEY=hunter2\nDB_PASSWORD=swordfish\n"),
        (".env.example", "API_KEY=\nDB_PASSWORD=\n"),
    ]);
    let env = tree.root.join(".env").join(".env");
    let example = tree.root.join(".env.example").join(".env.example");

    assert_eq!(
        building(&harness, &env).form,
        Form::Vault { glass: false },
        "a sealed vault"
    );
    assert_eq!(
        building(&harness, &example).form,
        Form::Vault { glass: true },
        "the same vault in glass"
    );
    let vault = map(&harness).item(&env).expect("the vault").clone();
    assert_eq!(vault.floors.len(), 2, "a floor per key");
    for floor in &vault.floors {
        assert!(
            floor.text.is_empty() && floor.tokens.is_empty(),
            "no value held"
        );
    }

    // From afar, and close enough to read every other line.
    harness.state_mut().jump(env.clone());
    harness.run();
    let scale = |harness: &Harness<'_, CellworksApp>| {
        harness.state().view().expect("the city is drawn").scale()
    };
    let mut seen_close = false;
    for _ in 0..80 {
        assert_eq!(
            tower(&harness, &env).floors.len(),
            1,
            "no line of the vault is drawn at scale {}",
            scale(&harness)
        );
        for secret in ["hunter2", "swordfish", "API_KEY"] {
            assert!(
                labels(&harness).iter().all(|label| !label.contains(secret)),
                "{secret} is shown"
            );
        }
        if scale(&harness) >= 20.0 {
            seen_close = true;
            break;
        }
        let at = tower(&harness, &env).outline.center();
        harness.hover_at(at);
        harness.event(Event::Zoom(1.25));
        harness.run();
    }
    assert!(seen_close, "zoomed close enough to read lines");
}
