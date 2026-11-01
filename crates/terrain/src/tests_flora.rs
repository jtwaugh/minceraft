//! Tests for the column-local flora pass (docs/design/biome-flora.md D.2).

use crate::climate::biome_for;
use crate::data::{Biome, Block};
use crate::decorate::{generate_decorated_with, World};
use crate::decorate::{MARGIN_CELLS, READ_REACH};
use crate::flora::{place, Flora, FloraShape, FloraTint, FLORA};
use crate::flora::{
    FloraRequires, FLORA_EDGE_RADIUS, FLORA_EXPOSURE_CAP, FLORA_HOLLOW_RADII, FLORA_LIGHT_RADIUS,
    FLORA_WIND,
};
use crate::niche::{edge, exposure, hollow, light, NicheLayer};
use crate::{Biases, Generator, Region, HEIGHT, SEA_LEVEL};

/// Two entries with high chances in different biomes, so tests see plenty.
static TEST: [Flora; 2] = [
    Flora {
        name: "a",
        block: Block::Daisy,
        upper: None,
        shape: FloraShape::Cross,
        ground: &[Block::Grass],
        tint: FloraTint::None,
        cluster_scale: 8.0,
        salt: 11,
        requires: None,
        snow_stays: false,
        biomes: &[(Biome::Plains, 0.9), (Biome::Forest, 0.5)],
        niche: &[],
    },
    Flora {
        name: "b",
        block: Block::Rose,
        upper: None,
        shape: FloraShape::GroundCover,
        ground: &[Block::Sand, Block::Grass],
        tint: FloraTint::None,
        cluster_scale: 8.0,
        salt: 12,
        requires: None,
        snow_stays: false,
        biomes: &[(Biome::Desert, 0.9), (Biome::Taiga, 0.5)],
        niche: &[],
    },
];

fn gen(seed: u64, x0: i32, z0: i32, w: u32, d: u32, table: &[Flora]) -> Region {
    let g = Generator::new(seed);
    generate_decorated_with(&g, x0, z0, w, d, &Biases::default(), table).0
}

fn at(r: &Region, x: i32, y: usize, z: i32) -> u8 {
    r.blocks[((z - r.z0) as usize * r.width as usize + (x - r.x0) as usize) * HEIGHT + y]
}

fn count(r: &Region, b: Block) -> usize {
    r.blocks.iter().filter(|&&v| v == b as u8).count()
}

#[test]
fn shipped_table_adds_only_flora_blocks_to_empty_table_output() {
    for seed in [12345u64, 9, 42] {
        let today = gen(seed, -50, 30, 120, 90, &[]);
        let r = crate::generate_decorated(seed, -50, 30, 120, 90);
        let mut added = 0;
        for (a, b) in r.blocks.iter().zip(&today.blocks) {
            if a != b {
                // Snow runs after flora, so a plant can replace a snow layer.
                assert!(*b == Block::Air as u8 || *b == Block::SnowLayer as u8);
                assert!(FLORA
                    .iter()
                    .any(|f| { f.block as u8 == *a || f.upper.is_some_and(|u| u as u8 == *a) }));
                added += 1;
            }
        }
        assert!(added > 0, "seed {seed}");
    }
}

#[test]
fn shipped_entries_stay_in_their_biomes() {
    for seed in [12345u64, 9, 42] {
        let (x0, z0, w, d) = (-200, -200, 400, 400);
        let base = gen(seed, x0, z0, w, d, &[]);
        let with = gen(seed, x0, z0, w, d, &FLORA);
        let g = Generator::new(seed);
        for z in z0..z0 + d as i32 {
            for x in x0..x0 + w as i32 {
                let (t, h) = g.climate.at(x, z, &[], &[]);
                let biome = biome_for(t, h);
                for y in 1..HEIGHT {
                    let (a, b) = (at(&base, x, y, z), at(&with, x, y, z));
                    if a == b {
                        continue;
                    }
                    let f = FLORA
                        .iter()
                        .find(|f| f.block as u8 == b || f.upper.is_some_and(|u| u as u8 == b))
                        .unwrap();
                    assert!(
                        f.biomes.iter().any(|e| e.0 == biome),
                        "{} in {biome:?} at {x},{z}",
                        f.name
                    );
                }
            }
        }
    }
}

#[test]
fn shipped_table_is_independent_of_region() {
    let big = gen(12345, -128, -128, 256, 256, &FLORA);
    for (ox, oz, w, d) in [(-37, 11, 41, 29), (0, 0, 17, 63), (60, -50, 33, 33)] {
        let small = gen(12345, ox, oz, w, d, &FLORA);
        for z in oz..oz + d as i32 {
            for x in ox..ox + w as i32 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&small, x, y, z), at(&big, x, y, z), "{x},{y},{z}");
                }
            }
        }
    }
}

#[test]
fn only_air_changes_and_flora_is_placed() {
    for seed in [12345u64, 9, 42] {
        let base = gen(seed, 0, 0, 160, 160, &[]);
        let with = gen(seed, 0, 0, 160, 160, &TEST);
        let mut changed = 0;
        for (a, b) in base.blocks.iter().zip(&with.blocks) {
            if a != b {
                // Snow runs after flora and skips an occupied top cell
                // (decision #1072), so a snow cell may hold a plant instead.
                assert!(*a == Block::Air as u8 || *a == Block::SnowLayer as u8);
                changed += 1;
            }
        }
        assert!(changed > 0, "seed {seed}");
    }
}

#[test]
fn flora_only_in_listed_biomes_on_ground() {
    let seed = 12345;
    let (x0, z0, w, d) = (-100, -100, 220, 220);
    let base = gen(seed, x0, z0, w, d, &[]);
    let with = gen(seed, x0, z0, w, d, &TEST);
    let g = Generator::new(seed);
    let mut seen = 0;
    for z in z0..z0 + d as i32 {
        for x in x0..x0 + w as i32 {
            let (t, h) = g.climate.at(x, z, &[], &[]);
            let biome = biome_for(t, h);
            for y in 1..HEIGHT {
                let (a, b) = (at(&base, x, y, z), at(&with, x, y, z));
                if a == b {
                    continue;
                }
                seen += 1;
                let ok = if b == Block::Daisy as u8 {
                    matches!(biome, Biome::Plains | Biome::Forest)
                } else {
                    b == Block::Rose as u8 && matches!(biome, Biome::Desert | Biome::Taiga)
                };
                assert!(ok, "{b} in {biome:?} at {x},{z}");
                let below = at(&with, x, y - 1, z);
                assert!(below == Block::Grass as u8 || below == Block::Sand as u8);
            }
        }
    }
    assert!(seen > 0);
}

#[test]
fn flora_is_independent_of_region() {
    let seed = 12345;
    let big = gen(seed, -64, -64, 192, 192, &TEST);
    assert!(count(&big, Block::Daisy) > 0);
    for (ox, oz, w, d) in [(-37, 11, 41, 29), (0, 0, 17, 63), (60, -50, 33, 33)] {
        let small = gen(seed, ox, oz, w, d, &TEST);
        for z in oz..oz + d as i32 {
            for x in ox..ox + w as i32 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&small, x, y, z), at(&big, x, y, z), "{x},{y},{z}");
                }
            }
        }
    }
}

