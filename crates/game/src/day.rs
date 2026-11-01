//! The day clock: time of day as a pure function of the sim tick, so replays
//! and the world hash stay deterministic. See docs/design/day-night-cycle.md.

use crate::clock::TICKS_PER_SECOND;
use crate::data_error::{DataError, DataErrorKind, DataFile};

const DEFAULT_DAY: &str = include_str!("../../../data/day.toml");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DayClock {
    day_ticks: u64,
    start_offset_ticks: u64,
}

fn err(kind: DataErrorKind) -> DataError {
    DataError::new(DataFile::Day, kind)
}

fn invalid(msg: &str) -> DataError {
    err(DataErrorKind::Invalid(msg.into()))
}

impl DayClock {
    pub fn default_day() -> Result<DayClock, DataError> {
        DayClock::from_toml(DEFAULT_DAY)
    }

    pub fn from_toml(text: &str) -> Result<DayClock, DataError> {
        let table: toml::Value =
            toml::from_str(text).map_err(|e| err(DataErrorKind::Parse(e.to_string())))?;
        let get = |key: &str| {
            table
                .get(key)
                .ok_or_else(|| err(DataErrorKind::MissingKey(key.into())))
        };
        let secs = get("length_seconds")?
            .as_integer()
            .filter(|s| *s > 0)
            .ok_or_else(|| invalid("`length_seconds` must be a positive integer"))?;
        let start = get("start_fraction")?
            .as_float()
            .filter(|f| (0.0..1.0).contains(f))
            .ok_or_else(|| invalid("`start_fraction` must be a number from 0 up to 1"))?;
        let day_ticks = (secs as u64)
            .checked_mul(u64::from(TICKS_PER_SECOND))
            .ok_or_else(|| invalid("`length_seconds` is too large"))?;
        // start < 1 so the rounded offset can reach day_ticks; wrap it.
        let start_offset_ticks = (start * day_ticks as f64).round() as u64 % day_ticks;
        Ok(DayClock {
            day_ticks,
            start_offset_ticks,
        })
    }

    /// Time of day at `tick`, in 0..1 (never 1.0); 0 is midnight. Wrapped in
    /// integer ticks and divided last, so it stays exact at huge ticks.
    pub fn time_of_day(&self, tick: u64) -> f64 {
        let ticks = (tick % self.day_ticks + self.start_offset_ticks) % self.day_ticks;
        ticks as f64 / self.day_ticks as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "length_seconds = 10\nstart_fraction = 0.25\n";

    #[test]
    fn starts_at_the_start_fraction_and_wraps() {
        let c = DayClock::from_toml(TEXT).unwrap();
        assert_eq!(c.time_of_day(0), 0.25);
        assert_eq!(c.time_of_day(100), 0.75);
        assert_eq!(c.time_of_day(150), 0.0);
        assert_eq!(c.time_of_day(200), 0.25);
        for t in 0..1000 {
            assert!((0.0..1.0).contains(&c.time_of_day(t)));
        }
    }

    #[test]
    fn exact_at_a_very_large_tick() {
        let c = DayClock::from_toml(TEXT).unwrap();
        let t = u64::MAX / 2;
        let want = ((t % 200 + 50) % 200) as f64 / 200.0;
        assert_eq!(c.time_of_day(t), want);
        // u64::MAX % 200 == 15, so 15 + 50 ticks into the day.
        assert_eq!(c.time_of_day(u64::MAX), 65.0 / 200.0);
    }

    #[test]
    fn default_data_loads() {
        let c = DayClock::default_day().unwrap();
        assert!((0.0..1.0).contains(&c.time_of_day(0)));
    }

    #[test]
    fn bad_data_is_an_error() {
        let bad = |t: &str| DayClock::from_toml(t).unwrap_err();
        assert_eq!(
            bad("start_fraction = 0.5\n").kind,
            DataErrorKind::MissingKey("length_seconds".into())
        );
        assert_eq!(
            bad("length_seconds = 5\n").kind,
            DataErrorKind::MissingKey("start_fraction".into())
        );
        for t in [
            "length_seconds = 0\nstart_fraction = 0.5\n",
            "length_seconds = -3\nstart_fraction = 0.5\n",
            "length_seconds = 5\nstart_fraction = 1.0\n",
            "length_seconds = 5\nstart_fraction = -0.1\n",
        ] {
            let e = bad(t);
            assert_eq!(e.file, DataFile::Day);
            assert!(matches!(e.kind, DataErrorKind::Invalid(_)), "{t}");
        }
        assert!(matches!(bad("[").kind, DataErrorKind::Parse(_)));
    }
}
