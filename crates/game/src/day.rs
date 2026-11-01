//! The day clock: time of day as a pure function of the sim tick, so replays
//! and the world hash stay deterministic. See docs/design/day-night-cycle.md.
//!
//! Axes: x is east, y is up, z is south (a right-handed frame with the
//! viewer looking along -z). Time 0 is midnight and 0.5 is noon. With zero
//! azimuth the sun rises due east at 0.25, is highest at 0.5 and sets due west
//! at 0.75, leaning toward +z (south) by the tilt. Azimuth turns that path
//! about +y by the given angle, counter-clockwise seen from above.

use crate::clock::TICKS_PER_SECOND;
use crate::data_error::{DataError, DataErrorKind, DataFile};

const DEFAULT_DAY: &str = include_str!("../../../data/day.toml");

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DayClock {
    day_ticks: u64,
    start_offset_ticks: u64,
    sun_tilt: f64,
    sun_azimuth: f64,
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
        let sun_tilt = get("sun_tilt_degrees")?
            .as_float()
            .filter(|d| (0.0..90.0).contains(d))
            .ok_or_else(|| invalid("`sun_tilt_degrees` must be a number from 0 up to 90"))?;
        let sun_azimuth = get("sun_azimuth_degrees")?
            .as_float()
            .filter(|d| (0.0..360.0).contains(d))
            .ok_or_else(|| invalid("`sun_azimuth_degrees` must be a number from 0 up to 360"))?;
        Ok(DayClock {
            day_ticks,
            start_offset_ticks,
            sun_tilt: sun_tilt.to_radians(),
            sun_azimuth: sun_azimuth.to_radians(),
        })
    }

    /// Time of day at `tick`, in 0..1 (never 1.0); 0 is midnight. Wrapped in
    /// integer ticks and divided last, so it stays exact at huge ticks.
    pub fn time_of_day(&self, tick: u64) -> f64 {
        let ticks = (tick % self.day_ticks + self.start_offset_ticks) % self.day_ticks;
        ticks as f64 / self.day_ticks as f64
    }

    /// Unit vector toward the sun at time of day `t` (0..1, as from
    /// `time_of_day`). y > 0 from 0.25 to 0.75, highest at 0.5.
    pub fn sun_dir(&self, t: f64) -> [f64; 3] {
        let a = std::f64::consts::TAU * (t - 0.25);
        let (sin_a, cos_a) = a.sin_cos();
        let (sin_t, cos_t) = self.sun_tilt.sin_cos();
        let (x, y, z) = (cos_a, sin_a * cos_t, sin_a * sin_t);
        let (sin_z, cos_z) = self.sun_azimuth.sin_cos();
        [x * cos_z + z * sin_z, y, z * cos_z - x * sin_z]
    }

    /// `sun_dir` at the time of day of `tick`.
    pub fn sun_dir_at_tick(&self, tick: u64) -> [f64; 3] {
        self.sun_dir(self.time_of_day(tick))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "length_seconds = 10\nstart_fraction = 0.25\nsun_tilt_degrees = 30.0\nsun_azimuth_degrees = 40.0\n";
    const SUN: &str = "sun_tilt_degrees = 30.0\nsun_azimuth_degrees = 40.0\n";

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
            bad(&format!("start_fraction = 0.5\n{SUN}")).kind,
            DataErrorKind::MissingKey("length_seconds".into())
        );
        assert_eq!(
            bad(&format!("length_seconds = 5\n{SUN}")).kind,
            DataErrorKind::MissingKey("start_fraction".into())
        );
        assert_eq!(
            bad("length_seconds = 5\nstart_fraction = 0.5\nsun_azimuth_degrees = 0.0\n").kind,
            DataErrorKind::MissingKey("sun_tilt_degrees".into())
        );
        assert_eq!(
            bad("length_seconds = 5\nstart_fraction = 0.5\nsun_tilt_degrees = 0.0\n").kind,
            DataErrorKind::MissingKey("sun_azimuth_degrees".into())
        );
        let mut cases: Vec<String> = [
            "length_seconds = 0\nstart_fraction = 0.5\n",
            "length_seconds = -3\nstart_fraction = 0.5\n",
            "length_seconds = 5\nstart_fraction = 1.0\n",
            "length_seconds = 5\nstart_fraction = -0.1\n",
        ]
        .iter()
        .map(|t| format!("{t}{SUN}"))
        .collect();
        let head = "length_seconds = 5\nstart_fraction = 0.5\n";
        for (tilt, az) in [
            ("90.0", "0.0"),
            ("-1.0", "0.0"),
            ("nan", "0.0"),
            ("inf", "0.0"),
            ("\"x\"", "0.0"),
            ("10.0", "360.0"),
            ("10.0", "-5.0"),
            ("10.0", "nan"),
            ("10.0", "inf"),
        ] {
            cases.push(format!(
                "{head}sun_tilt_degrees = {tilt}\nsun_azimuth_degrees = {az}\n"
            ));
        }
        for t in &cases {
            let e = bad(t);
            assert_eq!(e.file, DataFile::Day);
            assert!(matches!(e.kind, DataErrorKind::Invalid(_)), "{t}");
        }
        assert!(matches!(bad("[").kind, DataErrorKind::Parse(_)));
    }

    #[test]
    fn sun_is_a_unit_vector_up_by_day_and_highest_at_noon() {
        for text in [
            TEXT,
            &TEXT.replace("30.0", "0.0"),
            &TEXT.replace("30.0", "80.0"),
        ] {
            let c = DayClock::from_toml(text).unwrap();
            let mut best = (f64::MIN, 0.0);
            for i in 0..2000 {
                let t = i as f64 / 2000.0;
                let d = c.sun_dir(t);
                let len = d.iter().map(|v| v * v).sum::<f64>().sqrt();
                assert!((len - 1.0).abs() < 1e-9, "{t}");
                if t > 0.25 + 1e-9 && t < 0.75 - 1e-9 {
                    assert!(d[1] > 0.0, "{t}");
                } else if !(0.25 - 1e-9..=0.75 + 1e-9).contains(&t) {
                    assert!(d[1] < 0.0, "{t}");
                }
                if d[1] > best.0 {
                    best = (d[1], t);
                }
            }
            assert_eq!(best.1, 0.5);
        }
    }

    #[test]
    fn sun_moves_smoothly() {
        let c = DayClock::from_toml(TEXT).unwrap();
        let n = 4000;
        let max_step = std::f64::consts::TAU / n as f64 * 1.01;
        for i in 0..n {
            let a = c.sun_dir(i as f64 / n as f64);
            let b = c.sun_dir((i + 1) as f64 / n as f64);
            let dot: f64 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
            assert!(dot.clamp(-1.0, 1.0).acos() <= max_step, "{i}");
        }
        // The last time of the day joins the first.
        let (a, b) = (c.sun_dir(0.0), c.sun_dir(1.0));
        assert!(a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-9));
    }

    #[test]
    fn sun_path_follows_tilt_and_azimuth() {
        let c = DayClock::from_toml(TEXT).unwrap();
        let noon = c.sun_dir(0.5);
        assert!((noon[1] - 30f64.to_radians().cos()).abs() < 1e-9);
        let flat =
            DayClock::from_toml(&TEXT.replace("30.0", "0.0").replace("40.0", "0.0")).unwrap();
        let rise = flat.sun_dir(0.25);
        assert!((rise[0] - 1.0).abs() < 1e-9 && rise[1].abs() < 1e-9);
        assert!((flat.sun_dir(0.5)[1] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn tick_form_goes_through_time_of_day() {
        let c = DayClock::from_toml(TEXT).unwrap();
        for tick in [0, 17, 100, 12345, u64::MAX] {
            assert_eq!(c.sun_dir_at_tick(tick), c.sun_dir(c.time_of_day(tick)));
        }
    }
}
