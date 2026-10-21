//! Runs `find_route` for start seeds 1..=N (default 100, route of K further
//! anchorages, default 2): attempts needed and failing-check counts.
use std::collections::BTreeMap;

use layout::{find_route, Thresholds};

fn main() {
    let arg = |i: usize, d: u64| {
        std::env::args()
            .nth(i)
            .and_then(|a| a.parse().ok())
            .unwrap_or(d)
    };
    let (n, k) = (arg(1, 100), arg(2, 2) as usize);
    let t = Thresholds::shipped();
    let (mut found, mut attempts) = (0u32, Vec::new());
    let mut failing: BTreeMap<String, u32> = BTreeMap::new();
    for start in 1..=n {
        match find_route(start, k, &t) {
            Ok(f) => {
                found += 1;
                attempts.push(f.attempts);
            }
            Err(e) => {
                println!("start {start}: {e}");
                for c in e.failing_checks() {
                    *failing.entry(c.name.to_string()).or_default() += 1;
                }
            }
        }
    }
    attempts.sort_unstable();
    println!("start seeds 1..={n}, k={k}, cap {}", t.max_attempts);
    println!("found {found}/{n}");
    if let (Some(first), Some(last)) = (attempts.first(), attempts.last()) {
        let mean = attempts.iter().sum::<u32>() as f64 / attempts.len() as f64;
        let med = attempts[attempts.len() / 2];
        println!("attempts: min {first} median {med} mean {mean:.2} max {last}");
    }
    for (name, c) in &failing {
        println!("last-attempt failures {name}: {c}");
    }
}
