use crate::scoring_extra::*;
use crate::ScoringConfig;
use terrain::{Block, Region, HEIGHT, SEA_LEVEL};

const W: u32 = 60;
const D: u32 = 40;

fn sea() -> Region {
    let mut blocks = vec![Block::Air as u8; (W * D) as usize * HEIGHT];
    for col in blocks.chunks_mut(HEIGHT) {
        for (y, b) in col.iter_mut().enumerate().take(SEA_LEVEL as usize) {
            *b = if y < 50 { Block::Stone } else { Block::Water } as u8;
        }
    }
    Region {
        blocks,
        width: W,
        depth: D,
        height: HEIGHT as u32,
        x0: 0,
        z0: 0,
    }
}

/// Stone columns up to `top` over the rectangle.
fn rect(r: &mut Region, x: std::ops::Range<u32>, z: std::ops::Range<u32>, top: usize) {
    for zz in z {
        for xx in x.clone() {
            let base = ((zz * r.width + xx) as usize) * HEIGHT;
            for y in 50..=top {
                r.blocks[base + y] = Block::Stone as u8;
            }
        }
    }
}

/// Anchorage island with a tall ridge on its east side, one island in the
/// open to the north-east, one hidden behind the ridge.
fn world() -> Region {
    let mut r = sea();
    let s = SEA_LEVEL as usize;
    rect(&mut r, 5..31, 14..40, s);
    rect(&mut r, 28..29, 14..40, 100);
    rect(&mut r, 40..45, 2..7, s);
    rect(&mut r, 40..47, 30..37, s);
    r
}

const ROUTE: [[f64; 2]; 2] = [[2.0, 20.0], [12.0, 20.0]];
const CENTRES: [[i32; 2]; 1] = [[10, 20]];

#[test]
fn counts_only_visible_non_anchorage_islands() {
    let cfg = ScoringConfig::shipped();
    let e = extra_islands(&world(), &CENTRES, &ROUTE, &cfg);
    assert_eq!(e.count, 1);
    assert!((e.term - 1.0 / cfg.cap_extra_islands).abs() < 1e-9);
    assert!(e.term > 0.0 && e.term < 1.0);
}

#[test]
fn deterministic() {
    let cfg = ScoringConfig::shipped();
    let r = world();
    assert_eq!(
        extra_islands(&r, &CENTRES, &ROUTE, &cfg),
        extra_islands(&r, &CENTRES, &ROUTE, &cfg)
    );
}

#[test]
fn term_is_capped_and_empty_route_scores_zero() {
    let cfg = ScoringConfig {
        cap_extra_islands: 0.5,
        ..ScoringConfig::shipped()
    };
    assert_eq!(extra_islands(&world(), &CENTRES, &ROUTE, &cfg).term, 1.0);
    let none = extra_islands(&world(), &CENTRES, &[], &cfg);
    assert_eq!((none.count, none.term), (0, 0.0));
}