macro_rules! plains_entry {
    ($chance:expr) => {
        [Flora {
            name: "p",
            block: Block::Daisy,
            upper: None,
            shape: FloraShape::Cross,
            ground: &[Block::Grass],
            tint: FloraTint::None,
            cluster_scale: 8.0,
            salt: 11,
            requires: None,
            snow_stays: false,
            biomes: &[(Biome::Plains, $chance), (Biome::Forest, $chance)],
            niche: &[],
        }]
    };
}

/// Daisies per 8x8 cell of `r`.
fn daisies_per_cell(r: &Region) -> std::collections::HashMap<(i32, i32), usize> {
    let mut cells = std::collections::HashMap::new();
    for z in r.z0..r.z0 + r.depth as i32 {
        for x in r.x0..r.x0 + r.width as i32 {
            let c = cells
                .entry((x.div_euclid(8), z.div_euclid(8)))
                .or_insert(0usize);
            *c += (0..HEIGHT).any(|y| at(r, x, y, z) == Block::Daisy as u8) as usize;
        }
    }
    cells
}

#[test]
fn flora_density_varies_between_noise_cells() {
    // A huge chance fills every eligible column, so it counts them per cell;
    // against that, a fixed chance must land at clearly different fractions
    // in different cells, which only the patch noise can do.
    static ALL: [Flora; 1] = plains_entry!(1e9);
    static HALF: [Flora; 1] = plains_entry!(0.5);
    let all = daisies_per_cell(&gen(12345, -64, -64, 192, 192, &ALL));
    let half = daisies_per_cell(&gen(12345, -64, -64, 192, 192, &HALF));
    let ratios: Vec<f64> = all
        .iter()
        .filter(|(_, &e)| e >= 32)
        .map(|(k, &e)| half[k] as f64 / e as f64)
        .collect();
    let sparse = ratios.iter().filter(|&&r| r < 0.15).count();
    let dense = ratios.iter().filter(|&&r| r > 0.85).count();
    assert!(sparse >= 1 && dense >= 1, "{sparse} sparse, {dense} dense");
}

/// Two equal-chance entries that compete on grass in plains and forest.
static RIVALS: [Flora; 2] = [
    Flora {
        name: "r1",
        block: Block::Daisy,
        upper: None,
        shape: FloraShape::Cross,
        ground: &[Block::Grass],
        tint: FloraTint::None,
        cluster_scale: 8.0,
        salt: 21,
        requires: None,
        snow_stays: false,
        biomes: &[(Biome::Plains, 1e9), (Biome::Forest, 1e9)],
        niche: &[],
    },
    Flora {
        name: "r2",
        block: Block::Rose,
        upper: None,
        shape: FloraShape::Cross,
        ground: &[Block::Grass],
        tint: FloraTint::None,
        cluster_scale: 8.0,
        salt: 22,
        requires: None,
        snow_stays: false,
        biomes: &[(Biome::Plains, 1e9), (Biome::Forest, 1e9)],
        niche: &[],
    },
];

fn flower_at(r: &Region, x: i32, z: i32) -> Option<u8> {
    (0..HEIGHT)
        .map(|y| at(r, x, y, z))
        .find(|&b| b == Block::Daisy as u8 || b == Block::Rose as u8)
}

#[test]
fn competing_entries_form_drifts() {
    // Every eligible column is flowered, so each holds the winning entry.
    // Independent equal rolls would agree between neighbours half the time;
    // the winner noise must push that well above 75%.
    let (mut same, mut pairs) = (0, 0);
    for seed in [12345u64, 9, 42, 7] {
        let r = gen(seed, 0, 0, 64, 64, &RIVALS);
        for z in 0..63 {
            for x in 0..63 {
                let Some(a) = flower_at(&r, x, z) else {
                    continue;
                };
                for (nx, nz) in [(x + 1, z), (x, z + 1)] {
                    if let Some(b) = flower_at(&r, nx, nz) {
                        pairs += 1;
                        same += (a == b) as usize;
                    }
                }
            }
        }
    }
    assert!(pairs >= 300, "only {pairs} flowered pairs");
    assert!(same * 4 > pairs * 3, "{same} of {pairs} neighbours agree");
}

#[test]
fn competing_entries_are_independent_of_region() {
    let big = gen(12345, -64, -64, 192, 192, &RIVALS);
    assert!(count(&big, Block::Daisy) > 0 && count(&big, Block::Rose) > 0);
    for (ox, oz, w, d) in [(-37, 11, 41, 29), (0, 0, 17, 63), (60, -50, 33, 33)] {
        let small = gen(12345, ox, oz, w, d, &RIVALS);
        for z in oz..oz + d as i32 {
            for x in ox..ox + w as i32 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&small, x, y, z), at(&big, x, y, z), "{x},{y},{z}");
                }
            }
        }
    }
}

#[test]
fn ineligible_entry_does_not_change_single_entry_output() {
    // `b` in TEST grows only in desert and taiga, so in plains and forest
    // the lone `a` behaves exactly as without it.
    let alone = gen(12345, -64, -64, 160, 160, &TEST[..1]);
    let both = gen(12345, -64, -64, 160, 160, &TEST);
    for (i, (a, b)) in alone.blocks.iter().zip(&both.blocks).enumerate() {
        assert!(a == b || *b == Block::Rose as u8, "{i}");
    }
}

/// A two-block plant that grows on every plains and forest grass column.
static TALL: [Flora; 1] = [Flora {
    name: "t",
    block: Block::TallGrassDoubleLower,
    upper: Some(Block::TallGrassDoubleUpper),
    shape: FloraShape::TallCross,
    ground: &[Block::Grass],
    tint: FloraTint::Grass,
    cluster_scale: 8.0,
    salt: 31,
    requires: None,
    snow_stays: false,
    biomes: &[(Biome::Plains, 1e9), (Biome::Forest, 1e9)],
    niche: &[],
}];

#[test]
fn tall_plants_are_never_a_lone_half() {
    let (lo, up) = (
        Block::TallGrassDoubleLower as u8,
        Block::TallGrassDoubleUpper as u8,
    );
    let mut pairs = 0;
    for seed in [12345u64, 9] {
        let r = gen(seed, 0, 0, 96, 96, &TALL);
        for z in 0..96 {
            for x in 0..96 {
                for y in 0..HEIGHT {
                    let b = at(&r, x, y, z);
                    if b == lo {
                        assert!(y + 1 < HEIGHT && at(&r, x, y + 1, z) == up, "{x},{y},{z}");
                        pairs += 1;
                    } else if b == up {
                        assert!(y > 0 && at(&r, x, y - 1, z) == lo, "{x},{y},{z}");
                    }
                }
            }
        }
    }
    assert!(pairs > 100, "only {pairs} plants");
}

#[test]
fn tall_plants_are_independent_of_region() {
    let big = gen(12345, -64, -64, 192, 192, &TALL);
    assert!(count(&big, Block::TallGrassDoubleLower) > 0);
    for (ox, oz, w, d) in [(-37, 11, 41, 29), (0, 0, 17, 63), (60, -50, 33, 33)] {
        let small = gen(12345, ox, oz, w, d, &TALL);
        for z in oz..oz + d as i32 {
            for x in ox..ox + w as i32 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&small, x, y, z), at(&big, x, y, z), "{x},{y},{z}");
                }
            }
        }
    }
}

