//! Tests for the column-local flora pass (docs/design/biome-flora.md D.2).

use crate::climate::biome_for;
use crate::data::{Biome, Block};
use crate::decorate::generate_decorated_with;
use crate::flora::{Flora, FloraShape, FloraTint};
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
fn shipped_table_only_adds_daisies() {
    for seed in [12345u64, 9, 42] {
        let today = crate::generate_decorated(seed, -50, 30, 120, 90);
        let r = gen(seed, -50, 30, 120, 90, &[]);
        for (a, b) in r.blocks.iter().zip(&today.blocks) {
            assert!(a == b || (*a == Block::Air as u8 && *b == Block::Daisy as u8));
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

#[test]
fn flora_comes_in_patches() {
    let r = gen(12345, -64, -64, 192, 192, &TEST);
    let all = count(&gen(12345, -64, -64, 192, 192, &[]), Block::Daisy);
    assert!(count(&r, Block::Daisy) > all);
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
