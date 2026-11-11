//! STORY-058, TypeScript and JavaScript code stands as towers: the
//! functions, classes and imports of a `.ts`/`.tsx`/`.js`/`.mjs` file stand
//! as towers, a method stands named after its class, tests stand in the
//! cellar, and a file that does not parse stands as one tower of its lines.
//!
//! One test per scenario of issue #83, named after it. The project "web" is
//! a real folder tree of TypeScript and JavaScript sources in a temporary
//! directory. No test runs git, reads `$HOME` or opens a window.

mod common;

use cellworks::app::CellworksApp;
use cellworks_map::ItemKind;
use egui_kittest::Harness;

use common::{Recorded, Tree, map, open, tower};

/// "src/cart.ts": an import block, the function "total", the class "Cart"
/// with the method "add", the constant "discount" holding a function, and
/// the type declarations "Priced" (interface), "CartId" (type alias) and
/// "Currency" (enum).
const CART: &str = "\
import { Price } from './price';

export interface Priced {
  price: number;
}

export type CartId = string;

export enum Currency {
  USD,
  EUR,
}

export const discount = (price: number) => price * 0.9;

export class Cart {
  price = 0;

  add(item: number) {
    this.price += item;
  }
}

export function total(prices: number[]): number {
  return prices.reduce((a, b) => a + b, 0);
}
";

/// The background: "web" holds "src/cart.ts" with the function "total", the
/// class "Cart" and an import block, and is open as a city.
fn web(more: &[(&str, &str)]) -> (Tree, Harness<'static, CellworksApp>) {
    let mut files = vec![("src/cart.ts", CART)];
    files.extend_from_slice(more);
    let tree = Tree::sources("web", &files);
    let harness = open(&tree, None, Recorded::listing(&[]));
    (tree, harness)
}

/// "src/cart.ts" stands as the towers "total", "Cart" and its imports
#[test]
fn functions_classes_and_import_blocks_are_towers() {
    let (tree, harness) = web(&[]);

    let map = map(&harness);
    let cart = tree.root.join("src/cart.ts");
    let names: Vec<&str> = map
        .items_of(&cart)
        .expect("a block for cart.ts")
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    for expected in ["total", "Cart", "imports"] {
        assert!(names.contains(&expected), "{names:?}");
    }
}

/// A function held by a constant is a function tower
#[test]
fn a_function_held_by_a_constant_is_a_function_tower() {
    let (tree, harness) = web(&[]);

    let map = map(&harness);
    let cart = tree.root.join("src/cart.ts");
    let discount = map
        .items_of(&cart)
        .expect("a block for cart.ts")
        .iter()
        .find(|item| item.name == "discount")
        .expect("a tower for discount");
    assert_eq!(discount.kind, ItemKind::Function);
}

/// Type declarations are type towers
#[test]
fn type_declarations_are_type_towers() {
    let (tree, harness) = web(&[]);

    let map = map(&harness);
    let cart = tree.root.join("src/cart.ts");
    let items = map.items_of(&cart).expect("a block for cart.ts");
    for name in ["Priced", "CartId", "Currency"] {
        let item = items
            .iter()
            .find(|item| item.name == name)
            .unwrap_or_else(|| panic!("no tower {name}"));
        assert_eq!(item.kind, ItemKind::Type, "{name}");
    }
}

/// A method is a tower named after its class
#[test]
fn a_method_is_a_tower_named_after_its_class() {
    let (tree, harness) = web(&[]);

    let map = map(&harness);
    let cart_file = tree.root.join("src/cart.ts");
    let items = map.items_of(&cart_file).expect("a block for cart.ts");
    let class = items
        .iter()
        .find(|item| item.name == "Cart")
        .expect("a tower for Cart");
    let method_path = cart_file.join("Cart::add");
    let method = items
        .iter()
        .find(|item| item.path == method_path)
        .expect("a tower for Cart::add");
    assert_eq!(method.declaration.part_of.as_ref(), Some(&class.path));
    assert!(!tower(&harness, &method.path).floors.is_empty(), "drawn");
}

