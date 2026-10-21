//! `find_v0 [n]`: retry-count distribution over start seeds 1..=n (default
//! 100). `find_v0 level <start>`: the level file TOML for the winning seed.
use layout::{find_v0_level, LevelFile, Thresholds, WorldConfig};

fn main() {
    let t = Thresholds::shipped();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("level") {
        let start: u64 = args[1].parse().expect("start seed");
        let f = find_v0_level(start, &t).unwrap_or_else(|e| panic!("{e}"));
        eprintln!(
            "start {start} -> seed {} after {} attempts",
            f.seed, f.attempts
        );
        let file = LevelFile {
            world: WorldConfig { seed: f.seed },
            levels: vec![f.level],
            route: None,
        };
        print!("{}", file.to_toml().unwrap());
        return;
    }
    let n: u64 = args.first().and_then(|a| a.parse().ok()).unwrap_or(100);
    let mut counts = Vec::new();
    for start in 1..=n {
        match find_v0_level(start, &t) {
            Ok(f) => counts.push(f.attempts),
            Err(e) => println!("start {start}: {e}"),
        }
    }
    counts.sort_unstable();
    println!("ok {}/{n}", counts.len());
    println!(
        "attempts: min {} median {} p90 {} max {}",
        counts[0],
        counts[counts.len() / 2],
        counts[counts.len() * 9 / 10],
        counts[counts.len() - 1]
    );
    let mut hist = std::collections::BTreeMap::new();
    for c in &counts {
        *hist.entry(*c).or_insert(0) += 1;
    }
    println!("histogram (attempts: starts): {hist:?}");
}
