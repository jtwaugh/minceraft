//! The day phase clock: gathering and service alternate from sim tick 0.
//! A pure function of the tick, so replays and the world hash stay
//! deterministic. See docs/design/basic-gameplay.md section 12.

use crate::clock::TICKS_PER_SECOND;
use crate::data_error::{DataError, DataErrorKind, DataFile};

const DEFAULT_PHASES: &str = include_str!("../../../data/phases.toml");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Gathering,
    Service,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Gathering => "gathering",
            Phase::Service => "service",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseClock {
    gathering_ticks: u64,
    service_ticks: u64,
}

fn err(kind: DataErrorKind) -> DataError {
    DataError::new(DataFile::Phases, kind)
}

fn length(table: &toml::Value, phase: &str) -> Result<u64, DataError> {
    let key = format!("{phase}.seconds");
    let secs = table
        .get(phase)
        .and_then(|t| t.get("seconds"))
        .ok_or_else(|| err(DataErrorKind::MissingKey(key.clone())))?
        .as_integer()
        .filter(|s| *s > 0)
        .ok_or_else(|| {
            err(DataErrorKind::Invalid(format!(
                "`{key}` must be a positive integer"
            )))
        })?;
    Ok(secs as u64 * u64::from(TICKS_PER_SECOND))
}

impl PhaseClock {
    pub fn default_phases() -> Result<PhaseClock, DataError> {
        PhaseClock::from_toml(DEFAULT_PHASES)
    }

    pub fn from_toml(text: &str) -> Result<PhaseClock, DataError> {
        let table: toml::Value =
            toml::from_str(text).map_err(|e| err(DataErrorKind::Parse(e.to_string())))?;
        Ok(PhaseClock {
            gathering_ticks: length(&table, "gathering")?,
            service_ticks: length(&table, "service")?,
        })
    }

    /// The phase at `tick` and the whole seconds left in it (rounded up, so
    /// the last tick of a phase shows 1).
    pub fn at(&self, tick: u64) -> (Phase, u32) {
        let offset = tick % (self.gathering_ticks + self.service_ticks);
        let (phase, left) = if offset < self.gathering_ticks {
            (Phase::Gathering, self.gathering_ticks - offset)
        } else {
            (
                Phase::Service,
                self.gathering_ticks + self.service_ticks - offset,
            )
        };
        let tps = u64::from(TICKS_PER_SECOND);
        (phase, left.div_ceil(tps) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_alternate_from_tick_zero() {
        let c = PhaseClock::default_phases().unwrap();
        assert_eq!(c.at(0), (Phase::Gathering, 120));
        assert_eq!(c.at(2399), (Phase::Gathering, 1));
        assert_eq!(c.at(2400), (Phase::Service, 120));
        assert_eq!(c.at(4799), (Phase::Service, 1));
        assert_eq!(c.at(4800), (Phase::Gathering, 120));
        assert_eq!(c.at(20), (Phase::Gathering, 119));
    }

    #[test]
    fn bad_data_is_an_error() {
        let bad = |t: &str| PhaseClock::from_toml(t).unwrap_err();
        assert_eq!(
            bad("[gathering]\nseconds = 5\n").kind,
            DataErrorKind::MissingKey("service.seconds".into())
        );
        assert_eq!(
            bad("[service]\nseconds = 5\n").kind,
            DataErrorKind::MissingKey("gathering.seconds".into())
        );
        let zero = bad("[gathering]\nseconds = 0\n[service]\nseconds = 5\n");
        assert!(matches!(zero.kind, DataErrorKind::Invalid(_)));
        assert!(matches!(bad("[").kind, DataErrorKind::Parse(_)));
    }
}
