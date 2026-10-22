//! Climate gradient and biome variety terms (docs/design/world-layout.md
//! 6.3, Phase G; formulas and caps decided in #847). Pure functions of the
//! anchorage temperatures and biomes from `route_biome`.
//!
//! Choices made here:
//! - Fewer than two anchorages (no steps) or no biomes score 0.0, so an empty
//!   route never outscores a real one.
//! - A zero cap scores its part 1.0 when the measure is positive, else 0.0.

use terrain::Biome;

use crate::ScoringConfig;

fn capped(value: f64, cap: f64) -> f64 {
    if cap > 0.0 {
        (value / cap).clamp(0.0, 1.0)
    } else if value > 0.0 {
        1.0
    } else {
        0.0
    }
}

/// Mean of `min((T_last - T_first) / cap_temp_change, 1)` clamped to 0..1 and
/// the share of consecutive steps with `T_{i+1} >= T_i`, in 0..1.
pub fn climate_gradient(temps: &[f64], cfg: &ScoringConfig) -> f64 {
    let (Some(&first), Some(&last)) = (temps.first(), temps.last()) else {
        return 0.0;
    };
    if temps.len() < 2 {
        return 0.0;
    }
    let steps = temps.len() - 1;
    let rising = temps.windows(2).filter(|p| p[1] >= p[0]).count();
    let change = capped(last - first, cfg.cap_temp_change);
    (change + rising as f64 / steps as f64) / 2.0
}

/// `min(B / cap_biomes, 1)` with `B` the number of distinct biomes; islands
/// with no biome (`None`) are skipped.
pub fn biome_variety(biomes: &[Option<Biome>], cfg: &ScoringConfig) -> f64 {
    let mut seen = [false; Biome::COUNT];
    for b in biomes.iter().flatten() {
        seen[*b as usize] = true;
    }
    capped(seen.iter().filter(|&&s| s).count() as f64, cfg.cap_biomes)
}
