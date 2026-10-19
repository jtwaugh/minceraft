//! Per-random-tick growth odds of one crop. See SPEC "Crop growth".
//!
//! Pure arithmetic: no world access and no RNG. The caller rolls the
//! 1-in-N chance and applies the random-tick rate.

/// What one of the 8 squares around a crop holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surround {
    /// Not farmland: adds nothing.
    Other,
    DryFarmland,
    WetFarmland,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrowthInput {
    /// The farmland under the crop is wet (moisture above 0).
    pub wet: bool,
    /// The 8 surrounding squares, in any order.
    pub surround: [Surround; 8],
    /// Crop at -X, +X.
    pub crop_x: [bool; 2],
    /// Crop at -Z, +Z.
    pub crop_z: [bool; 2],
    /// Crop on any of the 4 diagonals.
    pub crop_diagonal: [bool; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrowthOdds {
    pub f: f64,
    /// The chance per random tick is 1 in `denominator`.
    pub denominator: u32,
}

/// Growth odds for one crop. `f` is a multiple of 0.25 (or 0.125 after
/// halving), so every step is exact in `f64`.
pub fn growth_odds(input: &GrowthInput) -> GrowthOdds {
    let mut f = if input.wet { 4.0 } else { 2.0 };
    for s in input.surround {
        f += match s {
            Surround::Other => 0.0,
            Surround::DryFarmland => 0.25,
            Surround::WetFarmland => 0.75,
        };
    }
    let on_x = input.crop_x.iter().any(|&c| c);
    let on_z = input.crop_z.iter().any(|&c| c);
    if input.crop_diagonal.iter().any(|&c| c) || (on_x && on_z) {
        f /= 2.0;
    }
    GrowthOdds {
        f,
        denominator: (100.0 / f) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(wet: bool, s: Surround) -> GrowthInput {
        GrowthInput {
            wet,
            surround: [s; 8],
            crop_x: [false; 2],
            crop_z: [false; 2],
            crop_diagonal: [false; 4],
        }
    }

    fn check(i: &GrowthInput, f: f64, n: u32) {
        let o = growth_odds(i);
        assert_eq!((o.f, o.denominator), (f, n));
    }

    #[test]
    fn single_row_wet() {
        let mut i = input(true, Surround::WetFarmland);
        i.crop_x = [true, true];
        check(&i, 10.0, 10);
    }

    #[test]
    fn single_row_dry() {
        let mut i = input(false, Surround::DryFarmland);
        i.crop_z = [true, true];
        check(&i, 4.0, 25);
    }

    #[test]
    fn full_field_wet_and_dry() {
        let full = |mut i: GrowthInput| {
            i.crop_x = [true; 2];
            i.crop_z = [true; 2];
            i.crop_diagonal = [true; 4];
            i
        };
        check(&full(input(true, Surround::WetFarmland)), 5.0, 20);
        check(&full(input(false, Surround::DryFarmland)), 2.0, 50);
    }

    #[test]
    fn straight_row_is_not_halved() {
        let mut i = input(true, Surround::WetFarmland);
        i.crop_x = [true, false];
        check(&i, 10.0, 10);
        i.crop_x = [false; 2];
        i.crop_z = [false, true];
        check(&i, 10.0, 10);
    }

    #[test]
    fn both_axes_halve() {
        let mut i = input(true, Surround::WetFarmland);
        i.crop_x = [true, false];
        i.crop_z = [false, true];
        check(&i, 5.0, 20);
    }

    #[test]
    fn one_diagonal_halves() {
        let mut i = input(true, Surround::WetFarmland);
        i.crop_diagonal[2] = true;
        check(&i, 5.0, 20);
    }

    #[test]
    fn surrounding_farmland_counts_without_crops() {
        // No crops anywhere, wet centre, all wet around.
        check(&input(true, Surround::WetFarmland), 10.0, 10);
        check(&input(false, Surround::DryFarmland), 4.0, 25);
    }

    #[test]
    fn non_farmland_adds_nothing() {
        check(&input(true, Surround::Other), 4.0, 25);
        let mut i = input(false, Surround::Other);
        i.surround[0] = Surround::WetFarmland;
        i.surround[1] = Surround::DryFarmland;
        check(&i, 3.0, 33);
    }

    #[test]
    fn denominator_truncates() {
        // Dry centre (2) + 6 dry squares (1.5) = 3.5; 100 / 3.5 = 28.57.
        let mut i = input(false, Surround::Other);
        i.surround[..6].fill(Surround::DryFarmland);
        check(&i, 3.5, 28);
    }
}
