use super::*;
use terrain::{generate_region_with_biases, Biases, HEIGHT};

fn exclusions() -> Exclusions {
    Exclusions(vec![
        Exclusion::Circle {
            center: (10.0, 10.0),
            radius: 20.0,
        },
        Exclusion::Corridor {
            points: vec![(-60.0, -40.0), (0.0, -40.0), (30.0, -10.0)],
            half_width: 6.0,
        },
    ])
}

#[test]
fn circle_inside_boundary_outside() {
    let c = Exclusion::Circle {
        center: (0.0, 0.0),
        radius: 5.0,
    };
    assert!(c.protected(0, 0, 0));
    assert!(c.protected(3, 100, 4)); // exactly on the boundary, any y
    assert!(!c.protected(4, 0, 4));
    assert!(!c.protected(6, 0, 0));
}

#[test]
fn corridor_inside_boundary_outside() {
    let c = Exclusion::Corridor {
        points: vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
        half_width: 2.0,
    };
    assert!(c.protected(5, 0, 0));
    assert!(c.protected(5, 0, 2)); // boundary
    assert!(c.protected(5, 0, -2));
    assert!(!c.protected(5, 0, 3));
    assert!(c.protected(-2, 0, 0)); // round cap at the start
    assert!(!c.protected(-3, 0, 0));
    assert!(c.protected(12, 0, 5)); // second segment
    assert!(!c.protected(13, 0, 5));
    assert!(c.protected(11, 0, 11)); // round end cap
    assert!(!c.protected(12, 0, 12));
}

#[test]
fn degenerate_corridors() {
    let none = Exclusion::Corridor {
        points: vec![],
        half_width: 5.0,
    };
    assert!(!none.protected(0, 0, 0));
    let one = Exclusion::Corridor {
        points: vec![(0.0, 0.0)],
        half_width: 2.0,
    };
    assert!(one.protected(2, 0, 0) && !one.protected(3, 0, 0));
}

#[test]
fn zone_has_no_decoration_and_terrain_is_unchanged() {
    let ex = exclusions();
    let mask: &dyn ProtectedMask = &ex;
    let with = Biases {
        protected: Some(mask),
        ..Biases::default()
    };
    let (x0, z0, w, d) = (-70, -60, 140, 120);
    let seed = 777;
    let plain = Biases::default();
    let terrain_only = generate_region_with_biases(seed, x0, z0, w, d, &plain, false);
    let terrain_masked = generate_region_with_biases(seed, x0, z0, w, d, &with, false);
    assert_eq!(terrain_only.blocks, terrain_masked.blocks);
    let free = generate_region_with_biases(seed, x0, z0, w, d, &plain, true);
    let masked = generate_region_with_biases(seed, x0, z0, w, d, &with, true);
    let (mut suppressed, mut kept_outside) = (0, 0);
    for z in 0..d {
        for x in 0..w {
            let i = masked.index(x, 0, z);
            let col = i..i + HEIGHT;
            if mask.protected(x0 + x as i32, 0, z0 + z as i32) {
                assert_eq!(masked.blocks[col.clone()], terrain_only.blocks[col.clone()]);
                if free.blocks[col.clone()] != terrain_only.blocks[col] {
                    suppressed += 1;
                }
            } else if masked.blocks[col.clone()] != terrain_only.blocks[col] {
                kept_outside += 1;
            }
        }
    }
    assert!(suppressed > 0, "zone has no decoration to suppress");
    assert!(kept_outside > 0, "no decoration outside the zone");
}

#[test]
fn overlapping_regions_agree_with_exclusion() {
    let ex = exclusions();
    let b = Biases {
        protected: Some(&ex),
        ..Biases::default()
    };
    let big = generate_region_with_biases(4242, -50, -50, 100, 100, &b, true);
    let small = generate_region_with_biases(4242, -13, -7, 40, 50, &b, true);
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
                "at ({x},{z})"
            );
        }
    }
}
