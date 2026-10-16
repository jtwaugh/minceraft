//! Compile-time data: parses and validates `data/habitats.toml` and writes
//! `$OUT_DIR/habitats.rs` (included by `src/habitat.rs`). A bad rule file
//! fails the build.
//!
//! The shipped rules reach the crate as a static table, so the wasm does not
//! link a parser for them.

use std::fmt::Write as _;
use std::path::PathBuf;

#[path = "src/habitat_rules.rs"]
mod habitat_rules;

fn opt<T: std::fmt::Debug>(v: &Option<T>) -> String {
    match v {
        Some(x) => format!("Some({x:?})"),
        None => "None".into(),
    }
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../data");
    let habitats_path = root.join("habitats.toml");
    let blocks_path = root.join("blocks.toml");
    println!("cargo:rerun-if-changed={}", habitats_path.display());
    println!("cargo:rerun-if-changed={}", blocks_path.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/habitat_rules.rs");

    let read =
        |p: &PathBuf| std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    // Block ids are dense and in file order (terrain/build.rs checks that).
    let blocks: toml::Value = read(&blocks_path)
        .parse()
        .unwrap_or_else(|e| panic!("{}: {e}", blocks_path.display()));
    let names: Vec<&str> = blocks["block"]
        .as_array()
        .expect("[[block]]")
        .iter()
        .map(|b| b["name"].as_str().expect("block name"))
        .collect();
    let block_id = |n: &str| names.iter().position(|b| *b == n).map(|i| i as u8);

    let rules = habitat_rules::parse_rules(&read(&habitats_path), &block_id)
        .unwrap_or_else(|e| panic!("{}: {e}", habitats_path.display()));

    let mut out = String::from("pub(crate) static SHIPPED_RULES: &[StaticRule] = &[\n");
    for r in &rules {
        let bottom = match &r.bottom {
            Some(ids) => format!("Some(&{ids:?})"),
            None => "None".into(),
        };
        writeln!(
            out,
            "    StaticRule {{ id: {:?}, frozen: {}, temp_min: {}, temp_max: {}, depth_min: {}, \
             depth_max: {}, shore_distance_min: {}, shore_distance_max: {}, bottom: {} }},",
            r.id,
            opt(&r.frozen),
            opt(&r.temp_min),
            opt(&r.temp_max),
            opt(&r.depth_min),
            opt(&r.depth_max),
            opt(&r.shore_distance_min),
            opt(&r.shore_distance_max),
            bottom
        )
        .unwrap();
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("habitats.rs");
    std::fs::write(dest, out).unwrap();
}