#[test]
fn no_flora_where_a_trunk_or_beta_plant_occupies_the_air_cell() {
    let plain = gen(12345, 0, 0, 96, 96, &[]);
    let with = gen(12345, 0, 0, 96, 96, &TALL);
    let mut checked = 0;
    for z in 0..96 {
        for x in 0..96 {
            for y in 1..HEIGHT - 1 {
                let below = at(&plain, x, y - 1, z);
                let cell = at(&plain, x, y, z);
                let occupied = cell == Block::OakLog as u8
                    || cell == Block::BirchLog as u8
                    || cell == Block::SpruceLog as u8
                    || cell == Block::TallGrass as u8
                    || cell == Block::Fern as u8
                    || cell == Block::Dandelion as u8
                    || cell == Block::Rose as u8;
                if below == Block::Grass as u8 && occupied {
                    checked += 1;
                    assert_eq!(at(&with, x, y, z), cell, "{x},{y},{z}");
                    assert_eq!(at(&with, x, y + 1, z), at(&plain, x, y + 1, z));
                }
            }
        }
    }
    assert!(checked > 50, "only {checked} occupied cells");
}

#[test]
fn fern_double_is_taiga_only_and_stacked() {
    let (lo, up) = (Block::FernDoubleLower as u8, Block::FernDoubleUpper as u8);
    // Taiga is cold, so snow replaces ferns there (D.2); any that survive
    // (none expected until the frost_grass exception) must still be whole.
    for seed in [12345u64, 9, 42] {
        let (x0, z0, w, d) = (-200, -200, 400, 400);
        let r = gen(seed, x0, z0, w, d, &FLORA);
        let g = Generator::new(seed);
        for z in z0..z0 + d as i32 {
            for x in x0..x0 + w as i32 {
                for y in 1..HEIGHT - 1 {
                    let b = at(&r, x, y, z);
                    if b == lo {
                        let (t, h) = g.climate.at(x, z, &[], &[]);
                        assert_eq!(biome_for(t, h), Biome::Taiga, "{x},{z}");
                        assert_eq!(at(&r, x, y + 1, z), up, "{x},{y},{z}");
                    } else if b == up {
                        assert_eq!(at(&r, x, y - 1, z), lo, "{x},{y},{z}");
                    }
                }
            }
        }
    }
}

const COLD: [(Biome, f64); 2] = [(Biome::Tundra, 1e9), (Biome::Taiga, 1e9)];

fn cold(shape: FloraShape, block: Block, upper: Option<Block>) -> [Flora; 1] {
    [Flora {
        name: "c",
        block,
        upper,
        shape,
        ground: &[Block::Grass],
        tint: FloraTint::None,
        cluster_scale: 8.0,
        salt: 41,
        biomes: &COLD,
        niche: &[],
        requires: None,
        snow_stays: false,
    }]
}

/// Over cells that are snow in the plant-free world, counts cells that are
/// still snow and cells holding `block` instead; any other block fails, as
/// does `upper` standing anywhere in a snow cell's column above ground.
fn snow_cells(table: &[Flora]) -> (usize, usize) {
    let (block, upper) = (table[0].block as u8, table[0].upper.map(|u| u as u8));
    let (mut snow, mut held) = (0, 0);
    for seed in [12345u64, 9, 42] {
        let base = gen(seed, 0, 0, 128, 128, &[]);
        let with = gen(seed, 0, 0, 128, 128, table);
        for z in 0..128 {
            for x in 0..128 {
                for y in 0..HEIGHT - 1 {
                    if at(&base, x, y, z) != Block::SnowLayer as u8 {
                        continue;
                    }
                    let b = at(&with, x, y, z);
                    if b == Block::SnowLayer as u8 {
                        snow += 1;
                        assert!(Some(at(&with, x, y + 1, z)) != upper || upper.is_none());
                    } else {
                        assert_eq!(b, block, "{x},{y},{z}");
                        held += 1;
                    }
                }
            }
        }
    }
    (snow, held)
}

#[test]
fn snow_replaces_cross_plants() {
    let (snow, held) = snow_cells(&cold(FloraShape::Cross, Block::Daisy, None));
    assert!(snow > 50 && held == 0, "{snow} {held}");
}

#[test]
fn snow_replaces_both_halves_of_tall_plants() {
    let t = cold(
        FloraShape::TallCross,
        Block::TallGrassDoubleLower,
        Some(Block::TallGrassDoubleUpper),
    );
    let (snow, held) = snow_cells(&t);
    assert!(snow > 50 && held == 0, "{snow} {held}");
}

#[test]
fn snow_leaves_ground_cover_and_bush_alone() {
    for shape in [FloraShape::GroundCover, FloraShape::Bush] {
        let (snow, held) = snow_cells(&cold(shape, Block::Rose, None));
        assert!(held > 50, "{shape:?} {snow} {held}");
    }
}

#[test]
fn snow_over_flora_is_independent_of_region() {
    let t = cold(
        FloraShape::TallCross,
        Block::TallGrassDoubleLower,
        Some(Block::TallGrassDoubleUpper),
    );
    let big = gen(12345, -64, -64, 192, 192, &t);
    for (ox, oz, w, d) in [(-37, 11, 41, 29), (0, 0, 17, 63)] {
        let small = gen(12345, ox, oz, w, d, &t);
        for z in oz..oz + d as i32 {
            for x in ox..ox + w as i32 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&small, x, y, z), at(&big, x, y, z), "{x},{y},{z}");
                }
            }
        }
    }
}

#[test]
fn cattail_is_swampland_only_stacked_and_beside_water() {
    let (lo, up) = (Block::CattailLower as u8, Block::CattailUpper as u8);
    let mut pairs = 0;
    for seed in [12345u64, 9, 42] {
        let (x0, z0, w, d) = (-300, -300, 600, 600);
        let r = gen(seed, x0, z0, w, d, &FLORA);
        let g = Generator::new(seed);
        for z in z0..z0 + d as i32 {
            for x in x0..x0 + w as i32 {
                for y in 1..HEIGHT - 1 {
                    let b = at(&r, x, y, z);
                    if b == lo {
                        let (t, h) = g.climate.at(x, z, &[], &[]);
                        assert_eq!(biome_for(t, h), Biome::Swampland, "{x},{z}");
                        assert_eq!(at(&r, x, y + 1, z), up, "{x},{y},{z}");
                        let ground = at(&r, x, y - 1, z);
                        assert!(ground == Block::Grass as u8 || ground == Block::Dirt as u8);
                        let wet = |dx: i32, dz: i32| {
                            let (nx, nz) = (x + dx, z + dz);
                            let inside = (x0..x0 + w as i32).contains(&nx)
                                && (z0..z0 + d as i32).contains(&nz);
                            !inside || at(&r, nx, y - 1, nz) == Block::Water as u8
                        };
                        let near = (-1..=1).any(|dz| (-1..=1).any(|dx| wet(dx, dz)));
                        assert!(near, "no water beside cattail at {x},{y},{z}");
                        pairs += 1;
                    } else if b == up {
                        assert_eq!(at(&r, x, y - 1, z), lo, "{x},{y},{z}");
                    }
                }
            }
        }
    }
    assert!(pairs > 0, "no cattails");
}

#[test]
fn cattail_is_independent_of_region_and_flora_off_matches_empty_table() {
    let seed = 12345;
    let big = gen(seed, -150, -150, 300, 300, &FLORA);
    let part = gen(seed, -40, -60, 90, 80, &FLORA);
    for z in part.z0..part.z0 + 80 {
        for x in part.x0..part.x0 + 90 {
            for y in 0..HEIGHT {
                assert_eq!(at(&big, x, y, z), at(&part, x, y, z), "{x},{y},{z}");
            }
        }
    }
    let off = gen(seed, -150, -150, 300, 300, &[]);
    assert_eq!(count(&off, Block::CattailLower), 0);
}

