//! Checks shared by the data-file parsers (`fish_rules`, `habitat_rules`).
//! `build.rs` includes this file by path, so keep it free of dependencies.

/// Error unless an inclusive `[min, max]` range, where both ends are set, has
/// `min <= max`. `what` names the rule, e.g. "species 'cod'".
pub fn check_range<T: PartialOrd>(
    what: &str,
    field: &str,
    min: Option<T>,
    max: Option<T>,
) -> Result<(), String> {
    match (min, max) {
        (Some(a), Some(b)) if a > b => Err(format!("{what}: {field}_min > {field}_max")),
        _ => Ok(()),
    }
}
