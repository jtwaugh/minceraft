use super::*;
use terrain::SEA_LEVEL;

const NEAR: f64 = 30.0;
const RAMP: f64 = 10.0;
const WIDTH: f64 = 20.0;
const FADE: f64 = 10.0;
const HEIGHT: f64 = 12.0;
const WOBBLE: f64 = 200.0;
const CHANNEL_REACH: f64 = 12.0 + 6.0;

fn coast(points: Vec<(f64, f64)>) -> DensityTerm {
    DensityTerm::Coast {
        points,
        seed: 0,
        near: (NEAR, NEAR),
        ramp: (RAMP, RAMP),
        height: (HEIGHT, HEIGHT),
        width: WIDTH,
        fade: FADE,
        wobble_length: WOBBLE,
    }
}

fn varying(seed: u64, points: Vec<(f64, f64)>) -> DensityTerm {
    DensityTerm::Coast {
        points,
        seed,
        near: (30.0, 40.0),
        ramp: (8.0, 24.0),
        height: (6.0, 14.0),
        width: WIDTH,
        fade: FADE,
        wobble_length: WOBBLE,
    }
}

#[test]
fn parameters_vary_inside_their_bounds() {
    let pts = vec![(0.0, 0.0), (3000.0, 0.0)];
    let c = CoastEval::new(&varying(5, pts.clone())).unwrap();
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for x in (0..3000).step_by(7) {
        let (n, r, h) = c.params_at(x, 50).unwrap();
        assert!((30.0..=40.0).contains(&n) && (8.0..=24.0).contains(&r));
        assert!((6.0..=14.0).contains(&h));
        lo = (n.min(lo.0), r.min(lo.1));
        hi = (n.max(hi.0), r.max(hi.1));
    }
    assert!(hi.0 - lo.0 > 3.0 && hi.1 - lo.1 > 8.0, "{lo:?} {hi:?}");
    let other = CoastEval::new(&varying(6, pts)).unwrap();
    let w = |e: &CoastEval| {
        (0..3000)
            .step_by(50)
            .map(|x| e.weight(x, 45))
            .collect::<Vec<_>>()
    };
    assert_eq!(w(&c), w(&c));
    assert_ne!(w(&c), w(&other));
}

#[test]
fn varying_overlapping_regions_agree() {
    let l = Layout {
        seed: 9,
        density: vec![varying(9, vec![(-100.0, 0.0), (0.0, 0.0), (300.0, 40.0)])],
        ..Layout::default()
    };
    for decorate in [false, true] {
        let a = l.generate_region(-40, -10, 96, 96, decorate);
        let b = l.generate_region(0, 20, 64, 48, decorate);
        for wz in 20..68 {
            for wx in 0..56 {
                for y in 0..terrain::HEIGHT as u32 {
                    let ia = a.index((wx + 40) as u32, y, (wz + 10) as u32);
                    let ib = b.index(wx as u32, y, (wz - 20) as u32);
                    assert_eq!(a.blocks[ia], b.blocks[ib], "({wx},{y},{wz}) {decorate}");
                }
            }
        }
    }
}

fn eval(points: Vec<(f64, f64)>) -> CoastEval {
    CoastEval::new(&coast(points)).unwrap()
}

/// Heading +x: the right of (1, 0) is (0, 1), so +z.
#[test]
fn weight_profile_on_the_right() {
    let c = eval(vec![(-200.0, 0.0), (200.0, 0.0)]);
    for z in [-60, -35, -1, 0, 10, 30] {
        assert_eq!(c.weight(0, z), 0.0, "z={z}");
    }
    assert!(c.weight(0, 35) > 0.0 && c.weight(0, 35) < 1.0);
    for z in 40..=60 {
        assert_eq!(c.weight(0, z), 1.0, "z={z}");
    }
    assert!(c.weight(0, 65) > 0.0 && c.weight(0, 65) < 1.0);
    for z in [70, 80, 500] {
        assert_eq!(c.weight(0, z), 0.0, "z={z}");
    }
}

#[test]
fn bias_moves_base_toward_target() {
    let c = eval(vec![(-200.0, 0.0), (200.0, 0.0)]);
    let t = SEA_LEVEL as f64 + HEIGHT;
    assert_eq!(c.bias(0, 50, 50.0), t - 50.0);
    assert_eq!(c.bias(0, -50, 50.0), 0.0);
    let term = coast(vec![(-200.0, 0.0), (200.0, 0.0)]);
    assert_eq!(term.bias(0, 50, 50.0), t - 50.0);
}

#[test]
fn bends_are_continuous_and_clear_of_the_channel() {
    // A 30 degree left bend and a 30 degree right bend.
    let (s, c) = (30f64.to_radians().sin(), 30f64.to_radians().cos());
    for sign in [1.0, -1.0] {
        let pts = vec![(-200.0, 0.0), (0.0, 0.0), (200.0 * c, sign * 200.0 * s)];
        let ev = eval(pts.clone());
        let mut worst = 0.0f64;
        for x in -150..=150 {
            for z in -150..=150 {
                let w = ev.weight(x, z);
                let d = pts
                    .windows(2)
                    .map(|w| {
                        nearest_on_segment((x as f64, z as f64), w[0], w[1])
                            .0
                            .sqrt()
                    })
                    .fold(f64::INFINITY, f64::min);
                if d <= CHANNEL_REACH {
                    assert_eq!(w, 0.0, "({x},{z}) d={d}");
                }
                for (dx, dz) in [(1, 0), (0, 1)] {
                    worst = worst.max((w - ev.weight(x + dx, z + dz)).abs());
                }
            }
        }
        assert!(worst < 0.2, "sign={sign} neighbour step {worst}");
    }
}

#[test]
fn bounds_cover_the_reach() {
    let l = Layout {
        seed: 3,
        density: vec![coast(vec![(0.0, 0.0), (100.0, 0.0)])],
        ..Layout::default()
    };
    let t = &l.density[0];
    let b = t.bounds().unwrap();
    assert_eq!(b, (-70.0, -70.0, 170.0, 70.0));
}

#[test]
fn overlapping_regions_agree() {
    let l = Layout {
        seed: 9,
        density: vec![coast(vec![(-100.0, 0.0), (0.0, 0.0), (100.0, 40.0)])],
        ..Layout::default()
    };
    for decorate in [false, true] {
        let a = l.generate_region(-40, -10, 96, 96, decorate);
        let b = l.generate_region(0, 20, 64, 48, decorate);
        let mut raised = false;
        for wz in 20..68 {
            for wx in 0..56 {
                for y in 0..terrain::HEIGHT as u32 {
                    let ia = a.index((wx + 40) as u32, y, (wz + 10) as u32);
                    let ib = b.index(wx as u32, y, (wz - 20) as u32);
                    assert_eq!(a.blocks[ia], b.blocks[ib], "({wx},{y},{wz}) {decorate}");
                    raised |= y as i32 >= SEA_LEVEL && a.blocks[ia] != terrain::Block::Air as u8;
                }
            }
        }
        assert!(raised, "no land in the overlap");
    }
}
