use super::*;
use std::collections::BTreeSet;
use terrain::{Region, HEIGHT};

const S: i32 = REGION_SIZE as i32;

#[test]
fn containing_and_origin_round_trip() {
    for x in [
        -3 * S - 1,
        -2 * S,
        -S - 1,
        -S,
        -1,
        0,
        1,
        S - 1,
        S,
        S + 1,
        5 * S,
    ] {
        for z in [-S - 1, -S, -1, 0, S - 1, S] {
            let c = RegionCoord::containing(x, z);
            let (ox, oz) = c.origin();
            assert!((ox..ox + S).contains(&x), "x {x} in {c:?}");
            assert!((oz..oz + S).contains(&z), "z {z} in {c:?}");
            assert_eq!(RegionCoord::containing(ox, oz), c);
            assert_eq!(RegionCoord::containing(ox + S - 1, oz + S - 1), c);
        }
    }
    assert_eq!(RegionCoord::containing(-1, -1), RegionCoord::new(-1, -1));
    assert_eq!(RegionCoord::containing(-S, S), RegionCoord::new(-1, 1));
    assert_eq!(RegionCoord::new(-2, 3).origin(), (-2 * S, 3 * S));
}

fn layout(seed: u64) -> Layout {
    Layout {
        seed,
        density: vec![DensityTerm::RadialBaseHeight {
            center: (0.0, 0.0),
            r_inner: 30.0,
            r_outer: 90.0,
            delta: 40.0,
        }],
        climate: vec![ClimateBias::Radial {
            center: (S as f64, 0.0),
            r_inner: 10.0,
            r_outer: 80.0,
            d_temp: 0.4,
            d_humid: -0.3,
        }],
        exclusions: vec![Exclusion::Circle {
            center: (0.0, 20.0),
            radius: 30.0,
        }],
        ..Layout::default()
    }
}

fn column(r: &Region, x: i32, z: i32) -> &[u8] {
    let i = ((z - r.z0) as usize * r.width as usize + (x - r.x0) as usize) * HEIGHT;
    &r.blocks[i..i + HEIGHT]
}

/// Every column of `part` equals the same world column of `whole`.
fn assert_inside(part: &Region, whole: &Region, what: &str) {
    for z in part.z0..part.z0 + part.depth as i32 {
        for x in part.x0..part.x0 + part.width as i32 {
            assert_eq!(column(part, x, z), column(whole, x, z), "{what} at {x},{z}");
        }
    }
}

#[test]
fn neighbours_agree_on_shared_edge_and_margin() {
    const MARGIN: i32 = 24;
    let l = layout(4242);
    for decorate in [false, true] {
        for (a, b) in [
            (RegionCoord::new(-1, -1), RegionCoord::new(0, -1)),
            (RegionCoord::new(-1, -1), RegionCoord::new(-1, 0)),
        ] {
            let ra = l.generate_layout_region_at(a, decorate).region;
            let rb = l.generate_layout_region_at(b, decorate).region;
            // A region covering both, so the margin either side of the edge
            // can be compared column by column.
            let (x0, z0) = (
                a.origin().0.min(b.origin().0),
                a.origin().1.min(b.origin().1),
            );
            let (w, d) = if a.rx != b.rx { (2 * S, S) } else { (S, 2 * S) };
            let both = l
                .generate_layout_region(x0, z0, w as u32, d as u32, decorate)
                .region;
            let edge_x = if a.rx != b.rx { b.origin().0 } else { i32::MIN };
            let edge_z = if a.rz != b.rz { b.origin().1 } else { i32::MIN };
            for r in [&ra, &rb] {
                for z in r.z0..r.z0 + S {
                    for x in r.x0..r.x0 + S {
                        let near = (x - edge_x).abs() <= MARGIN || (z - edge_z).abs() <= MARGIN;
                        if near {
                            assert_eq!(
                                column(r, x, z),
                                column(&both, x, z),
                                "decorate={decorate} {a:?}/{b:?} at {x},{z}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn three_by_three_any_order_matches_one_large_region() {
    let l = layout(777);
    let decorate = true;
    let origin = RegionCoord::new(-2, -2);
    let big = l
        .generate_layout_region(
            origin.origin().0,
            origin.origin().1,
            3 * REGION_SIZE,
            3 * REGION_SIZE,
            decorate,
        )
        .region;
    let coords: Vec<RegionCoord> = (0..3)
        .flat_map(|dz| (0..3).map(move |dx| RegionCoord::new(-2 + dx, -2 + dz)))
        .collect();
    let row_order: Vec<Region> = coords
        .iter()
        .map(|&c| l.generate_layout_region_at(c, decorate).region)
        .collect();
    // A fixed scrambled order, generated in that order.
    let mut shuffled = vec![None; 9];
    for &i in &[7usize, 2, 5, 0, 8, 3, 6, 1, 4] {
        shuffled[i] = Some(l.generate_layout_region_at(coords[i], decorate).region);
    }
    for (i, row) in row_order.iter().enumerate() {
        let other = shuffled[i].as_ref().unwrap();
        assert_eq!(row.blocks, other.blocks, "region {:?}", coords[i]);
        assert_inside(row, &big, &format!("region {:?}", coords[i]));
    }
}

#[test]
fn raft_straddling_a_border_splits_its_cells() {
    let l = Layout {
        seed: 9,
        structures: vec![StructurePlacement {
            structure: "raft_start".into(),
            origin: [S - 2, 64, 20],
            rotation: 0,
        }],
        ..Layout::default()
    };
    let left = l.generate_layout_region_at(RegionCoord::new(0, 0), false);
    let right = l.generate_layout_region_at(RegionCoord::new(1, 0), false);
    let both = l.generate_layout_region(0, 0, 2 * REGION_SIZE, REGION_SIZE, false);
    assert!(!left.raft_cells.is_empty() && !right.raft_cells.is_empty());
    assert!(left.raft_cells.iter().all(|c| c[0] < S));
    assert!(right.raft_cells.iter().all(|c| c[0] >= S));
    let union: BTreeSet<[i32; 3]> = left
        .raft_cells
        .iter()
        .chain(&right.raft_cells)
        .copied()
        .collect();
    assert_eq!(union.len(), left.raft_cells.len() + right.raft_cells.len());
    assert_eq!(union, both.raft_cells.iter().copied().collect());
}
