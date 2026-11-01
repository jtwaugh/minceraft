//! `Layout` and the `generate_region` wrapper (design: world-layout 4.2).

use crate::{
    ChannelEval, ClimateBias, DensityTerm, Exclusion, Exclusions, HumidityTerms, Structure,
    TemperatureTerms,
};
use terrain::{
    generate_region_with_biases, BaseHeightBias, Biases, Block, DensityClamp, HumidityBias,
    OpenWater, ProtectedMask, Region, TemperatureBias, HEIGHT,
};

/// Blocks added around a region when testing term bounds. Generous on
/// purpose: it covers the lattice reach of terrain and the decoration
/// margin, so skipping a term can never change output.
const BOUNDS_PAD: f64 = 256.0;

/// Everything that steers a world: the seed plus bias terms and exclusions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub seed: u64,
    pub density: Vec<DensityTerm>,
    pub climate: Vec<ClimateBias>,
    pub exclusions: Vec<Exclusion>,
    pub structures: Vec<StructurePlacement>,
    /// Zones the sea flora pass leaves empty (each level's spawn water).
    pub flora_free: Vec<Exclusion>,
}

/// A structure template placed in the world (design: world-layout 4.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructurePlacement {
    /// Template name, e.g. `"raft_start"`.
    pub structure: String,
    pub origin: [i32; 3],
    /// Quarter turns, taken mod 4.
    pub rotation: u8,
}

/// A generated region plus the raft mask (design: world-layout 4.5).
pub struct LayoutRegion {
    pub region: Region,
    /// Non-air cells of `raft = true` structures inside the region, in world
    /// coordinates.
    pub raft_cells: Vec<[i32; 3]>,
}

/// Axis-aligned box `(min_x, min_z, max_x, max_z)` in world blocks.
type Bounds = (f64, f64, f64, f64);

fn circle(c: (f64, f64), r: f64) -> Bounds {
    (c.0 - r, c.1 - r, c.0 + r, c.1 + r)
}

fn polyline(points: &[(f64, f64)], reach: f64) -> Option<Bounds> {
    let first = points.first()?;
    let mut b = (first.0, first.1, first.0, first.1);
    for p in points {
        b = (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1));
    }
    Some((b.0 - reach, b.1 - reach, b.2 + reach, b.3 + reach))
}

impl DensityTerm {
    fn bounds(&self) -> Option<Bounds> {
        match self {
            DensityTerm::RadialBaseHeight {
                center, r_outer, ..
            } => Some(circle(*center, *r_outer)),
            DensityTerm::Channel {
                points,
                half_width,
                falloff,
                ..
            } => polyline(points, half_width + falloff),
        }
    }
}

impl ClimateBias {
    fn bounds(&self) -> Option<Bounds> {
        match self {
            ClimateBias::Radial {
                center, r_outer, ..
            } => Some(circle(*center, *r_outer)),
            ClimateBias::AlongPath {
                points,
                half_width,
                falloff,
                ..
            } => polyline(points, half_width + falloff),
        }
    }
}

impl Exclusion {
    fn bounds(&self) -> Option<Bounds> {
        match self {
            Exclusion::Circle { center, radius } => Some(circle(*center, *radius)),
            Exclusion::Corridor { points, half_width } => polyline(points, *half_width),
        }
    }
}

/// Whether `bounds` meets the padded region. Terms without bounds (empty
/// polylines) affect nothing and are skipped.
fn intersects(bounds: Option<Bounds>, region: Bounds) -> bool {
    bounds.is_some_and(|b| {
        b.0 <= region.2 + BOUNDS_PAD
            && b.2 >= region.0 - BOUNDS_PAD
            && b.1 <= region.3 + BOUNDS_PAD
            && b.3 >= region.1 - BOUNDS_PAD
    })
}

/// Columns inside any channel's reach (weight above 0) stay free of sea ice.
struct ChannelsOpen<'a>(&'a [Option<ChannelEval>]);

impl OpenWater for ChannelsOpen<'_> {
    fn open(&self, x: i32, z: i32) -> bool {
        self.0.iter().flatten().any(|c| c.weight(x, z) > 0.0)
    }
}

impl Layout {
    /// A layout with no terms: generates exactly what `terrain` does.
    pub fn new(seed: u64) -> Layout {
        Layout {
            seed,
            ..Layout::default()
        }
    }

    /// Generate a `w x d` region at world `(x0, z0)`, terrain only or
    /// decorated. Only terms whose bounds meet the region are evaluated.
    pub fn generate_region(&self, x0: i32, z0: i32, w: u32, d: u32, decorate: bool) -> Region {
        let region = (
            x0 as f64,
            z0 as f64,
            x0 as f64 + w as f64,
            z0 as f64 + d as f64,
        );
        self.generate(x0, z0, w, d, decorate, |b| intersects(b, region))
    }

    /// Same as [`Layout::generate_region`] but evaluates every term; used to
    /// check that skipping is invisible.
    pub fn generate_region_all_terms(
        &self,
        x0: i32,
        z0: i32,
        w: u32,
        d: u32,
        decorate: bool,
    ) -> Region {
        self.generate(x0, z0, w, d, decorate, |_| true)
    }