fn lily_columns(r: &Region, seed: u64) -> (usize, usize) {
    let g = Generator::new(seed);
    let (pad, water) = (Block::LilyPad as u8, Block::Water as u8);
    let (mut pads, mut open_water) = (0, 0);
    for z in r.z0..r.z0 + r.depth as i32 {
        for x in r.x0..r.x0 + r.width as i32 {
            let (t, h) = g.climate.at(x, z, &[], &[]);
            for y in 1..HEIGHT - 1 {
                let b = at(r, x, y, z);
                if b == pad {
                    pads += 1;
                    assert_eq!(biome_for(t, h), Biome::Swampland, "{x},{z}");
                    assert_eq!(at(r, x, y - 1, z), water, "{x},{y},{z}");
                    assert_eq!(at(r, x, y + 1, z), 0, "{x},{y},{z}");
                    let land = |dx: i32, dz: i32| {
                        let (nx, nz) = (x + dx, z + dz);
                        let inside = (r.x0..r.x0 + r.width as i32).contains(&nx)
                            && (r.z0..r.z0 + r.depth as i32).contains(&nz);
                        let n = at(
                            r,
                            nx.clamp(r.x0, r.x0 + r.width as i32 - 1),
                            y - 1,
                            nz.clamp(r.z0, r.z0 + r.depth as i32 - 1),
                        );
                        !inside || (n != water && n != 0)
                    };
                    assert!(
                        (-4..=4).any(|dz| (-4..=4).any(|dx| land(dx, dz))),
                        "{x},{y},{z}"
                    );
                } else if b == water && at(r, x, y + 1, z) == 0 {
                    open_water += 1;
                }
            }
        }
    }
    (pads, open_water)
}

#[test]
fn lily_pads_sit_on_swampland_water_near_land_only() {
    let mut pads = 0;
    for seed in [12345u64, 9, 42] {
        let r = gen(seed, -300, -300, 600, 600, &FLORA);
        let (p, open) = lily_columns(&r, seed);
        assert!(p < open, "pads must not cover all open water");
        pads += p;
    }
    assert!(pads > 0, "no lily pads");
}

#[test]
fn lily_pads_are_independent_of_region_and_flora_off_matches_empty_table() {
    let seed = 12345;
    let big = gen(seed, -300, -300, 600, 600, &FLORA);
    assert!(count(&big, Block::LilyPad) > 0);
    let part = gen(seed, -40, -60, 90, 80, &FLORA);
    for z in part.z0..part.z0 + 80 {
        for x in part.x0..part.x0 + 90 {
            for y in 0..HEIGHT {
                assert_eq!(at(&big, x, y, z), at(&part, x, y, z), "{x},{y},{z}");
            }
        }
    }
    for seed in [12345u64, 9, 42] {
        let off = gen(seed, -150, -150, 300, 300, &[]);
        assert_eq!(count(&off, Block::LilyPad), 0);
    }
}

#[test]
fn jungle_bush_is_rainforest_grass_under_leaves_only() {
    let bush = Block::JungleBush as u8;
    let mut found = 0;
    for (seed, x0, z0) in [(12345u64, 1120, 1120), (9, -1184, 288), (42, 1888, -1632)] {
        let (w, d) = (128, 128);
        let r = gen(seed, x0, z0, w, d, &FLORA);
        let g = Generator::new(seed);
        for z in z0..z0 + d as i32 {
            for x in x0..x0 + w as i32 {
                for y in 1..HEIGHT - 1 {
                    if at(&r, x, y, z) != bush {
                        continue;
                    }
                    let (t, h) = g.climate.at(x, z, &[], &[]);
                    assert_eq!(biome_for(t, h), Biome::Rainforest, "{x},{z}");
                    assert_eq!(at(&r, x, y - 1, z), Block::Grass as u8, "{x},{y},{z}");
                    let roof = (y + 1..HEIGHT).any(|yy| crate::data::is_leaves(at(&r, x, yy, z)));
                    assert!(roof, "no leaves above bush at {x},{y},{z}");
                    found += 1;
                }
            }
        }
    }
    assert!(found > 0, "no jungle bushes");
}

#[test]
fn jungle_bush_is_independent_of_region_and_flora_off_matches_empty_table() {
    let seed = 12345;
    let big = gen(seed, 1120, 1120, 128, 128, &FLORA);
    assert!(count(&big, Block::JungleBush) > 0);
    let part = gen(seed, 1150, 1140, 70, 80, &FLORA);
    for z in part.z0..part.z0 + 80 {
        for x in part.x0..part.x0 + 70 {
            for y in 0..HEIGHT {
                assert_eq!(at(&big, x, y, z), at(&part, x, y, z), "{x},{y},{z}");
            }
        }
    }
    for (seed, x0, z0) in [(12345u64, 1120, 1120), (9, -1184, 288), (42, 1888, -1632)] {
        let off = gen(seed, x0, z0, 128, 128, &[]);
        assert_eq!(count(&off, Block::JungleBush), 0);
    }
}

#[test]
fn marsh_orchid_is_swampland_only_on_grass_and_independent_of_region() {
    let orchid = Block::MarshOrchid as u8;
    let mut n = 0;
    for seed in [12345u64, 9, 42] {
        let big = gen(seed, -300, -300, 600, 600, &FLORA);
        let g = Generator::new(seed);
        for z in big.z0..big.z0 + 600 {
            for x in big.x0..big.x0 + 600 {
                for y in 1..HEIGHT {
                    if at(&big, x, y, z) == orchid {
                        let (t, h) = g.climate.at(x, z, &[], &[]);
                        assert_eq!(biome_for(t, h), Biome::Swampland, "{x},{z}");
                        assert_eq!(at(&big, x, y - 1, z), Block::Grass as u8);
                        n += 1;
                    }
                }
            }
        }
        let part = gen(seed, -40, -60, 90, 80, &FLORA);
        for z in part.z0..part.z0 + 80 {
            for x in part.x0..part.x0 + 90 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&big, x, y, z), at(&part, x, y, z), "{x},{y},{z}");
                }
            }
        }
        assert_eq!(
            count(&gen(seed, -150, -150, 300, 300, &[]), Block::MarshOrchid),
            0
        );
    }
    assert!(n > 0, "no marsh orchids");
}

#[test]
fn shrubland_gets_bush_berry_bush_and_sparse_dry_grass() {
    let (bush, berry, dry) = (
        Block::Bush as u8,
        Block::BerryBush as u8,
        Block::DryGrass as u8,
    );
    let (mut n_bush, mut n_berry, mut n_dry) = (0, 0, 0);
    for seed in [12345u64, 9, 42] {
        let (x0, z0, w, d) = (-400, -400, 800, 800);
        let r = gen(seed, x0, z0, w, d, &FLORA);
        let g = Generator::new(seed);
        for z in z0..z0 + d as i32 {
            for x in x0..x0 + w as i32 {
                let (t, h) = g.climate.at(x, z, &[], &[]);
                let shrub = biome_for(t, h) == Biome::Shrubland;
                for y in 1..HEIGHT - 1 {
                    let b = at(&r, x, y, z);
                    if b == bush {
                        assert!(shrub, "bush outside Shrubland at {x},{z}");
                        n_bush += 1;
                    } else if b == berry && shrub {
                        n_berry += 1;
                    } else if b == dry && shrub {
                        n_dry += 1;
                    }
                }
            }
        }
    }
    assert!(
        n_bush > 0 && n_berry > 0 && n_dry > 0,
        "{n_bush} {n_berry} {n_dry}"
    );
    assert!(n_dry < n_bush, "dry grass must stay sparse");
}

