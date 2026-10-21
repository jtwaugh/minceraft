use super::*;

fn t() -> Thresholds {
    Thresholds::shipped()
}

#[test]
fn found_route_validates_and_is_deterministic() {
    let start = 1u64;
    let a = find_route(start, 1, &t());
    assert_eq!(a, find_route(start, 1, &t()));
    let f = a.expect("a valid route within the cap");
    assert!(f.report.passed());
    assert_eq!(f.seed, start + (f.attempts as u64 - 1));
    assert_eq!(f.route, build_route(f.seed, 1));
    assert_eq!(f.levels.len(), 2);
}

#[test]
fn cap_of_one_names_failing_checks_or_passes() {
    let one = Thresholds {
        max_attempts: 1,
        ..t()
    };
    for start in 1..4u64 {
        match find_route(start, 1, &one) {
            Ok(f) => assert_eq!((f.seed, f.attempts), (start, 1)),
            Err(e) => {
                assert_eq!((e.attempts, e.last_seed), (1, start));
                assert!(!e.failing_checks().is_empty());
            }
        }
    }
}
