//! Seed search driver (docs/design/world-layout.md Phase F, stages 2-3 and
//! "Use all CPU cores"; 6.1, 6.3): prefilter candidate seeds, validate the
//! survivors at full resolution, score them and keep the top N.
//!
//! Choices made here:
//! - One attempt per seed (the seed builds route and world), no retry loop.
//! - Score = sum of term * weight from `data/scoring.toml`; island terms are
//!   the mean over levels; the Phase G terms use the biased temperature at each
//!   anchorage and each island's biome (`route_biome`).
//! - Path directness uses `anchorage_paths`, the paths `validate_route` finds.
//! - Extra islands use an undecorated region around the route points plus
//!   `VIEW_MARGIN` blocks.
//! - Workers take seeds from a shared counter and keep their own top N; the
//!   merge sorts by score descending, then lower seed, so the worker count
//!   does not matter.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;

use crate::{
    anchorage_paths, anchorage_temperature, biome_variety, build_route, build_route_layout,
    climate_gradient, extra_islands, island_biome, path_directness, prefilter_layout, score_island,
    spacing_regularity, validate_route, window, Layout, Level, Route, RouteParams, ScoringConfig,
    Thresholds, WorldConfig,
};

/// Blocks of margin around the route points for the extra-islands region.
const VIEW_MARGIN: i32 = 64;

/// What to search.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchParams {
    /// First candidate seed; candidates are `start..start + candidates`.
    pub start: u64,
    pub candidates: u64,
    /// Entries kept.
    pub top: usize,
    /// Further anchorages per route (`build_route`'s `k`).
    pub k: usize,
    /// Worker threads; `None` is `available_parallelism()`. Always at least 1.
    pub workers: Option<usize>,
}

/// The scored terms (each 0..1) for one seed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TermScores {
    pub spacing_regularity: f64,
    pub island_quality: f64,
    pub coastline_interest: f64,
    pub extra_islands: f64,
    pub path_directness: f64,
    pub climate_gradient: f64,
    pub biome_variety: f64,
}

/// A seed that passed validation, with its scores.
#[derive(Clone, Debug, PartialEq)]
pub struct Scored {
    pub seed: u64,
    pub route: Route,
    pub levels: Vec<Level>,
    pub terms: TermScores,
    /// Weighted sum of `terms`.
    pub score: f64,
}

/// Running counts; read them from another thread while a search runs.
#[derive(Debug, Default)]
pub struct SearchProgress {
    pub prefilter_rejected: AtomicUsize,
    pub failed_validation: AtomicUsize,
    pub scored: AtomicUsize,
}

fn weighted(t: &TermScores, c: &ScoringConfig) -> f64 {
    t.spacing_regularity * c.weight_spacing_regularity
        + t.island_quality * c.weight_island_quality
        + t.coastline_interest * c.weight_coastline_interest
        + t.extra_islands * c.weight_extra_islands
        + t.path_directness * c.weight_path_directness
        + t.climate_gradient * c.weight_climate_gradient
        + t.biome_variety * c.weight_biome_variety
}

/// Region `(x0, z0, w, d)` boxing `pts` grown by `margin`.
fn bounds(pts: &[[i32; 2]], margin: i32) -> Option<(i32, i32, u32, u32)> {
    let lo = |k: usize| pts.iter().map(|p| p[k]).min().map(|v| v - margin);
    let hi = |k: usize| pts.iter().map(|p| p[k]).max().map(|v| v + margin);
    let (x0, z0) = (lo(0)?, lo(1)?);
    Some((x0, z0, (hi(0)? - x0 + 1) as u32, (hi(1)? - z0 + 1) as u32))
}

fn score_seed(
    layout: &Layout,
    levels: &[Level],
    route: &Route,
    t: &Thresholds,
    c: &ScoringConfig,
) -> TermScores {
    let (mut quality, mut coast) = (0.0, 0.0);
    for level in levels {
        let (x0, z0, w, d) = window(level, t);
        let (q, k) = score_island(&layout.generate_region(x0, z0, w, d, false), level, t, c);
        quality += q;
        coast += k;
    }
    let n = levels.len().max(1) as f64;
    let cells: Vec<[i32; 2]> = route
        .points
        .iter()
        .map(|p| p.map(|v| v.floor() as i32))
        .collect();
    let extra = bounds(&cells, VIEW_MARGIN).map_or(0.0, |(x0, z0, w, d)| {
        let region = layout.generate_region(x0, z0, w, d, false);
        let centres: Vec<[i32; 2]> = levels.iter().map(|l| l.island_center).collect();
        extra_islands(&region, &centres, &route.points, c).term
    });
    let temps: Vec<f64> = levels
        .iter()
        .map(|l| anchorage_temperature(layout, l))
        .collect();
    let biomes: Vec<_> = levels.iter().map(|l| island_biome(layout, l)).collect();
    TermScores {
        spacing_regularity: spacing_regularity(&route.points, &RouteParams::shipped()),
        island_quality: quality / n,
        coastline_interest: coast / n,
        extra_islands: extra,
        path_directness: path_directness(&anchorage_paths(layout, levels, route, t)),
        climate_gradient: climate_gradient(&temps, c),
        biome_variety: biome_variety(&biomes, c),
    }
}

/// Prefilter, validate and score one seed; `None` if it was rejected.
fn evaluate(
    seed: u64,
    k: usize,
    t: &Thresholds,
    c: &ScoringConfig,
    progress: &SearchProgress,
) -> Option<Scored> {
    let route = build_route(seed, k);
    let (layout, levels) = build_route_layout(&WorldConfig { seed }, &route);
    if prefilter_layout(&layout, &levels, &route).is_err() {
        progress.prefilter_rejected.fetch_add(1, Ordering::Relaxed);
        return None;
    }
    if !validate_route(&layout, &levels, &route, t).passed() {
        progress.failed_validation.fetch_add(1, Ordering::Relaxed);
        return None;
    }
    let terms = score_seed(&layout, &levels, &route, t, c);
    progress.scored.fetch_add(1, Ordering::Relaxed);
    Some(Scored {
        seed,
        route,
        levels,
        score: weighted(&terms, c),
        terms,
    })
}

fn rank(list: &mut Vec<Scored>, top: usize) {
    list.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.seed.cmp(&b.seed)));
    list.truncate(top);
}

/// Search `p.start..p.start + p.candidates` and return at most `p.top` seeds
/// that passed full validation, best first (ties: lower seed). The result
/// does not depend on `p.workers`. `progress` is updated as seeds finish.
pub fn search_seeds(
    p: &SearchParams,
    t: &Thresholds,
    c: &ScoringConfig,
    progress: &SearchProgress,
) -> Vec<Scored> {
    let workers = p
        .workers
        .or_else(|| thread::available_parallelism().ok().map(|n| n.get()))
        .unwrap_or(1)
        .max(1);
    let next = AtomicU64::new(0);
    let mut all = Vec::new();
    thread::scope(|s| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= p.candidates {
                            return mine;
                        }
                        let seed = p.start.wrapping_add(i);
                        mine.extend(evaluate(seed, p.k, t, c, progress));
                        rank(&mut mine, p.top);
                    }
                })
            })
            .collect();
        for h in handles {
            all.extend(h.join().expect("search worker panicked"));
        }
    });
    rank(&mut all, p.top);
    all
}
