//! Seed search driver (docs/design/world-layout.md Phase F, stages 2-3 and
//! "Use all CPU cores"; 6.1, 6.3): prefilter candidate seeds, validate the
//! survivors at full resolution, score them and keep the top N.
//!
//! Choices made here:
//! - Each candidate gets one attempt: its seed builds the route and the
//!   world, as in `find_route`, with no retry loop.
//! - The total score is the sum of each term times its weight in
//!   `data/scoring.toml` (not divided by the weights). The island terms are
//!   the mean over the levels. Phase G terms are not included.
//! - Path directness uses the same per-pair window and eroded A* as
//!   `validate_route`; a pair with no path (not possible after validation)
//!   is left out of the mean.
//! - Extra islands are counted on an undecorated region covering the route
//!   points grown by `VIEW_MARGIN` blocks.
//! - Workers take the next unclaimed seed from a shared counter and keep their
//!   own top N; the merge sorts by score descending, then lower seed, so the
//!   result does not depend on the worker count.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread;

use crate::{
    build_route, build_route_layout, erode, extra_islands, find_path, path_directness,
    prefilter_layout, score_island, spacing_regularity, validate_route, window, Layout, Level,
    NavMap, Route, RouteParams, ScoringConfig, Thresholds, WorldConfig,
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

/// The five scored terms (each 0..1) for one seed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TermScores {
    pub spacing_regularity: f64,
    pub path_directness: f64,
    pub island_quality: f64,
    pub coastline_interest: f64,
    pub extra_islands: f64,
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
        + t.path_directness * c.weight_path_directness
        + t.island_quality * c.weight_island_quality
        + t.coastline_interest * c.weight_coastline_interest
        + t.extra_islands * c.weight_extra_islands
}

/// One A* path per consecutive anchorage pair, windowed as in `validate_route`.
fn pair_paths(
    layout: &Layout,
    levels: &[Level],
    route: &Route,
    t: &Thresholds,
) -> Vec<Vec<[i32; 2]>> {
    let bare = Layout {
        structures: Vec::new(),
        ..layout.clone()
    };
    let half = RouteParams::shipped().channel_half_width.ceil() as i32;
    let margin = t.window_margin + t.raft_half_width as i32 + half;
    let pairs = levels.len().min(route.points.len()).saturating_sub(1);
    (0..pairs)
        .filter_map(|i| {
            let [a, b] = [levels[i].anchorage, levels[i + 1].anchorage];
            let [pa, pb] = [route.points[i], route.points[i + 1]];
            let xs = [a[0], b[0], pa[0].floor() as i32, pb[0].floor() as i32];
            let zs = [a[1], b[1], pa[1].floor() as i32, pb[1].floor() as i32];
            let x0 = xs.into_iter().min()? - margin;
            let z0 = zs.into_iter().min()? - margin;
            let w = (xs.into_iter().max()? + margin - x0 + 1) as u32;
            let d = (zs.into_iter().max()? + margin - z0 + 1) as u32;
            let region = bare.generate_region(x0, z0, w, d, true);
            let nav = erode(&NavMap::from_region(&region, t), t.raft_half_width);
            find_path(&nav, [a[0] - x0, a[1] - z0], [b[0] - x0, b[1] - z0])
        })
        .collect()
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
    let xs = route.points.iter().map(|p| p[0].floor() as i32);
    let zs = route.points.iter().map(|p| p[1].floor() as i32);
    let extra = match (xs.clone().min(), xs.max(), zs.clone().min(), zs.max()) {
        (Some(x0), Some(x1), Some(z0), Some(z1)) => {
            let (x0, z0) = (x0 - VIEW_MARGIN, z0 - VIEW_MARGIN);
            let region = layout.generate_region(
                x0,
                z0,
                (x1 + VIEW_MARGIN - x0 + 1) as u32,
                (z1 + VIEW_MARGIN - z0 + 1) as u32,
                false,
            );
            let centres: Vec<[i32; 2]> = levels.iter().map(|l| l.island_center).collect();
            extra_islands(&region, &centres, &route.points, c).term
        }
        _ => 0.0,
    };
    TermScores {
        spacing_regularity: spacing_regularity(&route.points, &RouteParams::shipped()),
        path_directness: path_directness(&pair_paths(layout, levels, route, t)),
        island_quality: quality / n,
        coastline_interest: coast / n,
        extra_islands: extra,
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
