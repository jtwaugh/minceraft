//! Searches candidate worlds and prints the Phase G climate statistics:
//! the share of the validated worlds whose first anchorage is cold and last
//! warm, and the share of consecutive anchorage pairs on different biomes in
//! the top-scored worlds.
//! Usage: route_climate_stats [candidates=400] [worlds=100] [top=10] [start=1]
use layout::{
    biome_pair_totals, cold_first_warm_last, search_seeds, share, ScoringConfig, SearchParams,
    SearchProgress, Thresholds,
};

fn main() {
    let arg = |i: usize, d: u64| {
        std::env::args()
            .nth(i)
            .and_then(|a| a.parse().ok())
            .unwrap_or(d)
    };
    let (candidates, worlds, top, start) = (arg(1, 400), arg(2, 100), arg(3, 10), arg(4, 1));
    let params = SearchParams {
        start,
        candidates,
        top: worlds as usize,
        k: 4,
        workers: None,
    };
    let progress = SearchProgress::default();
    let found = search_seeds(
        &params,
        &Thresholds::shipped(),
        &ScoringConfig::shipped(),
        &progress,
    );
    let (hits, n) = cold_first_warm_last(&found);
    println!(
        "candidates {start}..{}, validated worlds kept {n}",
        start + candidates
    );
    println!(
        "cold first and warm last: {hits}/{n} = {:.1}% (target at least 90%)",
        100.0 * share(hits, n)
    );
    let best = &found[..found.len().min(top as usize)];
    let (diff, pairs) = biome_pair_totals(best);
    println!(
        "top {} worlds: consecutive anchorage pairs on different biomes: {diff}/{pairs} = {:.1}% (target at least 75%)",
        best.len(),
        100.0 * share(diff, pairs)
    );
}