#[test]
fn shrubland_flora_is_independent_of_region_and_flora_off_matches_empty_table() {
    let seed = 12345;
    let big = gen(seed, -400, -400, 800, 800, &FLORA);
    assert!(count(&big, Block::Bush) > 0);
    let part = gen(seed, -40, -60, 90, 80, &FLORA);
    for z in part.z0..part.z0 + 80 {
        for x in part.x0..part.x0 + 90 {
            for y in 0..HEIGHT {
                assert_eq!(at(&big, x, y, z), at(&part, x, y, z), "{x},{y},{z}");
            }
        }
    }
    for seed in [12345u64, 9, 42] {
        let off = gen(seed, -150, -150, 300, 300, &[]);
        assert_eq!(count(&off, Block::Bush), 0);
    }
}

#[test]
fn rainforest_plants_stand_on_grass_in_rainforest_and_are_region_independent() {
    let (lf, bl) = (Block::LargeFern as u8, Block::Broadleaf as u8);
    let (mut nlf, mut nbl) = (0, 0);
    for seed in [12345u64, 9, 42] {
        let (x0, z0, w, d) = (-512, -512, 1024, 1024);
        let r = gen(seed, x0, z0, w, d, &FLORA);
        let g = Generator::new(seed);
        for z in z0..z0 + d as i32 {
            for x in x0..x0 + w as i32 {
                for y in 1..HEIGHT {
                    let b = at(&r, x, y, z);
                    if b != lf && b != bl {
                        continue;
                    }
                    let (t, h) = g.climate.at(x, z, &[], &[]);
                    assert_eq!(biome_for(t, h), Biome::Rainforest, "{x},{z}");
                    assert_eq!(at(&r, x, y - 1, z), Block::Grass as u8, "{x},{y},{z}");
                    if b == lf {
                        nlf += 1;
                    } else {
                        nbl += 1;
                    }
                }
            }
        }
        let part = gen(seed, -40, -60, 90, 80, &FLORA);
        for z in part.z0..part.z0 + 80 {
            for x in part.x0..part.x0 + 90 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&r, x, y, z), at(&part, x, y, z), "{x},{y},{z}");
                }
            }
        }
        let off = gen(seed, x0, z0, w, d, &[]);
        assert_eq!(
            count(&off, Block::LargeFern) + count(&off, Block::Broadleaf),
            0
        );
    }
    assert!(nlf > 0 && nbl > 0, "{nlf} large ferns, {nbl} broadleaf");
}

#[test]
fn snow_stays_plants_survive_snow_and_other_cross_plants_do_not() {
    let mut t = cold(FloraShape::Cross, Block::Daisy, None);
    t[0].snow_stays = true;
    let (snow, held) = snow_cells(&t);
    assert!(held > 50, "{snow} {held}");
}

#[test]
fn frost_grass_is_tundra_and_ice_desert_only_and_region_independent() {
    let fg = Block::FrostGrass as u8;
    let mut n = 0;
    for seed in [12345u64, 9, 42] {
        let (x0, z0, w, d) = (-512, -512, 1024, 1024);
        let r = gen(seed, x0, z0, w, d, &FLORA);
        let g = Generator::new(seed);
        for z in z0..z0 + d as i32 {
            for x in x0..x0 + w as i32 {
                for y in 1..HEIGHT {
                    if at(&r, x, y, z) != fg {
                        continue;
                    }
                    let (t, h) = g.climate.at(x, z, &[], &[]);
                    let b = biome_for(t, h);
                    assert!(matches!(b, Biome::Tundra | Biome::IceDesert), "{x},{z}");
                    assert_ne!(at(&r, x, y, z), Block::SnowLayer as u8);
                    n += 1;
                }
            }
        }
        let part = gen(seed, -40, -60, 90, 80, &FLORA);
        for z in part.z0..part.z0 + 80 {
            for x in part.x0..part.x0 + 90 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&r, x, y, z), at(&part, x, y, z), "{x},{y},{z}");
                }
            }
        }
        assert_eq!(count(&gen(seed, x0, z0, w, d, &[]), Block::FrostGrass), 0);
    }
    assert!(n > 0, "no frost grass");
}

#[test]
fn cactus_flowers_sit_on_cactus_tops_in_desert_and_are_region_independent() {
    let cf = Block::CactusFlower as u8;
    let mut n = 0;
    for seed in [12345u64, 9, 42] {
        let (x0, z0, w, d) = (-512, -512, 1024, 1024);
        let r = gen(seed, x0, z0, w, d, &FLORA);
        let g = Generator::new(seed);
        for z in z0..z0 + d as i32 {
            for x in x0..x0 + w as i32 {
                for y in 1..HEIGHT {
                    if at(&r, x, y, z) != cf {
                        continue;
                    }
                    let (t, h) = g.climate.at(x, z, &[], &[]);
                    assert_eq!(biome_for(t, h), Biome::Desert, "{x},{z}");
                    assert_eq!(at(&r, x, y - 1, z), Block::Cactus as u8, "{x},{y},{z}");
                    assert_eq!(at(&r, x, y + 1, z), crate::AIR, "{x},{y},{z}");
                    n += 1;
                }
            }
        }
        let part = gen(seed, -40, -60, 90, 80, &FLORA);
        let off = gen(seed, x0, z0, w, d, &[]);
        assert_eq!(count(&off, Block::CactusFlower), 0);
        for z in part.z0..part.z0 + 80 {
            for x in part.x0..part.x0 + 90 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&r, x, y, z), at(&part, x, y, z), "{x},{y},{z}");
                    // Cactus blocks are the same with flora off.
                    assert_eq!(
                        at(&r, x, y, z) == Block::Cactus as u8,
                        at(&off, x, y, z) == Block::Cactus as u8,
                        "{x},{y},{z}"
                    );
                }
            }
        }
    }
    assert!(n > 0, "no cactus flowers");
}

#[test]
fn tulips_grow_in_seasonal_forest_in_same_colour_drifts() {
    let tulips = [
        Block::TulipRed as u8,
        Block::TulipSalmon as u8,
        Block::TulipYellow as u8,
    ];
    let (mut pairs, mut same, mut n) = (0u64, 0u64, [0u64; 3]);
    for seed in [12345u64, 9, 42] {
        let (x0, z0, w, d) = (-512, -512, 1024, 1024);
        let r = gen(seed, x0, z0, w, d, &FLORA);
        let g = Generator::new(seed);
        let mut found = Vec::new();
        for z in z0..z0 + d as i32 {
            for x in x0..x0 + w as i32 {
                for y in 1..HEIGHT {
                    let b = at(&r, x, y, z);
                    if let Some(k) = tulips.iter().position(|&t| t == b) {
                        let (t, h) = g.climate.at(x, z, &[], &[]);
                        assert_eq!(biome_for(t, h), Biome::SeasonalForest, "{x},{z}");
                        assert_eq!(at(&r, x, y - 1, z), Block::Grass as u8, "{x},{y},{z}");
                        found.push((x, z, k));
                        n[k] += 1;
                    }
                }
            }
        }
        // Pairs of tulips within 8 columns: same colour vs. what the
        // colour mix alone would give for independent picks.
        for (i, a) in found.iter().enumerate() {
            for b in &found[i + 1..] {
                if (b.0 - a.0).abs() <= 8 && (b.1 - a.1).abs() <= 8 {
                    pairs += 1;
                    same += (a.2 == b.2) as u64;
                }
            }
        }
        let part = gen(seed, -40, -60, 90, 80, &FLORA);
        for z in part.z0..part.z0 + 80 {
            for x in part.x0..part.x0 + 90 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&r, x, y, z), at(&part, x, y, z), "{x},{y},{z}");
                }
            }
        }
        let off = gen(seed, x0, z0, w, d, &[]);
        assert!(tulips.iter().all(|&t| !off.blocks.contains(&t)));
    }
    let total: u64 = n.iter().sum();
    assert!(n.iter().all(|&c| c > 0), "{n:?}");
    let tot = total as f64;
    let chance: f64 = n.iter().map(|&c| (c as f64 / tot).powi(2)).sum();
    let rate = same as f64 / pairs as f64;
    assert!(
        pairs > 100 && rate > chance + 0.1,
        "{rate} vs {chance} over {pairs}"
    );
}

