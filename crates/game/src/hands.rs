//! Hand animation counter (docs/design/held-and-world-items.md section 1).
//!
//! Counts sim ticks since a primary swing or a secondary use started, so the
//! client and input scripts can read the animation from the sim.
//!
//! A phase is 0 when idle, rises linearly to 1 at the middle of the duration
//! and falls back to 0 as the duration ends. The tick an animation starts on
//! is tick 1 of it. A new action during an animation restarts it from tick 1
//! (it does not queue and is not ignored). Primary and secondary are
//! independent counters. Death clears both.

use crate::data_error::{DataError, DataErrorKind, DataFile};

const DEFAULT_HANDS: &str = include_str!("../../../data/hands.toml");

/// Animation lengths, in sim ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandsConfig {
    pub swing_ticks: u32,
    pub use_ticks: u32,
}

impl HandsConfig {
    /// The shipped values from `data/hands.toml`.
    pub fn default_from_data() -> Result<HandsConfig, DataError> {
        HandsConfig::parse(DEFAULT_HANDS)
    }

    /// Parse hands text; both durations are required and at least 2 ticks.
    pub fn parse(text: &str) -> Result<HandsConfig, DataError> {
        let err = |kind| DataError::new(DataFile::Hands, kind);
        let root: toml::Value = text
            .parse()
            .map_err(|e: toml::de::Error| err(DataErrorKind::Parse(e.to_string())))?;
        let get = |key: &str| -> Result<u32, DataError> {
            let v = root
                .get(key)
                .ok_or_else(|| err(DataErrorKind::MissingKey(key.into())))?;
            v.as_integer()
                .filter(|n| (2..=i64::from(u32::MAX)).contains(n))
                .map(|n| n as u32)
                .ok_or_else(|| {
                    err(DataErrorKind::Invalid(format!(
                        "{key} must be a whole number of ticks, at least 2"
                    )))
                })
        };
        Ok(HandsConfig {
            swing_ticks: get("swing_ticks")?,
            use_ticks: get("use_ticks")?,
        })
    }
}

/// Per-player animation counters. Zero means idle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hands {
    config: HandsConfig,
    swing_elapsed: u32,
    use_elapsed: u32,
}

impl Hands {
    pub fn new(config: HandsConfig) -> Hands {
        Hands {
            config,
            swing_elapsed: 0,
            use_elapsed: 0,
        }
    }

    /// Advance one sim tick; an animation that reaches its length ends.
    pub fn advance(&mut self) {
        Self::step(&mut self.swing_elapsed, self.config.swing_ticks);
        Self::step(&mut self.use_elapsed, self.config.use_ticks);
    }

    fn step(elapsed: &mut u32, length: u32) {
        if *elapsed > 0 {
            *elapsed += 1;
            if *elapsed >= length {
                *elapsed = 0;
            }
        }
    }

    /// Start (or restart) the primary swing on this tick.
    pub fn start_swing(&mut self) {
        self.swing_elapsed = 1;
    }

    /// Start (or restart) the secondary use on this tick.
    pub fn start_use(&mut self) {
        self.use_elapsed = 1;
    }

    /// Stop both animations, as when the player dies.
    pub fn clear(&mut self) {
        self.swing_elapsed = 0;
        self.use_elapsed = 0;
    }

    /// 0 when idle, up to 1 mid-swing, back to 0.
    pub fn swing_phase(&self) -> f32 {
        Self::phase(self.swing_elapsed, self.config.swing_ticks)
    }

    /// 0 when idle, up to 1 mid-use, back to 0.
    pub fn use_phase(&self) -> f32 {
        Self::phase(self.use_elapsed, self.config.use_ticks)
    }

    fn phase(elapsed: u32, length: u32) -> f32 {
        if elapsed == 0 {
            return 0.0;
        }
        let t = elapsed as f32 / length as f32;
        1.0 - (2.0 * t - 1.0).abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_error::test_support::*;

    fn hands() -> Hands {
        Hands::new(HandsConfig::default_from_data().unwrap())
    }

    #[test]
    fn shipped_data_loads() {
        let c = HandsConfig::default_from_data().unwrap();
        assert!(c.use_ticks < c.swing_ticks);
    }

    #[test]
    fn loader_rejects_bad_data() {
        assert_parse_error(
            HandsConfig::parse("swing_ticks =").unwrap_err(),
            DataFile::Hands,
        );
        assert_missing_key(
            HandsConfig::parse("swing_ticks = 6").unwrap_err(),
            DataFile::Hands,
            "use_ticks",
        );
        assert!(HandsConfig::parse("swing_ticks = 1\nuse_ticks = 4").is_err());
        assert!(HandsConfig::parse("swing_ticks = 6\nuse_ticks = 4.5").is_err());
    }

    #[test]
    fn idle_is_zero() {
        let mut h = hands();
        assert_eq!((h.swing_phase(), h.use_phase()), (0.0, 0.0));
        h.advance();
        assert_eq!((h.swing_phase(), h.use_phase()), (0.0, 0.0));
    }

    #[test]
    fn swing_rises_then_falls_to_zero() {
        let n = hands().config.swing_ticks;
        let mut h = hands();
        h.start_swing();
        let mut phases = vec![h.swing_phase()];
        for _ in 1..n {
            h.advance();
            phases.push(h.swing_phase());
        }
        let expected: Vec<f32> = (1..n)
            .map(|k| 1.0 - (2.0 * k as f32 / n as f32 - 1.0).abs())
            .chain([0.0])
            .collect();
        assert_eq!(phases, expected);
        assert!(phases[0] > 0.0);
        assert_eq!(h.use_phase(), 0.0);
        let peak = phases.iter().cloned().fold(0.0, f32::max);
        assert_eq!(peak, 1.0);
    }

    #[test]
    fn use_runs_its_own_length() {
        let n = hands().config.use_ticks;
        let mut h = hands();
        h.start_use();
        for _ in 1..n {
            assert!(h.use_phase() > 0.0);
            assert_eq!(h.swing_phase(), 0.0);
            h.advance();
        }
        assert_eq!(h.use_phase(), 0.0);
    }

    #[test]
    fn a_new_press_restarts_the_animation() {
        let mut h = hands();
        h.start_swing();
        h.advance();
        h.advance();
        h.start_swing();
        assert_eq!(h, {
            let mut f = hands();
            f.start_swing();
            f
        });
    }

    #[test]
    fn clear_stops_both() {
        let mut h = hands();
        h.start_swing();
        h.start_use();
        h.clear();
        assert_eq!((h.swing_phase(), h.use_phase()), (0.0, 0.0));
    }
}
