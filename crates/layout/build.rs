//! Compile-time data: parses and validates the shipped data files and writes
//! static tables to `$OUT_DIR`:
//!
//! - `habitats.rs` from `data/habitats.toml` (included by `src/habitat.rs`)
//! - `fish.rs` from `data/fish.toml` (`src/fish.rs`)
//! - `sea_flora.rs` from `data/sea_flora.toml` (`src/sea_flora.rs`)
//! - `validation.rs` from `data/validation.toml` (`src/validation.rs`)
//! - `route.rs` from `data/route.toml` (`src/route.rs`)
//! - `scoring.rs` from `data/scoring.toml` (`src/scoring.rs`)
//! - `structure.rs` from `data/structures/raft_start.toml` and every
//!   `data/structures/villages/*.toml` (`src/structure.rs`)
//!
//! A bad file fails the build. The shipped data reaches the crate as static
//! tables, so the wasm links no TOML parser (the `toml-io` feature is off).

use std::fmt::Write as _;
use std::path::PathBuf;

use serde::Deserialize;

#[path = "src/fish_rules.rs"]
mod fish_rules;
#[path = "src/habitat_rule.rs"]
mod habitat_rule;
#[path = "src/habitat_rules.rs"]
mod habitat_rules;
#[path = "src/rule_check.rs"]
mod rule_check;
#[path = "src/sea_flora_rules.rs"]
mod sea_flora_rules;
#[path = "src/structure_dir.rs"]
mod structure_dir;
#[path = "src/structure_error.rs"]
mod structure_error;
#[path = "src/structure_piece.rs"]
mod structure_piece;
#[path = "src/structure_rules.rs"]
mod structure_rules;

/// Mirrors `Thresholds` in `src/validation.rs`. The shipped file must set
/// every field; only `Thresholds::from_toml` falls back to a default
/// `max_attempts`.
#[derive(Deserialize)]
struct Thresholds {
    min_island_columns: u32,
    window_margin: i32,
    min_raft_water_depth: u32,
    raft_clearance: i32,
    min_flat_grass_tiles: u32,
    min_clear_flat_tiles: u32,
    min_trees: u32,
    min_navigable_depth: u32,
    navigable_clearance: i32,
    raft_half_width: u32,
    max_attempts: u32,
}

/// Mirrors `RouteParams` in `src/route.rs`.
#[derive(Deserialize)]
struct RouteParams {
    spacing: f64,
    spacing_spread: f64,
    max_turn_degrees: f64,
    channel_half_width: f64,
    channel_falloff: f64,
    channel_depth_below_sea: f64,
    channel_clear_above: i32,
    corridor_margin: u32,
    temp_start: f64,
    temp_end: f64,
    d_humid_start: f64,
    d_humid_end: f64,
    climate_half_width: f64,
    climate_falloff: f64,
    cold_below: f64,
    warm_above: f64,
}