    /// Like [`Layout::generate_region`], also returning the raft mask. With
    /// `toml-io`, the sea flora pass runs on the blocks.
    pub fn generate_layout_region(
        &self,
        x0: i32,
        z0: i32,
        w: u32,
        d: u32,
        decorate: bool,
    ) -> LayoutRegion {
        #[cfg(feature = "toml-io")]
        let region = self.generate_with_sea_flora(x0, z0, w, d, decorate);
        #[cfg(not(feature = "toml-io"))]
        let region = self.generate_region(x0, z0, w, d, decorate);
        LayoutRegion {
            region,
            raft_cells: self.raft_cells(x0, z0, w, d),
        }
    }

    /// Cells `(x, y, z, block)` of every placement that fall inside the
    /// region, with `keep` choosing which cells of each template count.
    ///
    /// # Panics
    /// If a placement names a template that does not exist.
    fn placed_cells(
        &self,
        (x0, z0, w, d): (i32, i32, u32, u32),
        keep: impl Fn(&Structure, u8) -> bool,
    ) -> Vec<(i32, i32, i32, u8)> {
        let (x1, z1) = (x0 as i64 + w as i64, z0 as i64 + d as i64);
        let mut out = Vec::new();
        for p in &self.structures {
            let s = Structure::named(&p.structure)
                .unwrap_or_else(|| panic!("unknown structure {:?}", p.structure));
            for c in s.cells(p.origin, p.rotation) {
                let (x, y, z, b) = c;
                if (x0 as i64..x1).contains(&(x as i64))
                    && (z0 as i64..z1).contains(&(z as i64))
                    && (0..HEIGHT as i32).contains(&y)
                    && keep(&s, b)
                {
                    out.push(c);
                }
            }
        }
        out
    }

    fn raft_cells(&self, x0: i32, z0: i32, w: u32, d: u32) -> Vec<[i32; 3]> {
        self.placed_cells((x0, z0, w, d), |s, b| s.raft && b != Block::Air as u8)
            .into_iter()
            .map(|(x, y, z, _)| [x, y, z])
            .collect()
    }

    /// Overwrites the region with every placement's cells inside it.
    fn stamp(&self, region: &mut Region) {
        let cells = self.placed_cells(
            (region.x0, region.z0, region.width, region.depth),
            |_, _| true,
        );
        for (x, y, z, b) in cells {
            let i = ((z - region.z0) as usize * region.width as usize + (x - region.x0) as usize)
                * HEIGHT
                + y as usize;
            region.blocks[i] = b;
        }
    }

    fn generate(
        &self,
        x0: i32,
        z0: i32,
        w: u32,
        d: u32,
        decorate: bool,
        keep: impl Fn(Option<Bounds>) -> bool,
    ) -> Region {
        let mut region = self.with_biases(keep, |biases| {
            generate_region_with_biases(self.seed, x0, z0, w, d, biases, decorate)
        });
        self.stamp(&mut region);
        region
    }

    /// Compile the terms `keep` selects into `Biases` and pass them to `f`.
    pub(crate) fn with_biases<R>(
        &self,
        keep: impl Fn(Option<Bounds>) -> bool,
        f: impl FnOnce(&Biases) -> R,
    ) -> R {
        let density: Vec<DensityTerm> = self
            .density
            .iter()
            .filter(|t| keep(t.bounds()))
            .cloned()
            .collect();
        let climate: Vec<ClimateBias> = self
            .climate
            .iter()
            .filter(|t| keep(t.bounds()))
            .cloned()
            .collect();
        let exclusions = Exclusions(
            self.exclusions
                .iter()
                .filter(|t| keep(t.bounds()))
                .cloned()
                .collect(),
        );
        // Channels are compiled once so each segment keeps its bounding box.
        let channels: Vec<Option<ChannelEval>> = density.iter().map(ChannelEval::new).collect();
        let base_height: Vec<&dyn BaseHeightBias> = density
            .iter()
            .zip(&channels)
            .map(|(t, c)| match c {
                Some(c) => c as &dyn BaseHeightBias,
                None => t as &dyn BaseHeightBias,
            })
            .collect();
        let clamp: Vec<&dyn DensityClamp> = channels
            .iter()
            .flatten()
            .map(|c| c as &dyn DensityClamp)
            .collect();
        let temp = TemperatureTerms(&climate);
        let humid = HumidityTerms(&climate);
        let temperature: [&dyn TemperatureBias; 1] = [&temp];
        let humidity: [&dyn HumidityBias; 1] = [&humid];
        let open = ChannelsOpen(&channels);
        let biases = Biases {
            temperature: if climate.is_empty() {
                &[]
            } else {
                &temperature
            },
            humidity: if climate.is_empty() { &[] } else { &humidity },
            base_height: &base_height,
            clamp: &clamp,
            protected: if exclusions.0.is_empty() {
                None
            } else {
                Some(&exclusions as &dyn ProtectedMask)
            },
            open_water: if clamp.is_empty() { None } else { Some(&open) },
            ..Biases::default()
        };
        f(&biases)
    }
}

/// Free-function form of [`Layout::generate_region`].
pub fn generate_region(
    layout: &Layout,
    x0: i32,
    z0: i32,
    w: u32,
    d: u32,
    decorate: bool,
) -> Region {
    layout.generate_region(x0, z0, w, d, decorate)
}