#[test]
fn dune_grass_is_on_coastal_sand_in_warm_biomes_and_region_independent() {
    let (lo, up) = (Block::DuneGrassLower as u8, Block::DuneGrassUpper as u8);
    let mut n = 0;
    for seed in [12345u64, 9, 42] {
        let (x0, z0, w, d) = (-512, -512, 1024, 1024);
        let r = gen(seed, x0, z0, w, d, &FLORA);
        let g = Generator::new(seed);
        for z in z0..z0 + d as i32 {
            for x in x0..x0 + w as i32 {
                for y in 1..HEIGHT - 1 {
                    let b = at(&r, x, y, z);
                    if b == lo {
                        let (t, h) = g.climate.at(x, z, &[], &[]);
                        let biome = biome_for(t, h);
                        assert!(
                            !matches!(biome, Biome::Taiga | Biome::Tundra | Biome::IceDesert),
                            "{x},{z}"
                        );
                        assert_eq!(at(&r, x, y + 1, z), up, "{x},{y},{z}");
                        assert_eq!(at(&r, x, y - 1, z), Block::Sand as u8);
                        assert!((62..=66).contains(&((y - 1) as i32)), "{x},{y},{z}");
                        n += 1;
                    } else if b == up {
                        assert_eq!(at(&r, x, y - 1, z), lo, "{x},{y},{z}");
                    }
                }
            }
        }
        let part = gen(seed, -40, -60, 90, 80, &FLORA);
        for z in part.z0..part.z0 + 80 {
            for x in part.x0..part.x0 + 90 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&r, x, y, z), at(&part, x, y, z), "{x},{y},{z}");
                }
            }
        }
        assert_eq!(
            count(&gen(seed, x0, z0, w, d, &[]), Block::DuneGrassLower),
            0
        );
    }
    assert!(n > 0, "no dune grass");
}

/// A Plains and Forest daisy with `niche`, its chance so high that every
/// column where the niche response is above zero gets one.
macro_rules! niche_entry {
    ($niche:expr) => {
        [Flora {
            name: "n",
            block: Block::Daisy,
            upper: None,
            shape: FloraShape::Cross,
            ground: &[Block::Grass],
            tint: FloraTint::None,
            cluster_scale: 8.0,
            salt: 11,
            requires: None,
            snow_stays: false,
            biomes: &[(Biome::Plains, 1e9), (Biome::Forest, 1e9)],
            niche: $niche,
        }]
    };
}

/// Hand-built Plains columns along x, one per `(ground y, soil depth)`:
/// stone, then dirt, with grass as the top soil block. Which got a daisy.
fn niche_placed(cols: &[(i32, i32)], table: &[Flora]) -> Vec<bool> {
    let w = cols.len();
    let mut blocks = vec![0u8; w * HEIGHT];
    let biome = vec![Biome::Plains as u8; w];
    let mut world = World::for_test(&mut blocks, w as i32, 1).with_biome(&biome);
    for (x, &(g, soil)) in cols.iter().enumerate() {
        for y in 0..=g {
            let b = match y {
                _ if y == g => Block::Grass,
                _ if y > g - soil => Block::Dirt,
                _ => Block::Stone,
            };
            world.set(x as i32, y, 0, b as u8);
        }
    }
    place(&mut world, 12345, (0, 0, w as i32, 1), table);
    let daisy =
        |(x, &(g, _)): (usize, &(i32, i32))| world.get(x as i32, g + 1, 0) == Block::Daisy as u8;
    cols.iter().enumerate().map(daisy).collect()
}

#[test]
fn niche_places_where_it_holds_and_not_where_it_fails() {
    let sea = SEA_LEVEL;
    let height = niche_entry!(&[(NicheLayer::HeightAboveSea, 2.0, 4.0)]);
    let cols = [
        (sea + 3, 3),
        (sea + 2, 3),
        (sea + 4, 3),
        (sea + 20, 3),
        (sea - 4, 3),
    ];
    assert_eq!(
        niche_placed(&cols, &height),
        [true, true, true, false, false]
    );
    let soil = niche_entry!(&[(NicheLayer::SoilDepth, 6.0, 8.0)]);
    let cols = [(70, 7), (70, 6), (70, 8), (70, 1), (70, 20)];
    assert_eq!(niche_placed(&cols, &soil), [true, true, true, false, false]);
    let both = niche_entry!(&[
        (NicheLayer::HeightAboveSea, 2.0, 4.0),
        (NicheLayer::SoilDepth, 6.0, 8.0)
    ]);
    let cols = [(sea + 3, 7), (sea + 3, 1), (sea + 20, 7)];
    assert_eq!(niche_placed(&cols, &both), [true, false, false]);
    assert_eq!(niche_placed(&cols, &niche_entry!(&[])), [true, true, true]);
}

/// Which of the columns `x = 1, z in zs` got a daisy on a hand-built
/// 3-wide Plains ridge whose grass height along z is `RIDGE`, rising
/// towards +z (the sun) and falling after its peak at z = 7. An oak trunk
/// stands at (0, 1) with leaves over it and a daisy at (1, 0), which must
/// not tilt (1, 1).
fn ridge_placed(zs: &[i32], table: &[Flora]) -> Vec<bool> {
    const RIDGE: [i32; 14] = [70, 70, 70, 70, 71, 72, 73, 74, 73, 72, 71, 70, 70, 70];
    let (w, d) = (3, RIDGE.len());
    let mut blocks = vec![0u8; w * d * HEIGHT];
    let biome = vec![Biome::Plains as u8; w * d];
    let mut world = World::for_test(&mut blocks, w as i32, d as i32).with_biome(&biome);
    for (z, &g) in RIDGE.iter().enumerate() {
        for x in 0..w as i32 {
            for y in 0..=g {
                let b = if y == g { Block::Grass } else { Block::Stone };
                world.set(x, y, z as i32, b as u8);
            }
        }
    }
    for y in 71..75 {
        world.set(0, y, 1, Block::OakLog as u8);
    }
    for z in 0..3 {
        world.set(0, 75, z, Block::OakLeaves as u8);
    }
    world.set(1, 71, 0, Block::Daisy as u8);
    place(&mut world, 12345, (0, 0, w as i32, d as i32), table);
    let daisy = |&z: &i32| world.get(1, RIDGE[z as usize] + 1, z) == Block::Daisy as u8;
    zs.iter().map(daisy).collect()
}

