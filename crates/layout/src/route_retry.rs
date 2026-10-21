//! Route retry loop (docs/design/world-layout.md Phase E): like `find_level`,
//! but each seed builds a whole route and its layout and is checked with
//! `validate_route`. The seed is used for both the route and the world.

use std::fmt;

use crate::{
    build_route, build_route_layout, validate_route, Layout, Level, RetryError, Route, Thresholds,
    ValidationReport, WorldConfig,
};

/// A route that passed validation.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteFound {
    /// The seed that passed (start seed + attempts - 1).
    pub seed: u64,
    pub route: Route,
    pub levels: Vec<Level>,
    pub layout: Layout,
    pub report: ValidationReport,
    /// Seeds tried, counting the winning one (1 = first try).
    pub attempts: u32,
}

/// Try seeds `start_seed`, `start_seed + 1`, ... up to `t.max_attempts` (at
/// least 1), each with `k` further anchorages. The error is the shared
/// `RetryError`, naming the failing checks of the last attempt.
pub fn find_route(start_seed: u64, k: usize, t: &Thresholds) -> Result<RouteFound, RetryError> {
    let cap = t.max_attempts.max(1);
    let mut last = None;
    for i in 0..cap {
        let seed = start_seed.wrapping_add(i as u64);
        let route = build_route(seed, k);
        let (layout, levels) = build_route_layout(&WorldConfig { seed }, &route);
        let report = validate_route(&layout, &levels, &route, t);
        if report.passed() {
            return Ok(RouteFound {
                seed,
                route,
                levels,
                layout,
                report,
                attempts: i + 1,
            });
        }
        last = Some((seed, report));
    }
    let (last_seed, last_report) = last.expect("cap is at least 1");
    Err(RetryError {
        start_seed,
        attempts: cap,
        last_seed,
        last_report,
    })
}

impl fmt::Display for RouteFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "route seed {} after {} attempt(s)",
            self.seed, self.attempts
        )
    }
}
