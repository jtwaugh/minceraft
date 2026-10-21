//! Seed search throughput: `seed_bench [candidates=500] [k=4] [start=1]`.
//! Run with `--profile bench-native`. Prints candidates per second, the
//! prefilter reject and validation fail rates, time per stage, and the biome
//! and temperature at each anchorage of the top results (k=4 is five levels).
use std::time::{Duration, Instant};

use layout::{
    build_route, build_route_layout, prefilter_layout, search_seeds, validate_route, ScoringConfig,
    SearchParams, SearchProgress, Thresholds, WorldConfig,
};
use terrain::{biome_for, Climate};

fn main() {
    let arg = |i: usize, d: u64| {
        std::env::args()
            .nth(i)
            .and_then(|a| a.parse().ok())
            .unwrap_or(d)
    };
    let (n, k, start) = (arg(1, 500), arg(2, 4) as usize, arg(3, 1));
    let (t, c) = (Thresholds::shipped(), ScoringConfig::shipped());
    let params = |workers| SearchParams {
        start,
        candidates: n,
        top: 5,
        k,
        workers,
    };

    // Stage times, single thread, same candidates as the search.
    let (mut build, mut pre, mut val) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    let (mut rejected, mut failed, mut passed) = (0u64, 0u64, 0u64);
    for seed in start..start + n {
        let now = Instant::now();
        let route = build_route(seed, k);
        let (layout, levels) = build_route_layout(&WorldConfig { seed }, &route);
        build += now.elapsed();
        let now = Instant::now();
        let ok = prefilter_layout(&layout, &levels, &route).is_ok();
        pre += now.elapsed();
        if !ok {
            rejected += 1;
            continue;
        }
        let now = Instant::now();
        let ok = validate_route(&layout, &levels, &route, &t).passed();
        val += now.elapsed();
        if ok {
            passed += 1;
        } else {
            failed += 1;
        }
    }
    let staged = build + pre + val;

    // Whole search on one worker gives scoring by difference.
    let now = Instant::now();
    search_seeds(&params(Some(1)), &t, &c, &SearchProgress::default());
    let single = now.elapsed();
    let score = single.saturating_sub(staged);

    // Whole search on all cores.
    let progress = SearchProgress::default();
    let now = Instant::now();
    let top = search_seeds(&params(None), &t, &c, &progress);
    let wall = now.elapsed();

    let pct = |a: u64, b: u64| 100.0 * a as f64 / b.max(1) as f64;
    let cores = std::thread::available_parallelism().map_or(1, |c| c.get());
    println!("candidates {n} from seed {start}, k={k} ({} levels)", k + 1);
    println!(
        "all cores ({cores}): {:.2}s, {:.1} candidates/s; projected 5000: {:.0}s",
        wall.as_secs_f64(),
        n as f64 / wall.as_secs_f64(),
        5000.0 * wall.as_secs_f64() / n as f64
    );
    println!(
        "one worker: {:.2}s, {:.1} candidates/s; projected 5000: {:.0}s",
        single.as_secs_f64(),
        n as f64 / single.as_secs_f64(),
        5000.0 * single.as_secs_f64() / n as f64
    );
    println!(
        "prefilter rejected {rejected} ({:.1}%), failed validation {failed} ({:.1}% of survivors), scored {passed}",
        pct(rejected, n),
        pct(failed, n - rejected)
    );
    println!(
        "search counters: rejected {}, failed {}, scored {}",
        progress
            .prefilter_rejected
            .load(std::sync::atomic::Ordering::Relaxed),
        progress
            .failed_validation
            .load(std::sync::atomic::Ordering::Relaxed),
        progress.scored.load(std::sync::atomic::Ordering::Relaxed),
    );
    println!("stage times, one thread (ms per candidate entering the stage):");
    let ms = |d: Duration, m: u64| 1000.0 * d.as_secs_f64() / m.max(1) as f64;
    println!("  route+layout {:.2}", ms(build, n));
    println!("  prefilter    {:.2}", ms(pre, n));
    println!("  validation   {:.2}", ms(val, n - rejected));
    println!("  scoring      {:.2}", ms(score, passed));

    println!(
        "top {} (score, seed; anchorage: biome, temperature):",
        top.len()
    );
    let climate_of = |seed| Climate::new(seed);
    for s in &top {
        println!("seed {} score {:.3}", s.seed, s.score);
        let clim = climate_of(s.seed);
        for l in &s.levels {
            let [x, z] = l.anchorage;
            let (temp, hum) = clim.raw(x, z);
            println!(
                "  {} ({x},{z}): {:?} T {temp:.3} H {hum:.3}",
                l.id,
                biome_for(temp, hum)
            );
        }
    }
}