/// Mirrors `ScoringConfig` in `src/scoring.rs`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoringConfig {
    weight_spacing_regularity: f64,
    weight_path_directness: f64,
    weight_island_quality: f64,
    weight_coastline_interest: f64,
    weight_extra_islands: f64,
    weight_climate_gradient: f64,
    weight_biome_variety: f64,
    cap_area_factor: f64,
    cap_flat_factor: f64,
    cap_ratio: f64,
    cap_extra_islands: f64,
    eye_height: f64,
    cap_temp_change: f64,
    cap_biomes: f64,
}

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
    for f in [
        "src/habitat_rule.rs",
        "src/habitat_rules.rs",
        "src/fish_rules.rs",
        "src/sea_flora_rules.rs",
        "src/structure_dir.rs",
        "src/structure_error.rs",
        "src/structure_piece.rs",
        "src/structure_rules.rs",
    ] {
        println!("cargo:rerun-if-changed={f}");
    }
    let fish_path = root.join("fish.toml");
    let sea_flora_path = root.join("sea_flora.toml");
    let validation_path = root.join("validation.toml");
    let route_path = root.join("route.toml");
    let scoring_path = root.join("scoring.toml");
    let raft_path = root.join("structures/raft_start.toml");
    // Watching `structures/` also catches `villages/` appearing.
    println!(
        "cargo:rerun-if-changed={}",
        root.join("structures").display()
    );
    let villages_dir = root.join("structures/villages");
    for p in [
        &fish_path,
        &sea_flora_path,
        &validation_path,
        &route_path,
        &scoring_path,
        &raft_path,
    ] {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

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
    std::fs::write(out_dir.join("habitats.rs"), out).unwrap();

    // Fish: habitat names resolve to positions in the rule list above.
    let species =
        fish_rules::parse_species(&read(&fish_path), &|n| rules.iter().position(|r| r.id == n))
            .unwrap_or_else(|e| panic!("{}: {e}", fish_path.display()));
    let mut out = String::from("pub(crate) static SHIPPED_SPECIES: &[StaticSpecies] = &[\n");
    for s in &species {
        let habitats = match &s.habitats {
            Some(ids) => format!("Some(&{ids:?})"),
            None => "None".into(),
        };
        writeln!(
            out,
            "    StaticSpecies {{ id: {:?}, weight: {}, habitats: {}, temp_min: {}, \
             temp_max: {}, depth_min: {}, depth_max: {} }},",
            s.id,
            s.weight,
            habitats,
            opt(&s.temp_min),
            opt(&s.temp_max),
            opt(&s.depth_min),
            opt(&s.depth_max)
        )
        .unwrap();
    }
    out.push_str("];\n");
    std::fs::write(out_dir.join("fish.rs"), out).unwrap();

    // Sea flora: block names resolve like structures', habitats like fish's.
    let flora = sea_flora_rules::parse_sea_flora(&read(&sea_flora_path), &block_id, &|n| {
        rules.iter().position(|r| r.id == n)
    })
    .unwrap_or_else(|e| panic!("{}: {e}", sea_flora_path.display()));
    let mut out = String::from("pub(crate) static SHIPPED_SEA_FLORA: &[StaticSeaFlora] = &[\n");
    for f in &flora {
        writeln!(
            out,
            "    StaticSeaFlora {{ name: {:?}, column: {:?}, blocks: &{:?}, habitats: &{:?}, \
             bottom: &{:?}, depth_min: {}, depth_max: {}, shore_distance_max: {}, \
             cluster_scale: {:?}, salt: {}, chance: {:?} }},",
            f.name,
            f.column
                .map(|c| (c.height_min, c.height_max, c.surface_gap_min)),
            f.blocks,
            f.habitats,
            f.bottom,
            f.depth_min,
            f.depth_max,
            opt(&f.shore_distance_max),
            f.cluster_scale,
            f.salt,
            f.chance
        )
        .unwrap();
    }
    out.push_str("];\n");
    std::fs::write(out_dir.join("sea_flora.rs"), out).unwrap();

    // Validation thresholds.
    let t: Thresholds = toml::from_str(&read(&validation_path))
        .unwrap_or_else(|e| panic!("{}: {e}", validation_path.display()));
    let out = format!(
        "static SHIPPED_THRESHOLDS: Thresholds = Thresholds {{ min_island_columns: {}, \
         window_margin: {}, min_raft_water_depth: {}, raft_clearance: {}, \
         min_flat_grass_tiles: {}, min_clear_flat_tiles: {}, min_trees: {}, min_navigable_depth: {}, navigable_clearance: {}, raft_half_width: {}, max_attempts: {} }};\n",
        t.min_island_columns,
        t.window_margin,
        t.min_raft_water_depth,
        t.raft_clearance,
        t.min_flat_grass_tiles,
        t.min_clear_flat_tiles,
        t.min_trees,
        t.min_navigable_depth,
        t.navigable_clearance,
        t.raft_half_width,
        t.max_attempts
    );
    std::fs::write(out_dir.join("validation.rs"), out).unwrap();

    // Route defaults.
    let r: RouteParams = toml::from_str(&read(&route_path))
        .unwrap_or_else(|e| panic!("{}: {e}", route_path.display()));
    let out = format!(
        "static SHIPPED_ROUTE: RouteParams = RouteParams {{ spacing: {:?}, \
         spacing_spread: {:?}, max_turn_degrees: {:?}, channel_half_width: {:?}, \
         channel_falloff: {:?}, channel_depth_below_sea: {:?}, channel_clear_above: {:?}, \
         corridor_margin: {:?}, temp_start: {:?}, temp_end: {:?}, \
         d_humid_start: {:?}, d_humid_end: {:?}, climate_half_width: {:?}, \
         climate_falloff: {:?}, cold_below: {:?}, warm_above: {:?} }};\n",
        r.spacing,
        r.spacing_spread,
        r.max_turn_degrees,
        r.channel_half_width,
        r.channel_falloff,
        r.channel_depth_below_sea,
        r.channel_clear_above,
        r.corridor_margin,
        r.temp_start,
        r.temp_end,
        r.d_humid_start,
        r.d_humid_end,
        r.climate_half_width,
        r.climate_falloff,
        r.cold_below,
        r.warm_above
    );
    std::fs::write(out_dir.join("route.rs"), out).unwrap();

    // Scoring weights and caps.
    let c: ScoringConfig = toml::from_str(&read(&scoring_path))
        .unwrap_or_else(|e| panic!("{}: {e}", scoring_path.display()));
    let out = format!(
        "static SHIPPED_SCORING: ScoringConfig = ScoringConfig {{ \
         weight_spacing_regularity: {:?}, weight_path_directness: {:?}, \
         weight_island_quality: {:?}, weight_coastline_interest: {:?}, \
         weight_extra_islands: {:?}, weight_climate_gradient: {:?}, \
         weight_biome_variety: {:?}, cap_area_factor: {:?}, cap_flat_factor: {:?}, \
         cap_ratio: {:?}, cap_extra_islands: {:?}, eye_height: {:?}, cap_temp_change: {:?}, cap_biomes: {:?} }};\n",
        c.weight_spacing_regularity,
        c.weight_path_directness,
        c.weight_island_quality,
        c.weight_coastline_interest,
        c.weight_extra_islands,
        c.weight_climate_gradient,
        c.weight_biome_variety,
        c.cap_area_factor,
        c.cap_flat_factor,
        c.cap_ratio,
        c.cap_extra_islands,
        c.eye_height,
        c.cap_temp_change,
        c.cap_biomes
    );
    std::fs::write(out_dir.join("scoring.rs"), out).unwrap();

    // Built-in structure templates.
    let parse = |p: &PathBuf| {
        structure_rules::parse_structure(&read(p), &|n| block_id(n))
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    };
    let mut out = format!(
        "pub(crate) static RAFT_START: StaticStructure = {};\n",
        static_structure(&parse(&raft_path))
    );
    out.push_str("pub(crate) static VILLAGE_TEMPLATES: &[(&str, StaticStructure)] = &[\n");
    let files = structure_dir::template_files(&villages_dir)
        .unwrap_or_else(|e| panic!("{}: {e}", villages_dir.display()));
    for (name, path) in &files {
        if name == "raft_start" {
            panic!("{}: the name raft_start is taken", path.display());
        }
        writeln!(out, "    ({name:?}, {}),", static_structure(&parse(path))).unwrap();
    }
    out.push_str("];\n");
    std::fs::write(out_dir.join("structure.rs"), out).unwrap();
}

/// A `StaticStructure` literal for `p`.
fn static_structure(p: &structure_rules::ParsedStructure) -> String {
    let kind = match p.kind {
        Some(k) => format!("Some(crate::PieceKind::{k:?})"),
        None => "None".into(),
    };
    let connectors: Vec<String> = p
        .connectors
        .iter()
        .map(|c| {
            format!(
                "Connector {{ cell: {:?}, facing: crate::Facing::{:?}, pool: crate::Pool::{:?} }}",
                c.cell, c.facing, c.pool
            )
        })
        .collect();
    format!(
        "StaticStructure {{ size: {:?}, origin_offset: {:?}, raft: {}, blocks: &{:?}, \
         kind: {kind}, connectors: &[{}], spawns: &{:?}, rotations: &{:?} }}",
        p.size,
        p.origin_offset,
        p.raft,
        p.blocks,
        connectors.join(", "),
        p.spawns,
        p.rotations
    )
}
