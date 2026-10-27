//! Validation of `data/flora.toml` entries, on plain types so `build.rs` and
//! the unit tests in `src/flora.rs` share it.

/// One `[[flora]]` entry as read from the file, before name resolution.
pub struct RawFlora {
    pub name: String,
    /// One block name, or lower and upper for a two-block plant.
    pub block: Vec<String>,
    pub shape: String,
    pub ground: Vec<String>,
    pub tint: String,
    pub cluster_scale: f64,
    pub salt: i64,
    pub biomes: Vec<(String, f64)>,
}

/// Returns the first problem in `entries` as a readable message.
pub fn validate(entries: &[RawFlora], blocks: &[String], biomes: &[String]) -> Result<(), String> {
    let mut salts: Vec<(i64, &str)> = Vec::new();
    for e in entries {
        let n = &e.name;
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
