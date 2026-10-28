//! Tests for the column-local flora pass (docs/design/biome-flora.md D.2).

use crate::climate::biome_for;
use crate::data::{Biome, Block};
use crate::decorate::generate_decorated_with;
use crate::flora::{Flora, FloraShape, FloraTint, FLORA};
use crate::{Biases, Generator, Region, HEIGHT};

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
        biomes: &[(Biome::Plains, 0.9), (Biome::Forest, 0.5)],
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
        biomes: &[(Biome::Desert, 0.9), (Biome::Taiga, 0.5)],
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
            biomes: &[(Biome::Plains, $chance), (Biome::Forest, $chance)],
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
        biomes: &[(Biome::Plains, 1e9), (Biome::Forest, 1e9)],
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
        biomes: &[(Biome::Plains, 1e9), (Biome::Forest, 1e9)],
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
    biomes: &[(Biome::Plains, 1e9), (Biome::Forest, 1e9)],
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
        requires: None,
        biomes: &COLD,
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
