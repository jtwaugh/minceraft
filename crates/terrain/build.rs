//! Compile-time data: parses `data/blocks.toml` and `data/biomes.toml` and
//! writes `$OUT_DIR/data.rs` (included by `src/data.rs`).
//!
//! The `toml` crate is only a build-dependency; nothing here reaches the
//! runtime crate or the wasm output.

mod flora_check;

use std::fmt::Write as _;
use std::path::PathBuf;
use toml::Value;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../data");
    let blocks_path = root.join("blocks.toml");
    let biomes_path = root.join("biomes.toml");
    println!("cargo:rerun-if-changed={}", blocks_path.display());
    println!("cargo:rerun-if-changed={}", biomes_path.display());
    let flora_path = root.join("flora.toml");
    println!("cargo:rerun-if-changed={}", flora_path.display());
    println!("cargo:rerun-if-changed=flora_check.rs");
    println!("cargo:rerun-if-changed=build.rs");

    let blocks: Value = read(&blocks_path);
    let biomes: Value = read(&biomes_path);

    let mut out = String::new();
    let mut json = String::from("{\"blocks\":[");
    let block_names = gen_blocks(&blocks, &mut out, &mut json);
    json.push_str("],\"biomes\":[");
    gen_biomes(&biomes, &block_names, &mut out, &mut json);
    let radius = int(&biomes["blend"], "radius");
    assert!((0..=7).contains(&radius), "[blend] radius must be 0 to 7");
    json.push_str(
        "],\"ramps\":{\"grass\":\"humidity\",\"leaf\":\"humidity\",\"water\":\"humidity\",\"dry\":\"humidity\",\"sky\":\"temperature\"}",
    );
    write!(json, ",\"blend\":{{\"radius\":{radius}}}}}").unwrap();
    gen_lookup(&biomes, &mut out);
    gen_trees(&biomes, &block_names, &mut out);
    gen_flora(&flora_path, &block_names, &biome_names(&biomes));
    writeln!(
        out,
        "/// Legend for viewers: blocks and biomes with colours and ramps (JSON)."
    )
    .unwrap();
    writeln!(out, "pub const LEGEND_JSON: &str = {:?};", json).unwrap();

    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("data.rs");
    std::fs::write(dest, out).unwrap();
}

fn biome_names(doc: &Value) -> Vec<String> {
    let list = doc["biome"].as_array().expect("[[biome]]");
    list.iter()
        .map(|b| b["name"].as_str().unwrap().to_string())
        .collect()
}

/// Validates `flora.toml` and writes `$OUT_DIR/flora.rs`.
fn gen_flora(path: &PathBuf, blocks: &[String], biomes: &[String]) {
    let doc = read(path);
    let strings = |v: &Value| -> Vec<String> {
        match v {
            Value::String(s) => vec![s.clone()],
            Value::Array(a) => a
                .iter()
                .map(|e| e.as_str().expect("name string").to_string())
                .collect(),
            _ => panic!("flora.toml: expected a name or a list of names"),
        }
    };
    let list = doc["flora"].as_array().expect("[[flora]]");
    let raw: Vec<flora_check::RawFlora> = list
        .iter()
        .map(|f| flora_check::RawFlora {
            name: f["name"].as_str().expect("flora name").to_string(),
            block: strings(&f["block"]),
            ground: strings(&f["ground"]),
            shape: f["shape"].as_str().expect("flora shape").to_string(),
            tint: f["tint"].as_str().expect("flora tint").to_string(),
            salt: int(f, "salt"),
            biomes: f["biomes"]
                .as_table()
                .expect("biomes table")
                .iter()
                .map(|(k, v)| (k.clone(), num(v)))
                .collect(),
        })
        .collect();
    if let Err(e) = flora_check::validate(&raw, blocks, biomes) {
        panic!("data/flora.toml: {e}");
    }

    let blk = |n: &String| format!("Block::{}", camel(n));
    let mut out = String::new();
    for k in ["patch_noise_salt", "colour_noise_salt"] {
        let v = int(&doc["settings"], k);
        writeln!(out, "pub const FLORA_{}: u32 = {v};", k.to_uppercase()).unwrap();
    }
    let scale = num(&doc["settings"]["colour_scale"]);
    writeln!(out, "pub const FLORA_COLOUR_SCALE: f64 = {scale:?};").unwrap();
    writeln!(out, "pub static FLORA: [Flora; {}] = [", raw.len()).unwrap();
    for (f, r) in list.iter().zip(&raw) {
        let ground: Vec<String> = r.ground.iter().map(blk).collect();
        let chances: Vec<String> = r
            .biomes
            .iter()
            .map(|(b, c)| format!("(Biome::{}, {c:?})", camel(b)))
            .collect();
        let upper = r
            .block
            .get(1)
            .map_or("None".into(), |u| format!("Some({})", blk(u)));
        writeln!(
            out,
            "    Flora {{ name: {:?}, block: {}, upper: {upper}, shape: FloraShape::{}, ground: &[{}], tint: FloraTint::{}, cluster_scale: {:?}, salt: {}, biomes: &[{}] }},",
            r.name,
            blk(&r.block[0]),
            camel(&r.shape),
            ground.join(", "),
            camel(&r.tint),
            num(&f["cluster_scale"]),
            r.salt,
            chances.join(", ")
        )
        .unwrap();
    }
    writeln!(out, "];").unwrap();
    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("flora.rs");
    std::fs::write(dest, out).unwrap();
}

