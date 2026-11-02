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
    min_daylight: f64,
    dawn_width: f64,
    dusk_width: f64,
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
        let min_daylight = get("min_daylight")?
            .as_float()
            .filter(|f| (0.0..=1.0).contains(f))
            .ok_or_else(|| invalid("`min_daylight` must be a number from 0 to 1"))?;
        let dawn_width = get("dawn_width")?
            .as_float()
            .filter(|w| *w > 0.0 && *w <= 0.5)
            .ok_or_else(|| invalid("`dawn_width` must be a number above 0, up to 0.5"))?;
        let dusk_width = get("dusk_width")?
            .as_float()
            .filter(|w| *w > 0.0 && *w <= 0.5)
            .ok_or_else(|| invalid("`dusk_width` must be a number above 0, up to 0.5"))?;
        Ok(DayClock {
            min_daylight,
            dawn_width,
            dusk_width,
            day_ticks,
            start_offset_ticks,
            sun_tilt: sun_tilt.to_radians(),
            sun_azimuth: sun_azimuth.to_radians(),
        })
    }

    /// Ticks in one day.
    pub fn day_ticks(&self) -> u64 {
        self.day_ticks
    }

    /// The tick within the day at which tick 0 starts (the start fraction).
    pub fn start_offset_ticks(&self) -> u64 {
        self.start_offset_ticks
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

    /// Scene brightness at time of day `t`, from `min_daylight` (night) to 1
    /// (day). Dawn is a smoothstep of time up from the minimum, centred on the
    /// sunrise at 0.25 and `dawn_width` of the day wide; dusk is the mirror
    /// down, centred on 0.75 and `dusk_width` wide. Smoothstep has zero slope
    /// at both ends, so there is no visible kink where a ramp meets day or night.
    pub fn daylight(&self, t: f64) -> f64 {
        // Measure from the start of dawn so neither ramp wraps past the end
        // of the day; the loader's width limit (0.5) keeps them apart
        // and makes noon full daylight.
        let dawn_start = 0.25 - self.dawn_width / 2.0;
        let dusk_start = 0.75 - self.dusk_width / 2.0 - dawn_start;
        let u = (t - dawn_start).rem_euclid(1.0);
        let ramp = |start: f64, width: f64| {
            let x = ((u - start) / width).clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        let k = ramp(0.0, self.dawn_width) - ramp(dusk_start, self.dusk_width);
        self.min_daylight + (1.0 - self.min_daylight) * k
    }

    /// `daylight` at the time of day of `tick`.
    pub fn daylight_at_tick(&self, tick: u64) -> f64 {
        self.daylight(self.time_of_day(tick))
    }

    /// `sun_dir` at the time of day of `tick`.
    pub fn sun_dir_at_tick(&self, tick: u64) -> [f64; 3] {
        self.sun_dir(self.time_of_day(tick))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "length_seconds = 10\nstart_fraction = 0.25\nsun_tilt_degrees = 30.0\nsun_azimuth_degrees = 40.0\nmin_daylight = 0.2\ndawn_width = 0.1\ndusk_width = 0.2\n";
    const SUN: &str = "sun_tilt_degrees = 30.0\nsun_azimuth_degrees = 40.0\nmin_daylight = 0.2\ndawn_width = 0.1\ndusk_width = 0.2\n";

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

    #[test]
    fn daylight_is_full_at_noon_and_minimum_at_midnight() {
        let c = DayClock::from_toml(TEXT).unwrap();
        assert_eq!(c.daylight(0.5), 1.0);
        assert_eq!(c.daylight(0.0), 0.2);
        assert!((c.daylight(0.25) - 0.6).abs() < 1e-12);
        assert!((c.daylight(0.75) - 0.6).abs() < 1e-12);
    }

    #[test]
    fn daylight_is_monotonic_bounded_and_smooth() {
        for (dawn, dusk) in [
            (0.1, 0.2),
            (0.5, 0.1),
            (0.1, 0.5),
            (0.49, 1e-2),
            (0.5, 0.5f64),
        ] {
            let text = TEXT
                .replace("dawn_width = 0.1", &format!("dawn_width = {dawn:?}"))
                .replace("dusk_width = 0.2", &format!("dusk_width = {dusk:?}"));
            let c = DayClock::from_toml(&text).unwrap();
            assert_eq!(c.daylight(0.5), 1.0, "noon {dawn} {dusk}");
            let n = 10_000;
            let mut prev = c.daylight(0.0);
            for i in 1..=n {
                let t = i as f64 / n as f64;
                let d = c.daylight(t);
                assert!((0.2..=1.0).contains(&d), "{t}");
                assert!(
                    (d - prev).abs() < 0.01 + 1.2 / (f64::min(dawn, dusk) * n as f64),
                    "{t}"
                );
                let lo: f64 = 0.25 - dawn / 2.0;
                let hi = 0.25 + dawn / 2.0;
                if t - 1.0 / n as f64 >= lo.max(0.0) && t <= hi {
                    assert!(d >= prev, "dawn {t}");
                }
                let (lo, hi) = (0.75 - dusk / 2.0, 0.75 + dusk / 2.0);
                if t - 1.0 / n as f64 >= lo && t <= hi.min(1.0) {
                    assert!(d <= prev, "dusk {t}");
                }
                prev = d;
            }
        }
    }

    #[test]
    fn daylight_tick_form_goes_through_time_of_day() {
        let c = DayClock::from_toml(TEXT).unwrap();
        for tick in [0, 17, 100, 12345, u64::MAX] {
            assert_eq!(c.daylight_at_tick(tick), c.daylight(c.time_of_day(tick)));
        }
    }

    #[test]
    fn bad_daylight_data_is_an_error() {
        let head = "length_seconds = 5\nstart_fraction = 0.5\nsun_tilt_degrees = 10.0\nsun_azimuth_degrees = 0.0\n";
        let case = |m: &str, d: &str, s: &str| {
            DayClock::from_toml(&format!(
                "{head}min_daylight = {m}\ndawn_width = {d}\ndusk_width = {s}\n"
            ))
        };
        assert!(case("0.0", "0.1", "0.1").is_ok());
        assert!(case("1.0", "0.5", "0.5").is_ok());
        for (m, d, s) in [
            ("-0.1", "0.1", "0.1"),
            ("1.1", "0.1", "0.1"),
            ("nan", "0.1", "0.1"),
            ("0.2", "0.0", "0.1"),
            ("0.2", "0.1", "-0.1"),
            ("0.2", "0.1", "nan"),
            ("0.2", "0.6", "0.5"),
            ("0.2", "0.6", "0.1"),
            ("0.2", "0.1", "0.6"),
        ] {
            let e = case(m, d, s).unwrap_err();
            assert!(matches!(e.kind, DataErrorKind::Invalid(_)), "{m} {d} {s}");
        }
        let e = DayClock::from_toml(head).unwrap_err();
        assert_eq!(e.kind, DataErrorKind::MissingKey("min_daylight".into()));
    }
}
