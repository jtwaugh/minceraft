use super::*;

fn t() -> Thresholds {
    Thresholds::shipped()
}

/// A start seed known to fail its first attempt (see `known_failing_seed`).
fn known_failing_seed() -> u64 {
    (1..1000u64)
        .find(|&s| {
            find_v0_level(
                s,
                &Thresholds {
                    max_attempts: 1,
                    ..t()
                },
            )
            .is_err()
        })
        .expect("some seed fails")
}

#[test]
fn cap_comes_from_config() {
    assert_eq!(t().max_attempts, 64);
}

#[test]
fn same_start_seed_same_result() {
    for start in [1u64, 7, 12345] {
        assert_eq!(find_v0_level(start, &t()), find_v0_level(start, &t()));
    }
}

#[test]
fn cap_of_one_on_a_failing_seed_names_the_failing_checks() {
    let start = known_failing_seed();
    let capped = Thresholds {
        max_attempts: 1,
        ..t()
    };
    let err = find_v0_level(start, &capped).unwrap_err();
    assert_eq!(
        (err.start_seed, err.last_seed, err.attempts),
        (start, start, 1)
    );
    let failing = err.failing_checks();
    assert!(!failing.is_empty());
    assert!(failing.iter().all(|c| !c.passed));
    let msg = err.to_string();
    for c in failing {
        assert!(msg.contains(c.name), "{msg} should name {}", c.name);
    }
}

#[test]
fn retry_advances_the_seed() {
    let start = known_failing_seed();
    let found = find_v0_level(start, &t()).unwrap();
    assert!(found.attempts > 1);
    assert_eq!(found.seed, start + (found.attempts - 1) as u64);
    assert!(found.report.passed());
    assert_eq!(found.layout.seed, found.seed);
    assert_eq!(found.level, default_v0_level(found.seed));
}

#[test]
fn committed_v0_level_file_validates_first_try() {
    let file = LevelFile::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/levels/v0.toml"
    ))
    .unwrap();
    assert_eq!(file.levels.len(), 1);
    let level = file.levels[0].clone();
    let layout = build_layout(&file.world, std::slice::from_ref(&level));
    assert!(validate_level(&layout, &level, &t()).passed());
    // The loop agrees: first attempt, same level.
    let found = find_level(file.world.seed, |_| level.clone(), &t()).unwrap();
    assert_eq!((found.seed, found.attempts), (file.world.seed, 1));
    assert_eq!(found.level, default_v0_level(file.world.seed));
}

#[test]
fn raft_mask_cells_lie_at_the_stamped_positions() {
    let found = find_v0_level(3, &t()).unwrap();
    let [ax, az] = found.level.anchorage;
    let raft = Structure::raft_start();
    let mut expected: Vec<[i32; 3]> = raft
        .solid_cells([ax, terrain::SEA_LEVEL, az], 0)
        .iter()
        .map(|&(x, y, z, _)| [x, y, z])
        .collect();
    expected.sort_unstable();
    let lr = found
        .layout
        .generate_layout_region(ax - 8, az - 8, 17, 17, false);
    let mut got = lr.raft_cells.clone();
    got.sort_unstable();
    assert!(!got.is_empty());
    assert_eq!(got, expected);
}

#[test]
fn random_start_seeds_validate_or_report() {
    // A cheap xorshift stream of start seeds.
    let mut s = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..20 {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let start = s >> 1;
        match find_v0_level(start, &t()) {
            Ok(f) => {
                assert!(f.report.passed());
                assert!((1..=64).contains(&f.attempts));
            }
            Err(e) => {
                assert_eq!(e.attempts, 64);
                assert!(!e.failing_checks().is_empty());
            }
        }
    }
}

#[test]
fn hundred_starts_succeed_with_median_at_most_two() {
    let mut counts: Vec<u32> = (1..=100u64)
        .map(|s| find_v0_level(s, &t()).expect("within cap").attempts)
        .collect();
    counts.sort_unstable();
    assert!(counts[50] <= 2, "median {}", counts[50]);
}