fn read(p: &PathBuf) -> Value {
    let s = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    s.parse().unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn camel(name: &str) -> String {
    name.split(['_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            let f = c.next().unwrap().to_ascii_uppercase();
            core::iter::once(f).chain(c).collect::<String>()
        })
        .collect()
}

fn color(v: &Value) -> [u8; 3] {
    let s = v.as_str().expect("colour string");
    let h = s.trim_start_matches('#');
    assert!(h.len() == 6, "bad colour {s}");
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).expect("bad colour");
    [p(0), p(2), p(4)]
}

fn int(v: &Value, key: &str) -> i64 {
    v.get(key)
        .and_then(Value::as_integer)
        .unwrap_or_else(|| panic!("missing integer {key}"))
}

fn num(v: &Value) -> f64 {
    v.as_float()
        .or_else(|| v.as_integer().map(|i| i as f64))
        .expect("number")
}

fn pair(v: &Value, key: &str) -> (i64, i64) {
    let a = v
        .get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing {key}"));
    (a[0].as_integer().unwrap(), a[1].as_integer().unwrap())
}

fn gen_blocks(doc: &Value, out: &mut String, json: &mut String) -> Vec<String> {
    let list = doc["block"].as_array().expect("[[block]]");
    let mut names = Vec::new();
    let mut infos = String::new();
    for (i, b) in list.iter().enumerate() {
        assert_eq!(
            int(b, "id") as usize,
            i,
            "block ids must be dense and in order"
        );
        let name = b["name"].as_str().unwrap().to_string();
        let fire = b["fire"].as_array().unwrap();
        let collision = b["collision"].as_str().unwrap();
        let col = match collision {
            "none" => "None",
            "full" => "Full",
            "inset" => "Inset",
            c => panic!("bad collision {c}"),
        };
        let opacity = int(b, "opacity");
        let opaque = b["opaque_cube"].as_bool().unwrap();
        let liquid = b.get("liquid").and_then(Value::as_bool).unwrap_or(false);
        let c = color(&b["color"]);
        writeln!(
            infos,
            "    BlockInfo {{ name: {name:?}, fire_encouragement: {}, flammability: {}, collision: Collision::{col}, opacity: {opacity}, opaque_cube: {opaque}, liquid: {liquid}, color: {c:?} }},",
            fire[0].as_integer().unwrap(),
            fire[1].as_integer().unwrap()
        )
        .unwrap();
        if i > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"id\":{i},\"name\":{name:?},\"color\":{c:?},\"opacity\":{opacity},\"collision\":{collision:?},\"fire\":[{},{}]}}",
            fire[0].as_integer().unwrap(),
            fire[1].as_integer().unwrap()
        )
        .unwrap();
        names.push(name);
    }
    let n = names.len();
    writeln!(out, "/// Block palette, generated from `data/blocks.toml`.").unwrap();
    writeln!(
        out,
        "#[repr(u8)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum Block {{"
    )
    .unwrap();
    for (i, nm) in names.iter().enumerate() {
        writeln!(out, "    {} = {i},", camel(nm)).unwrap();
    }
    writeln!(
        out,
        "}}\n\nimpl Block {{\n    pub const COUNT: usize = {n};"
    )
    .unwrap();
    write!(out, "    pub const ALL: [Block; {n}] = [").unwrap();
    for nm in &names {
        write!(out, "Block::{}, ", camel(nm)).unwrap();
    }
    writeln!(out, "];\n}}\n").unwrap();
    writeln!(out, "/// Per-block properties indexed by block id.").unwrap();
    writeln!(
        out,
        "pub static BLOCK_INFO: [BlockInfo; {n}] = [\n{infos}];\n"
    )
    .unwrap();
    names
}

