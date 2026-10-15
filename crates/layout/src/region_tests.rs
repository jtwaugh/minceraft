use super::*;
use terrain::{
    column_summary, generate_coarse_heightmap, generate_decorated, Block, Region, HEIGHT, SEA_LEVEL,
};

const ISLAND: DensityTerm = DensityTerm::RadialBaseHeight {
    center: (0.0, 0.0),
    r_inner: 24.0,
    r_outer: 64.0,
    delta: 40.0,
};

fn island(seed: u64) -> Layout {
    Layout {
        seed,
        density: vec![ISLAND],
        ..Layout::default()
    }
}

#[test]
fn empty_layout_matches_terrain() {
    for seed in [1u64, 42, 12345, 987654321] {
        let l = Layout::new(seed);
        let a = generate_region(&l, -37, 11, 70, 50, false);
        let b = terrain::generate_region(seed, -37, 11, 70, 50);
        assert_eq!(a.blocks, b.blocks, "terrain seed {seed}");
        let a = generate_region(&l, -37, 11, 70, 50, true);
        let b = generate_decorated(seed, -37, 11, 70, 50);
        assert_eq!(a.blocks, b.blocks, "decorated seed {seed}");
    }
}

#[test]
fn island_centre_is_land_for_95_percent_of_seeds() {
    let land = (1..=200u64)
        .filter(|&seed| {
            let r = island(seed).generate_region(0, 0, 1, 1, false);
            let (top, ty, _, _) = column_summary(&r.blocks[..HEIGHT]);
            top != Block::Water as u8 && top != Block::Ice as u8 && ty as i32 >= SEA_LEVEL
        })
        .count();
    assert!(land >= 190, "only {land}/200 seeds have land at the centre");
}

/// Columns with air above sea level that has solid both above and below.
fn overhang_columns(r: &Region) -> usize {
    (0..r.depth)
        .flat_map(|z| (0..r.width).map(move |x| (x, z)))
        .filter(|&(x, z)| {
            let col = &r.blocks[r.index(x, 0, z)..][..HEIGHT];
            let air = Block::Air as u8;
            let top = (0..HEIGHT).rev().find(|&y| col[y] != air);
            top.is_some_and(|top| {
                (SEA_LEVEL as usize..top)
                    .any(|y| col[y] == air && col[..y].iter().any(|&b| b != air))
            })
        })
        .count()
}

#[test]
fn biased_island_keeps_overhangs() {
    const SEEDS: u64 = 20;
    let (mut island_total, mut base_total, mut coarse_gaps) = (0usize, 0usize, 0u32);
    for seed in 1..=SEEDS {
        island_total += overhang_columns(&island(seed).generate_region(-80, -80, 160, 160, false));
        base_total +=
            overhang_columns(&Layout::new(seed).generate_region(-80, -80, 160, 160, false));
        coarse_gaps += generate_coarse_heightmap(seed, -80, -80, 4, 40, 40)
            .chunks(4)
            .map(|c| c[2] as u32)
            .sum::<u32>();
    }
    eprintln!(
        "overhang columns over {SEEDS} seeds: island {island_total}, baseline {base_total}, coarse gaps {coarse_gaps}"
    );
    assert!(island_total >= N_MIN_OVERHANG, "island has {island_total}");
}

/// About half the unbiased baseline (442 over the same 20 seeds and area;
/// 337 with the island).
const N_MIN_OVERHANG: usize = 200;

fn busy_layout(seed: u64) -> Layout {
    Layout {
        seed,
        density: vec![
            ISLAND,
            DensityTerm::RadialBaseHeight {
                center: (5000.0, 5000.0),
                r_inner: 10.0,
                r_outer: 30.0,
                delta: 30.0,
            },
        ],
        climate: vec![
            ClimateBias::Radial {
                center: (20.0, 20.0),
                r_inner: 10.0,
                r_outer: 60.0,
                d_temp: 0.4,
                d_humid: -0.3,
            },
            ClimateBias::Radial {
                center: (-9000.0, 0.0),
                r_inner: 10.0,
                r_outer: 60.0,
                d_temp: 0.4,
                d_humid: 0.3,
            },
        ],
        exclusions: vec![
            Exclusion::Circle {
                center: (30.0, -30.0),
                radius: 25.0,
            },
            Exclusion::Circle {
                center: (0.0, 7000.0),
                radius: 25.0,
            },
        ],
    }
}

#[test]
fn skipping_terms_does_not_change_output() {
    for decorate in [false, true] {
        for (x0, z0) in [(-64, -64), (200, 150), (-40, 500)] {
            let l = busy_layout(77);
            let a = l.generate_region(x0, z0, 96, 96, decorate);
            let b = l.generate_region_all_terms(x0, z0, 96, 96, decorate);
            assert_eq!(a.blocks, b.blocks, "decorate={decorate} at ({x0},{z0})");
        }
    }
}

#[test]
fn overlapping_regions_agree() {
    let l = busy_layout(4242);
    for decorate in [false, true] {
        let big = l.generate_region(-50, -50, 100, 100, decorate);
        let small = l.generate_region(-13, -7, 40, 50, decorate);
        for z in 0..small.depth {
            for x in 0..small.width {
                let i = small.index(x, 0, z);
                let j = big.index(
                    (small.x0 + x as i32 + 50) as u32,
                    0,
                    (small.z0 + z as i32 + 50) as u32,
                );
                assert_eq!(
                    small.blocks[i..i + HEIGHT],
                    big.blocks[j..j + HEIGHT],
                    "decorate={decorate} at ({x},{z})"
                );
            }
        }
    }
}
