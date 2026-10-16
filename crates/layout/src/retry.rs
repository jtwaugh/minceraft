//! Retry loop (docs/design/world-layout.md Phase B, 6.1): build the layout
//! for a seed, validate it, and on failure try the next seed. The increment
//! lives here, above `terrain`, so terrain determinism is untouched.

use std::fmt;

use crate::{
    build_layout, default_v0_level, validate_level, CheckResult, Layout, Level, Thresholds,
    ValidationReport, WorldConfig,
};

/// A level that passed validation.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelFound {
    /// The seed that passed (start seed + attempts - 1).
    pub seed: u64,
    pub level: Level,
    pub layout: Layout,
    pub report: ValidationReport,
    /// Seeds tried, counting the winning one (1 = first try).
    pub attempts: u32,
}

/// The cap was hit; names the failing checks of the last attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct RetryError {
    pub start_seed: u64,
    pub attempts: u32,
    pub last_seed: u64,
    pub last_report: ValidationReport,
}

impl RetryError {
    pub fn failing_checks(&self) -> Vec<&CheckResult> {
        self.last_report
            .checks
            .iter()
            .filter(|c| !c.passed)
            .collect()
    }
}

impl fmt::Display for RetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "no valid level in {} attempts from seed {} (last seed {}); failing checks:",
            self.attempts, self.start_seed, self.last_seed
        )?;
        for c in self.failing_checks() {
            write!(f, " [{c}]")?;
        }
        Ok(())
    }
}

impl std::error::Error for RetryError {}

/// Try seeds `start_seed`, `start_seed + 1`, ... up to `t.max_attempts`
/// (at least 1). `make_level` describes the level for each seed.
pub fn find_level(
    start_seed: u64,
    make_level: impl Fn(u64) -> Level,
    t: &Thresholds,
) -> Result<LevelFound, RetryError> {
    let cap = t.max_attempts.max(1);
    let mut last = None;
    for i in 0..cap {
        let seed = start_seed.wrapping_add(i as u64);
        let level = make_level(seed);
        let layout = build_layout(&WorldConfig { seed }, std::slice::from_ref(&level));
        let report = validate_level(&layout, &level, t);
        if report.passed() {
            return Ok(LevelFound {
                seed,
                level,
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

/// `find_level` with the default v0 level placement.
pub fn find_v0_level(start_seed: u64, t: &Thresholds) -> Result<LevelFound, RetryError> {
    find_level(start_seed, default_v0_level, t)
}