fn block_id(names: &[String], v: &Value) -> usize {
    let s = v.as_str().expect("block name");
    names
        .iter()
        .position(|n| n == s)
        .unwrap_or_else(|| panic!("unknown block {s}"))
}

fn species(v: Option<&Value>) -> String {
    let mut s = String::from("&[");
    if let Some(a) = v.and_then(Value::as_array) {
        for e in a {
            let e = e.as_array().unwrap();
            write!(
                s,
                "({:?}, {}), ",
                e[0].as_str().unwrap(),
                e[1].as_integer().unwrap()
            )
            .unwrap();
        }
    }
    s.push(']');
    s
}

const TREE_KINDS: [&str; 5] = ["oak", "birch", "taiga_a", "taiga_b", "big_oak"];
const PLANT_KEYS: [&str; 10] = [
    "dandelion",
    "rose",
    "tall_grass",
    "fern_share",
    "dead_bush",
    "cactus",
    "sugar_cane",
    "pumpkin",
    "brown_mushroom",
    "red_mushroom",
];

fn gen_biomes(doc: &Value, blocks: &[String], out: &mut String, json: &mut String) {
    let list = doc["biome"].as_array().expect("[[biome]]");
    let spawns = &doc["spawns"];
    let mut names = Vec::new();
    let mut infos = String::new();
    for (i, b) in list.iter().enumerate() {
        assert_eq!(
            int(b, "id") as usize,
            i,
            "biome ids must be dense and in order"
        );
        let name = b["name"].as_str().unwrap().to_string();
        let top = block_id(blocks, &b["top"]);
        let filler = block_id(blocks, &b["filler"]);
        let map = color(&b["map_color"]);
        let trees = b["trees"].as_table().unwrap();
        let mut w = [0i64; 5];
        for (k, v) in trees {
            let idx = TREE_KINDS
                .iter()
                .position(|t| t == k)
                .unwrap_or_else(|| panic!("tree {k}"));
            w[idx] = v.as_integer().unwrap();
        }
        let plants = b["plants"].as_table().unwrap();
        for k in plants.keys() {
            assert!(PLANT_KEYS.contains(&k.as_str()), "unknown plant key {k}");
        }
        let mut p = String::from("Plants { ");
        for k in PLANT_KEYS {
            let v = plants.get(k).map(num).unwrap_or(0.0);
            write!(p, "{k}: {v:?}, ").unwrap();
        }
        p.push('}');
        let ramp = |key: &str| -> [[u8; 3]; 3] {
            let a = b
                .get(key)
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("biome {name} needs {key}"));
            assert_eq!(a.len(), 3, "{key} needs 3 stops");
            [color(&a[0]), color(&a[1]), color(&a[2])]
        };
        let (g, l, s, wr, dr) = (
            ramp("grass_ramp"),
            ramp("leaf_ramp"),
            ramp("sky_ramp"),
            ramp("water_ramp"),
            ramp("dry_ramp"),
        );
        let mut creatures = spawns["creatures"].as_array().unwrap().clone();
        if let Some(extra) = b.get("extra_creatures").and_then(Value::as_array) {
            creatures.extend(extra.iter().cloned());
        }
        writeln!(
            infos,
            "    BiomeInfo {{ name: {name:?}, map_color: {map:?}, top: {top}, filler: {filler}, tree_noise: {}, tree_bonus: {}, tree_weights: {w:?}, plants: {p}, grass_ramp: {g:?}, leaf_ramp: {l:?}, sky_ramp: {s:?}, water_ramp: {wr:?}, dry_ramp: {dr:?}, creatures: {}, monsters: {}, water_creatures: {} }},",
            b["tree_noise"].as_bool().unwrap(),
            int(b, "tree_bonus"),
            species(Some(&Value::Array(creatures))),
            species(spawns.get("monsters")),
            species(spawns.get("water")),
        )
        .unwrap();
        if i > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"id\":{i},\"name\":{name:?},\"map\":{map:?},\"grass\":{},\"leaf\":{},\"sky\":{},\"water\":{},\"dry\":{}}}",
            fmt_ramp(&g),
            fmt_ramp(&l),
            fmt_ramp(&s),
            fmt_ramp(&wr),
            fmt_ramp(&dr)
        )
        .unwrap();
        names.push(name);
    }
    let n = names.len();
    writeln!(out, "/// Biomes, generated from `data/biomes.toml`.").unwrap();
    writeln!(
        out,
        "#[repr(u8)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum Biome {{"
    )
    .unwrap();
    for (i, nm) in names.iter().enumerate() {
        writeln!(out, "    {} = {i},", camel(nm)).unwrap();
    }
    writeln!(
        out,
        "}}\n\nimpl Biome {{\n    pub const COUNT: usize = {n};"
    )
    .unwrap();
    write!(out, "    pub const ALL: [Biome; {n}] = [").unwrap();
    for nm in &names {
        write!(out, "Biome::{}, ", camel(nm)).unwrap();
    }
    writeln!(out, "];\n}}\n").unwrap();
    writeln!(out, "/// Per-biome data indexed by biome id.").unwrap();
    writeln!(
        out,
        "pub static BIOME_INFO: [BiomeInfo; {n}] = [\n{infos}];\n"
    )
    .unwrap();
    writeln!(
        out,
        "/// Spawn caps (creatures, monsters, water creatures).\npub const SPAWN_CAPS: [u32; 3] = [{}, {}, {}];\n",
        int(spawns, "creature_cap"),
        int(spawns, "monster_cap"),
        int(spawns, "water_cap")
    )
    .unwrap();
}

