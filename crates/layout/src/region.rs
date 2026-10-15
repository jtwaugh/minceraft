//! `Layout` and the `generate_region` wrapper (design: world-layout 4.2).

use crate::{ClimateBias, DensityTerm, Exclusion, Exclusions, HumidityTerms, TemperatureTerms};
use terrain::{
    generate_region_with_biases, BaseHeightBias, Biases, HumidityBias, ProtectedMask, Region,
    TemperatureBias,
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
        match *self {
            DensityTerm::RadialBaseHeight {
                center, r_outer, ..
            } => Some(circle(center, r_outer)),
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

    fn generate(
        &self,
        x0: i32,
        z0: i32,
        w: u32,
        d: u32,
        decorate: bool,
        keep: impl Fn(Option<Bounds>) -> bool,
    ) -> Region {
        let density: Vec<DensityTerm> = self
            .density
            .iter()
            .filter(|t| keep(t.bounds()))
            .copied()
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
        let base_height: Vec<&dyn BaseHeightBias> =
            density.iter().map(|t| t as &dyn BaseHeightBias).collect();
        let temp = TemperatureTerms(&climate);
        let humid = HumidityTerms(&climate);
        let temperature: [&dyn TemperatureBias; 1] = [&temp];
        let humidity: [&dyn HumidityBias; 1] = [&humid];
        let biases = Biases {
            temperature: if climate.is_empty() {
                &[]
            } else {
                &temperature
            },
            humidity: if climate.is_empty() { &[] } else { &humidity },
            base_height: &base_height,
            protected: if exclusions.0.is_empty() {
                None
            } else {
                Some(&exclusions as &dyn ProtectedMask)
            },
            ..Biases::default()
        };
        generate_region_with_biases(self.seed, x0, z0, w, d, &biases, decorate)
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