/// Tests stand in the cellar
#[test]
fn tests_stand_in_the_cellar() {
    let (tree, harness) = web(&[(
        "src/cart.test.ts",
        "import { Cart } from './cart';\n\n\
         test('adds an item', () => {\n  const cart = new Cart();\n});\n",
    )]);

    let map = map(&harness);
    let test_file = tree.root.join("src/cart.test.ts");
    let items = map.items_of(&test_file).expect("a block for cart.test.ts");
    let test = items
        .iter()
        .find(|item| item.name == "adds an item")
        .expect("a tower for the test");
    assert!(tower(&harness, &test.path).cellar);
}

/// Each test of a suite is its own tower, the suite is none
#[test]
fn each_test_of_a_suite_is_its_own_tower_the_suite_is_none() {
    let (tree, harness) = web(&[(
        "src/cart.test.ts",
        "describe('Cart', () => {\n  it('adds an item', () => {});\n  it('empties', () => {});\n});\n",
    )]);

    let map = map(&harness);
    let test_file = tree.root.join("src/cart.test.ts");
    let items = map.items_of(&test_file).expect("a block for cart.test.ts");
    let names: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
    assert!(!names.contains(&"Cart"), "{names:?}");
    for name in ["adds an item", "empties"] {
        let item = items
            .iter()
            .find(|item| item.name == name)
            .unwrap_or_else(|| panic!("no tower {name}"));
        assert!(tower(&harness, &item.path).cellar, "{name}");
    }
}

/// Every file of the family is read. Each fixture holds the function "load"
/// written in the syntax only its own grammar reads: ".ts" an angle-bracket
/// type assertion, which the TSX grammar takes for markup, and ".tsx", ".js"
/// and ".mjs" markup, which the TypeScript grammar does not read. A file read
/// with the wrong grammar has a syntax error, so it yields no items and falls
/// back to one plain tower named after the file — and the tower "load" is
/// gone.
const FAMILY: [(&str, &str); 4] = [
    (
        "src/load.ts",
        "export function load(value: unknown): number {\n  return <number>value;\n}\n",
    ),
    (
        "src/load.tsx",
        "export function load() {\n  return <div />;\n}\n",
    ),
    (
        "src/load.js",
        "export function load() {\n  return <div />;\n}\n",
    ),
    (
        "src/load.mjs",
        "export function load() {\n  return <div />;\n}\n",
    ),
];

/// Every file of the family is read
#[test]
fn every_file_of_the_family_is_read() {
    let tree = Tree::sources("web", &FAMILY);
    let harness = open(&tree, None, Recorded::listing(&[]));

    let map = map(&harness);
    for (file, _) in FAMILY {
        let path = tree.root.join(file);
        let items = map
            .items_of(&path)
            .unwrap_or_else(|| panic!("a block for {file}"));
        assert!(
            items.iter().any(|item| item.name == "load"),
            "a tower for load on {file}, read with its own grammar"
        );
    }
}

/// A file that does not parse stands as one tower of its lines
#[test]
fn a_file_that_does_not_parse_stands_as_one_tower_of_its_lines() {
    let (tree, harness) = web(&[(
        "src/broken.ts",
        "function broken(\n  return 1\n  // still broken\n",
    )]);

    let map = map(&harness);
    let broken = tree.root.join("src/broken.ts");
    let items = map.items_of(&broken).expect("a block for broken.ts");
    assert_eq!(items.len(), 1, "one tower");
    assert_eq!(items[0].name, "broken.ts");
    assert_eq!(items[0].kind, ItemKind::Text);
    assert_eq!(items[0].floors.len(), 3, "a floor per line");
    assert_eq!(
        tower(&harness, &items[0].path).floors.len(),
        3,
        "it is drawn"
    );
}
