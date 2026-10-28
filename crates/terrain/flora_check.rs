//! Validation of `data/flora.toml` entries, on plain types so `build.rs` and
//! the unit tests in `src/flora.rs` share it.

/// One `[[flora]]` entry as read from the file, before name resolution.
pub struct RawFlora {
    pub name: String,
    /// One block name, or lower and upper for a two-block plant.
    pub block: Vec<String>,
    pub ground: Vec<String>,
    pub shape: String,
    pub tint: String,
    pub salt: i64,
    pub biomes: Vec<(String, f64)>,
    /// Optional column condition, one of `REQUIRES`.
    pub requires: Option<String>,
}

const SHAPES: [&str; 5] = ["cross", "tall_cross", "ground_cover", "pad", "bush"];
/// Column conditions an entry may name in `requires`.
pub const REQUIRES: [&str; 1] = ["near_water"];
const TINTS: [&str; 4] = ["none", "grass", "leaf", "dry"];

/// Returns the first problem in `entries` as a readable message.
pub fn validate(entries: &[RawFlora], blocks: &[String], biomes: &[String]) -> Result<(), String> {
    let mut salts: Vec<(i64, &str)> = Vec::new();
    for e in entries {
        let n = &e.name;
        let want = 1 + (e.shape == "tall_cross") as usize;
        if !SHAPES.contains(&e.shape.as_str()) || !TINTS.contains(&e.tint.as_str()) {
            return Err(format!("flora {n}: bad shape or tint"));
        } else if e.block.len() != want {
            return Err(format!("flora {n}: {} needs {want} block(s)", e.shape));
        }
        if let Some(r) = &e.requires {
            if !REQUIRES.contains(&r.as_str()) {
                return Err(format!("flora {n}: unknown requires {r:?}"));
            }
        }
        for b in e.block.iter().chain(&e.ground) {
            if !blocks.contains(b) {
                return Err(format!("flora {n}: unknown block {b:?}"));
            }
        }
        for (b, c) in &e.biomes {
            if !biomes.contains(b) {
                return Err(format!("flora {n}: unknown biome {b:?}"));
            }
            if !(0.0..=1.0).contains(c) {
                return Err(format!("flora {n}: chance {c} for {b} is outside 0..=1"));
            }
        }
        if let Some((_, other)) = salts.iter().find(|(s, _)| *s == e.salt) {
            return Err(format!("flora {n}: salt {} repeats {other}'s", e.salt));
        }
        salts.push((e.salt, n));
    }
    Ok(())
}