fn fmt_ramp(r: &[[u8; 3]; 3]) -> String {
    format!("[{:?},{:?},{:?}]", r[0], r[1], r[2])
}

fn gen_lookup(doc: &Value, out: &mut String) {
    let t = doc["lookup"].as_table().unwrap();
    writeln!(
        out,
        "/// Biome lookup thresholds (SPEC 8.2), from `[lookup]` in `data/biomes.toml`."
    )
    .unwrap();
    writeln!(out, "pub mod lookup {{").unwrap();
    for k in [
        "cold_t",
        "cool_t",
        "swamp_t",
        "hot_t",
        "very_hot_t",
        "dry_w",
        "shrub_w",
        "plains_w",
        "swamp_w",
        "seasonal_w",
    ] {
        let v = num(t.get(k).unwrap_or_else(|| panic!("lookup.{k} missing")));
        writeln!(
            out,
            "    pub const {}: f64 = {v:?};",
            k.to_ascii_uppercase()
        )
        .unwrap();
    }
    writeln!(out, "}}\n").unwrap();
}

fn gen_trees(doc: &Value, blocks: &[String], out: &mut String) {
    let t = &doc["trees"];
    let lw = |v: &Value| (block_id(blocks, &v["log"]), block_id(blocks, &v["leaves"]));
    writeln!(
        out,
        "/// Tree shape parameters (SPEC 11), from `[trees.*]` in `data/biomes.toml`."
    )
    .unwrap();
    writeln!(out, "pub mod tree_params {{\n    use super::*;").unwrap();
    for (name, key) in [("OAK", "oak"), ("BIRCH", "birch")] {
        let v = &t[key];
        let (log, leaves) = lw(v);
        let canopy: Vec<i64> = v["canopy"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_integer().unwrap())
            .collect();
        writeln!(
            out,
            "    pub const {name}: SmallTree = SmallTree {{ log: {log}, leaves: {leaves}, height: {:?}, canopy: &{canopy:?} }};",
            pair(v, "height")
        )
        .unwrap();
    }
    let v = &t["taiga_a"];
    let (log, leaves) = lw(v);
    writeln!(
        out,
        "    pub const TAIGA_A: TaigaA = TaigaA {{ log: {log}, leaves: {leaves}, height: {:?}, bare_trunk: {:?} }};",
        pair(v, "height"),
        pair(v, "bare_trunk")
    )
    .unwrap();
    let v = &t["taiga_b"];
    let (log, leaves) = lw(v);
    writeln!(
        out,
        "    pub const TAIGA_B: TaigaB = TaigaB {{ log: {log}, leaves: {leaves}, height: {:?}, bare_trunk: {:?}, max_radius: {:?} }};",
        pair(v, "height"),
        pair(v, "bare_trunk"),
        pair(v, "max_radius")
    )
    .unwrap();
    let v = &t["big_oak"];
    let (log, leaves) = lw(v);
    let radii: Vec<f64> = v["foliage_radii"]
        .as_array()
        .unwrap()
        .iter()
        .map(num)
        .collect();
    writeln!(
        out,
        "    pub const BIG_OAK: BigOak = BigOak {{ log: {log}, leaves: {leaves}, height: {:?}, min_height: {}, trunk_ratio: {:?}, branch_slope: {:?}, foliage_density: {:?}, branch_cutoff: {:?}, level_cutoff: {:?}, foliage_radii: &{radii:?} }};",
        pair(v, "height"),
        int(v, "min_height"),
        num(&v["trunk_ratio"]),
        num(&v["branch_slope"]),
        num(&v["foliage_density"]),
        num(&v["branch_cutoff"]),
        num(&v["level_cutoff"]),
    )
    .unwrap();
    writeln!(out, "}}\n").unwrap();
}
