//! Environmental niches for flora (docs/design/swedish-summer-flora.md
//! sections 3 and 4): the layers an entry's `niche` may name and the
//! trapezoid response to one layer's value.

use crate::flora::{
    NICHE_MARGIN_HEIGHT_ABOVE_SEA, NICHE_MARGIN_HOLLOW, NICHE_MARGIN_LIGHT, NICHE_MARGIN_SLOPE,
    NICHE_MARGIN_SOIL_DEPTH, NICHE_MARGIN_SUN_FACING,
};

/// A column layer a `niche` range applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NicheLayer {
    HeightAboveSea,
    SoilDepth,
    Light,
    Slope,
    SunFacing,
    Hollow,
}

impl NicheLayer {
    /// Soft margin from `[settings]` `niche_margin_<layer>` in `data/flora.toml`.
    pub fn margin(self) -> f64 {
        match self {
            NicheLayer::HeightAboveSea => NICHE_MARGIN_HEIGHT_ABOVE_SEA,
            NicheLayer::SoilDepth => NICHE_MARGIN_SOIL_DEPTH,
            NicheLayer::Light => NICHE_MARGIN_LIGHT,
            NicheLayer::Slope => NICHE_MARGIN_SLOPE,
            NicheLayer::SunFacing => NICHE_MARGIN_SUN_FACING,
            NicheLayer::Hollow => NICHE_MARGIN_HOLLOW,
        }
    }
}

/// 1 for `value` in `[lo, hi]`, falling linearly to 0 at `margin` outside it.
pub fn response(value: f64, lo: f64, hi: f64, margin: f64) -> f64 {
    let out = (lo - value).max(value - hi).max(0.0);
    (1.0 - out / margin).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trapezoid() {
        let r = |v| response(v, 2.0, 4.0, 1.0);
        assert_eq!(r(3.0), 1.0);
        assert_eq!(r(2.0), 1.0);
        assert_eq!(r(4.0), 1.0);
        assert_eq!(r(1.5), 0.5);
        assert_eq!(r(4.5), 0.5);
        assert_eq!(r(1.0), 0.0);
        assert_eq!(r(5.0), 0.0);
        assert_eq!(r(-10.0), 0.0);
        assert_eq!(r(10.0), 0.0);
    }

    #[test]
    fn margins_come_from_data() {
        assert!(NicheLayer::Light.margin() > 0.0);
    }
}