#[test]
fn slope_and_sun_facing_place_only_where_they_hold() {
    assert_eq!(
        crate::flora::FLORA_SUN,
        (0.0, 1.0),
        "the ridge assumes the sun at +z"
    );
    assert_eq!(crate::flora::FLORA_SUN_FACING_FLAT, 0.5);
    // Flat ground, the ramp up towards the sun, the peak, the ramp down.
    let zs = [1, 5, 7, 10];
    let p = |niche: &'static [(NicheLayer, f64, f64)]| ridge_placed(&zs, &niche_entry!(niche));
    assert_eq!(
        p(&[(NicheLayer::Slope, 0.8, 2.0)]),
        [false, true, false, true]
    );
    assert_eq!(
        p(&[(NicheLayer::Slope, 0.0, 0.1)]),
        [true, false, true, false]
    );
    // Facing the sun: the downhill side of the ramp falls towards +z.
    assert_eq!(
        p(&[(NicheLayer::SunFacing, 0.9, 1.0)]),
        [false, false, false, true]
    );
    assert_eq!(
        p(&[(NicheLayer::SunFacing, 0.0, 0.1)]),
        [false, true, false, false]
    );
    assert_eq!(
        p(&[(NicheLayer::SunFacing, 0.5, 0.5)]),
        [true, false, true, false]
    );
}

#[test]
fn niche_entry_is_independent_of_region() {
    region_independent(&niche_entry!(&[
        (NicheLayer::HeightAboveSea, 2.0, 8.0),
        (NicheLayer::SoilDepth, 2.0, 3.0)
    ]));
    region_independent(&niche_entry!(&[
        (NicheLayer::Slope, 0.5, 2.0),
        (NicheLayer::SunFacing, 0.6, 1.0)
    ]));
}

fn region_independent(table: &[Flora]) {
    let seed = 12345;
    let big = gen(seed, -64, -64, 192, 192, table);
    let all = gen(seed, -64, -64, 192, 192, &niche_entry!(&[]));
    let n = count(&big, Block::Daisy);
    assert!(n > 0 && n < count(&all, Block::Daisy), "{n}");
    for (ox, oz, w, d) in [(-37, 11, 41, 29), (0, 0, 17, 63), (60, -50, 33, 33)] {
        let small = gen(seed, ox, oz, w, d, table);
        for z in oz..oz + d as i32 {
            for x in ox..ox + w as i32 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&small, x, y, z), at(&big, x, y, z), "{x},{y},{z}");
                }
            }
        }
    }
}

#[test]
fn entries_without_a_niche_match_entries_whose_niche_always_holds() {
    static WIDE: [(NicheLayer, f64, f64); 2] = [
        (NicheLayer::HeightAboveSea, -1000.0, 1000.0),
        (NicheLayer::SoilDepth, 0.0, 1000.0),
    ];
    assert!(FLORA.iter().all(|f| f.niche.is_empty()));
    let wide: Vec<Flora> = FLORA.iter().map(|f| Flora { niche: &WIDE, ..*f }).collect();
    for seed in [12345u64, 9] {
        let a = gen(seed, -50, 30, 120, 90, &FLORA);
        assert!(
            a.blocks == gen(seed, -50, 30, 120, 90, &wide).blocks,
            "seed {seed}"
        );
        assert!(a.blocks == crate::generate_decorated(seed, -50, 30, 120, 90).blocks);
    }
}

/// Hand-built Plains grass at y 70 over 13 by 5 columns, with a leaf roof at
/// y 76 over x >= 6. Light at x 0, 6 and 10 of the middle row, and which of
/// them got a daisy.
fn light_placed(table: &[Flora]) -> ([f64; 3], [bool; 3]) {
    let (w, d) = (13, 5);
    let mut blocks = vec![0u8; w * d * HEIGHT];
    let biome = vec![Biome::Plains as u8; w * d];
    let mut world = World::for_test(&mut blocks, w as i32, d as i32).with_biome(&biome);
    for z in 0..d as i32 {
        for x in 0..w as i32 {
            for y in 0..70 {
                world.set(x, y, z, Block::Dirt as u8);
            }
            world.set(x, 70, z, Block::Grass as u8);
            if x >= 6 {
                world.set(x, 76, z, Block::OakLeaves as u8);
            }
        }
    }
    let xs = [0, 6, 10];
    let lit = xs.map(|x| light(&world, x, 70, 2));
    place(&mut world, 12345, (0, 0, w as i32, d as i32), table);
    (lit, xs.map(|x| world.get(x, 71, 2) == Block::Daisy as u8))
}

#[test]
fn light_is_lower_under_leaves_and_its_niche_places_by_it() {
    let (lit, _) = light_placed(&[]);
    // At x 6 the 4 cone columns with x < 6 are open, of 13.
    assert_eq!(lit, [1.0, 4.0 / 13.0, 0.0]);
    // Under leaves the surface is the roof, so a shade plant needs `canopy`.
    let [shade] = niche_entry!(&[(NicheLayer::Light, 0.0, 0.2)]);
    let shade = Flora {
        requires: Some(FloraRequires::Canopy),
        ..shade
    };
    assert_eq!(light_placed(&[shade]).1, [false, false, true]);
    let sun = niche_entry!(&[(NicheLayer::Light, 0.8, 1.0)]);
    assert_eq!(light_placed(&sun).1, [true, false, false]);
}

#[test]
fn light_cone_and_tree_reach_stay_inside_the_decoration_margin() {
    // A tree writes at most 7 blocks out of its decoration cell (decorate.rs).
    const TREE_REACH: i32 = 7;
    const {
        assert!(FLORA_LIGHT_RADIUS >= 0);
        assert!(FLORA_LIGHT_RADIUS + TREE_REACH <= MARGIN_CELLS * 16);
        assert!(FLORA_LIGHT_RADIUS <= READ_REACH);
    }
}

#[test]
fn light_niche_entry_is_independent_of_region() {
    let seed = 12345;
    let table = niche_entry!(&[(NicheLayer::Light, 0.0, 0.6)]);
    let big = gen(seed, -64, -64, 192, 192, &table);
    let all = gen(seed, -64, -64, 192, 192, &niche_entry!(&[]));
    let n = count(&big, Block::Daisy);
    assert!(n > 0 && n < count(&all, Block::Daisy), "{n}");
    for (ox, oz, w, d) in [(-37, 11, 41, 29), (0, 0, 17, 63), (60, -50, 33, 33)] {
        let small = gen(seed, ox, oz, w, d, &table);
        for z in oz..oz + d as i32 {
            for x in ox..ox + w as i32 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&small, x, y, z), at(&big, x, y, z), "{x},{y},{z}");
                }
            }
        }
    }
}

/// Hand-built Plains grass at y 70 over 75 by 33 columns, with a pit down to
/// y 66 around (16, 16) and a knoll up to y 74 around (58, 16), both of radius
/// 2. Hollow at the pit, a flat column and the knoll, and which got a daisy.
fn hollow_placed(table: &[Flora]) -> ([f64; 3], [bool; 3]) {
    let (w, d) = (75, 33);
    let mut blocks = vec![0u8; w * d * HEIGHT];
    let biome = vec![Biome::Plains as u8; w * d];
    let mut world = World::for_test(&mut blocks, w as i32, d as i32).with_biome(&biome);
    let near = |x: i32, z: i32, cx: i32| (x - cx).pow(2) + (z - 16).pow(2) <= 4;
    for z in 0..d as i32 {
        for x in 0..w as i32 {
            let g = match () {
                _ if near(x, z, 16) => 66,
                _ if near(x, z, 58) => 74,
                _ => 70,
            };
            for y in 0..g {
                world.set(x, y, z, Block::Dirt as u8);
            }
            world.set(x, g, z, Block::Grass as u8);
        }
    }
    let xs = [16, 37, 58];
    let h = xs.map(|x| hollow(&world, x, 16));
    place(&mut world, 12345, (0, 0, w as i32, d as i32), table);
    let top = [67, 71, 75];
    let daisy = [0, 1, 2].map(|i| world.get(xs[i], top[i], 16) == Block::Daisy as u8);
    (h, daisy)
}

