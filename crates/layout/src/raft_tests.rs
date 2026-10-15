use super::*;
use std::collections::BTreeSet;
use terrain::{Region, HEIGHT};

const ORIGIN: [i32; 3] = [0, 64, 0];

fn with_raft(seed: u64, rotation: u8) -> Layout {
    Layout {
        seed,
        structures: vec![StructurePlacement {
            structure: "raft_start".into(),
            origin: ORIGIN,
            rotation,
        }],
        ..Layout::default()
    }
}

fn at(r: &Region, x: i32, y: i32, z: i32) -> u8 {
    let (lx, lz) = ((x - r.x0) as usize, (z - r.z0) as usize);
    r.blocks[(lz * r.width as usize + lx) * HEIGHT + y as usize]
}

fn mask(l: &LayoutRegion) -> BTreeSet<[i32; 3]> {
    l.raft_cells.iter().copied().collect()
}

#[test]
fn no_placements_is_byte_identical() {
    for seed in [1u64, 42, 12345, 987654321] {
        let l = Layout::new(seed);
        for decorate in [false, true] {
            let a = l.generate_layout_region(-20, -20, 48, 48, decorate);
            let b = generate_region(&l, -20, -20, 48, 48, decorate);
            let c = if decorate {
                terrain::generate_decorated(seed, -20, -20, 48, 48)
            } else {
                terrain::generate_region(seed, -20, -20, 48, 48)
            };
            assert_eq!(a.region.blocks, b.blocks);
            assert_eq!(a.region.blocks, c.blocks);
            assert!(a.raft_cells.is_empty());
        }
    }
}

fn check_stamp(rotation: u8) {
    let s = Structure::raft_start();
    for seed in [7u64, 12345] {
        for decorate in [false, true] {
            let l = with_raft(seed, rotation);
            let out = l.generate_layout_region(-16, -16, 32, 32, decorate);
            let cells = s.cells(ORIGIN, rotation);
            for &(x, y, z, b) in &cells {
                assert_eq!(at(&out.region, x, y, z), b, "cell {x},{y},{z}");
            }
            let want: BTreeSet<[i32; 3]> = s
                .solid_cells(ORIGIN, rotation)
                .into_iter()
                .map(|(x, y, z, _)| [x, y, z])
                .collect();
            assert_eq!(mask(&out), want);
            assert_eq!(out.raft_cells.len(), want.len());
            // Only template cells differ from the unstamped world.
            let base = Layout::new(seed).generate_region(-16, -16, 32, 32, decorate);
            let tpl: BTreeSet<(i32, i32, i32)> = cells.iter().map(|c| (c.0, c.1, c.2)).collect();
            for z in -16..16 {
                for x in -16..16 {
                    for y in 0..HEIGHT as i32 {
                        if !tpl.contains(&(x, y, z)) {
                            assert_eq!(at(&out.region, x, y, z), at(&base, x, y, z));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn raft_start_stamps_template_and_mask() {
    check_stamp(0);
}

#[test]
fn rotation_one_stamps_rotated_cells() {
    check_stamp(1);
    let s = Structure::raft_start();
    assert_ne!(s.cells(ORIGIN, 0), s.cells(ORIGIN, 1));
}

#[test]
fn overlapping_and_clipped_regions_agree() {
    for rotation in [0u8, 1] {
        for decorate in [false, true] {
            let l = with_raft(12345, rotation);
            let full = l.generate_layout_region(-24, -24, 48, 48, decorate);
            // All overlap the raft; the second cuts it in half at x = 0.
            let regions = [(-10, -12, 30, 24), (0, -3, 20, 20), (-24, -2, 24, 9)];
            for (x0, z0, w, d) in regions {
                let part = l.generate_layout_region(x0, z0, w, d, decorate);
                for z in z0..z0 + d as i32 {
                    for x in x0..x0 + w as i32 {
                        for y in 0..HEIGHT as i32 {
                            assert_eq!(
                                at(&part.region, x, y, z),
                                at(&full.region, x, y, z),
                                "{x},{y},{z}"
                            );
                        }
                    }
                }
                let clipped: BTreeSet<[i32; 3]> = mask(&full)
                    .into_iter()
                    .filter(|c| {
                        (x0..x0 + w as i32).contains(&c[0]) && (z0..z0 + d as i32).contains(&c[2])
                    })
                    .collect();
                assert_eq!(mask(&part), clipped);
            }
            let cut = l.generate_layout_region(0, -3, 20, 20, decorate);
            assert!(!cut.raft_cells.is_empty());
            assert!(cut.raft_cells.len() < full.raft_cells.len());
        }
    }
}
