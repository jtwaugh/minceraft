//! Habitat map for a region (design: world-layout Phase C, 3 step 7, 4.5).

use crate::{DerivedMaps, HabitatInputs, Habitats, HumidityTerms, Layout, TemperatureTerms};
use terrain::{generate_climate, HumidityBias, TemperatureBias};

/// `habitat` value for land columns (no habitat). Real habitats are rule
/// indices, so this never collides with one.
pub const NO_HABITAT: u8 = u8::MAX;

/// Derived maps, biased temperature and habitat for a `w x d` area. Every
/// vector has `width * depth` entries in row order `z * width + x`.
#[derive(Clone, Debug, PartialEq)]
pub struct HabitatMap {
    pub x0: i32,
    pub z0: i32,
    pub width: u32,
    pub depth: u32,
    /// Water depth, bottom block, frozen flag and shore distance.
    pub derived: DerivedMaps,
    /// Biased temperature per column (the layout's climate terms applied).
    pub temperature: Vec<f32>,
    /// [`HabitatId`](crate::HabitatId) index per water column;
    /// [`NO_HABITAT`] on land.
    pub habitat: Vec<u8>,
}

impl Layout {
    /// Habitat map for the `w x d` area at `(x0, z0)` using the shipped
    /// rules. Terrain is generated undecorated, so trees never change the
    /// maps; each value depends only on the seed, the layout and the world
    /// coordinate.
    pub fn habitat_map(&self, x0: i32, z0: i32, w: u32, d: u32) -> HabitatMap {
        self.habitat_map_with(&Habitats::shipped(), x0, z0, w, d)
    }

    /// Like [`Layout::habitat_map`] with explicit rules.
    pub fn habitat_map_with(
        &self,
        habitats: &Habitats,
        x0: i32,
        z0: i32,
        w: u32,
        d: u32,
    ) -> HabitatMap {
        let derived = DerivedMaps::generate(self, x0, z0, w, d, false);
        let temp = TemperatureTerms(&self.climate);
        let humid = HumidityTerms(&self.climate);
        let temperature: [&dyn TemperatureBias; 1] = [&temp];
        let humidity: [&dyn HumidityBias; 1] = [&humid];
        let climate = generate_climate(self.seed, x0, z0, w, d, &temperature, &humidity);
        let habitat = (0..w as usize * d as usize)
            .map(|i| {
                if derived.water_depth[i] == 0 {
                    return NO_HABITAT;
                }
                let id = habitats.classify(&HabitatInputs {
                    temperature: f64::from(climate.temperature[i]),
                    depth: u32::from(derived.water_depth[i]),
                    bottom: derived.bottom_block[i],
                    shore_distance: u32::from(derived.shore_distance[i]),
                    frozen: derived.frozen[i],
                });
                u8::try_from(id.0).expect("fewer than 255 habitat rules")
            })
            .collect();
        HabitatMap {
            x0,
            z0,
            width: w,
            depth: d,
            derived,
            temperature: climate.temperature,
            habitat,
        }
    }
}