#[test]
fn a_pit_scores_lower_than_a_knoll_and_a_hollow_niche_places_by_it() {
    let (h, _) = hollow_placed(&[]);
    assert_eq!(h, [-4.0, 0.0, 4.0]);
    let pit = niche_entry!(&[(NicheLayer::Hollow, -10.0, -2.0)]);
    assert_eq!(hollow_placed(&pit).1, [true, false, false]);
    let ridge = niche_entry!(&[(NicheLayer::Hollow, 2.0, 10.0)]);
    assert_eq!(hollow_placed(&ridge).1, [false, false, true]);
}

#[test]
fn hollow_rings_stay_inside_the_decoration_margin() {
    const {
        let mut i = 0;
        while i < FLORA_HOLLOW_RADII.len() {
            assert!(FLORA_HOLLOW_RADII[i] > 0);
            assert!(FLORA_HOLLOW_RADII[i] <= MARGIN_CELLS * 16);
            i += 1;
        }
    }
}

#[test]
fn hollow_niche_entry_is_independent_of_region() {
    let seed = 12345;
    let table = niche_entry!(&[(NicheLayer::Hollow, -40.0, -0.5)]);
    let big = gen(seed, -64, -64, 192, 192, &table);
    let all = gen(seed, -64, -64, 192, 192, &niche_entry!(&[]));
    let n = count(&big, Block::Daisy);
    assert!(n > 0 && n < count(&all, Block::Daisy), "{n}");
    for (ox, oz, w, d) in [(-37, 11, 41, 29), (0, 0, 17, 63), (60, -50, 33, 33)] {
        let small = gen(seed, ox, oz, w, d, &table);
        for z in oz..oz + d as i32 {
            for x in ox..ox + w as i32 {
                for y in 0..HEIGHT {
                    assert_eq!(at(&small, x, y, z), at(&big, x, y, z), "{x},{y},{z}");
                }
            }
        }
    }
}

/// Hand-built Plains grass at y 70 over 30 by 5 columns, with a leaf roof at
/// y 76 over x < 4. Edge at x 2, 4, 7 and 20 of the middle row, and which of
/// them got a daisy.
fn edge_placed(table: &[Flora]) -> ([f64; 4], [bool; 4]) {
    let (w, d) = (30, 5);
    let mut blocks = vec![0u8; w * d * HEIGHT];
    let biome = vec![Biome::Plains as u8; w * d];
    let mut world = World::for_test(&mut blocks, w as i32, d as i32).with_biome(&biome);
    for z in 0..d as i32 {
        for x in 0..w as i32 {
            for y in 0..70 {
                world.set(x, y, z, Block::Dirt as u8);
            }
            world.set(x, 70, z, Block::Grass as u8);
            if x < 4 {
                world.set(x, 76, z, Block::OakLeaves as u8);
            }
        }
    }
    let xs = [2, 4, 7, 20];
    let e = xs.map(|x| edge(&world, x, 70, 2));
    place(&mut world, 12345, (0, 0, w as i32, d as i32), table);
    (e, xs.map(|x| world.get(x, 71, 2) == Block::Daisy as u8))
}

#[test]
fn edge_is_the_distance_to_leaves_and_its_niche_places_by_it() {
    assert_eq!(FLORA_EDGE_RADIUS, 8, "x 20 assumes the cap is under 17");
    let (e, _) = edge_placed(&[]);
    assert_eq!(e, [0.0, 1.0, 4.0, 8.0]);
    let band = niche_entry!(&[(NicheLayer::Edge, 1.0, 6.0)]);
    assert_eq!(edge_placed(&band).1, [false, true, true, false]);
    let far = niche_entry!(&[(NicheLayer::Edge, 8.0, 8.0)]);
    assert_eq!(edge_placed(&far).1, [false, false, false, true]);
}

#[test]
fn edge_search_and_tree_reach_stay_inside_the_decoration_margin() {
    // A tree writes at most 7 blocks out of its decoration cell (decorate.rs).
    const TREE_REACH: i32 = 7;
    const {
        assert!(FLORA_EDGE_RADIUS > 0);
        assert!(FLORA_EDGE_RADIUS + TREE_REACH <= MARGIN_CELLS * 16);
        assert!(FLORA_EDGE_RADIUS <= READ_REACH);
    }
}

#[test]
fn edge_niche_entry_is_independent_of_region() {
    region_independent(&niche_entry!(&[(NicheLayer::Edge, 1.0, 6.0)]));
}

/// Hand-built grass at y 70 over 80 by 7 columns, with water (surface at y 63)
/// from x 40 on, except a land spit at x 66..70 across rows z 0..3. With the
/// wind from +x the ray runs towards larger x: x 39 at z 5 faces open water,
/// x 39 at z 1 has the spit across its ray (a cove), x 10 is inland.
fn exposure_world(blocks: &mut [u8]) -> World<'_> {
    let (w, d) = (80, 7);
    let mut world = World::for_test(blocks, w, d);
    for z in 0..d {
        for x in 0..w {
            let wet = x >= 40 && !((66..70).contains(&x) && z < 3);
            let g = if wet { 62 } else { 70 };
            for y in 0..g {
                world.set(x, y, z, Block::Dirt as u8);
            }
            if wet {
                world.set(x, 63, z, Block::Water as u8);
            } else {
                world.set(x, g, z, Block::Grass as u8);
            }
        }
    }
    world
}

#[test]
fn exposure_scores_a_shore_facing_water_above_a_cove_or_inland() {
    assert_eq!(FLORA_WIND, (1.0, 0.0), "the layout assumes wind from +x");
    assert_eq!(FLORA_EXPOSURE_CAP, 48);
    let mut blocks = vec![0u8; 80 * 7 * HEIGHT];
    let world = exposure_world(&mut blocks);
    let shore = exposure(&world, 39, 5);
    let cove = exposure(&world, 39, 1);
    let inland = exposure(&world, 10, 5);
    assert_eq!(shore, 40.0 / 48.0);
    assert_eq!(cove, 26.0 / 48.0);
    assert_eq!(inland, 0.0);
    assert!(shore > cove && cove > inland);
}

#[test]
fn exposure_is_capped_and_in_unit_range() {
    let mut blocks = vec![0u8; 80 * 7 * HEIGHT];
    let world = exposure_world(&mut blocks);
    assert_eq!(exposure(&world, 0, 5), 0.0);
    assert!(exposure(&world, 39, 5) <= 1.0);
}

#[test]
fn exposure_ray_stays_inside_the_decoration_margin() {
    const {
        assert!(FLORA_EXPOSURE_CAP > 0);
        assert!(FLORA_EXPOSURE_CAP <= MARGIN_CELLS * 16);
    }
}

#[test]
fn exposure_niche_is_accepted_and_independent_of_region() {
    region_independent(&niche_entry!(&[(NicheLayer::Exposure, 0.2, 1.0)]));
}
