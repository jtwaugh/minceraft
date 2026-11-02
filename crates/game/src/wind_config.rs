//! Wind tunables loaded from `data/wind.toml`.
//! See docs/design/wind.md Phase A.

use crate::data_error::{DataError, DataErrorKind, DataFile};
use crate::wind_exposure::ExposureParams;
use crate::wind_height::WindHeightParams;
use crate::wind_roughness_config::ROUGHNESS_KEYS;

const DEFAULT_WIND: &str = include_str!("../../../data/wind.toml");

#[derive(Clone, Debug, PartialEq)]
pub struct WindConfig {
    /// Feature size of the pressure field, blocks.
    pub front_size: f64,
    /// `(u, v)` drift of the pressure field, blocks per second.
    pub front_drift: [f64; 2],
    /// Blocks per second per unit gradient of the pressure field.
    pub speed_scale: f64,
    /// Feature size of the gust noise, blocks.
    pub gust_size: f64,
    /// Seconds for the gust noise to change by one feature.
    pub gust_time_scale: f64,
    /// `g` in `speed · (1 + g·gust)`.
    pub gust_strength: f64,
    /// Largest sideways push from gusts, as a fraction of the speed.
    pub direction_wobble: f64,
    /// Wind speed limits, blocks per second.
    pub speed_floor: f64,
    pub speed_ceiling: f64,
    /// Smoothed ground height lattice.
    pub height: WindHeightParams,
    /// Speed factor from height above the wide mean.
    pub exposure: ExposureParams,
}

const KEYS: [&str; 17] = [
    "front_size",
    "front_drift",
    "speed_scale",
    "gust_size",
    "gust_time_scale",
    "gust_strength",
    "direction_wobble",
    "speed_floor",
    "speed_ceiling",
    "height_cell",
    "height_step",
    "height_radius",
    "height_radius_wide",
    "exposure_gain",
    "exposure_floor",
    "exposure_ceiling",
    "exposure_ease_len",
];

fn err(kind: DataErrorKind) -> DataError {
    DataError::new(DataFile::Wind, kind)
}

fn invalid(msg: String) -> DataError {
    err(DataErrorKind::Invalid(msg))
}

fn get<'a>(root: &'a toml::value::Table, key: &str) -> Result<&'a toml::Value, DataError> {
    root.get(key)
        .ok_or_else(|| err(DataErrorKind::MissingKey(key.into())))
}

fn finite(v: &toml::Value) -> Option<f64> {
    v.as_float()
        .or_else(|| v.as_integer().map(|i| i as f64))
        .filter(|n| n.is_finite())
}

fn positive(root: &toml::value::Table, key: &str) -> Result<f64, DataError> {
    finite(get(root, key)?)
        .filter(|n| *n > 0.0)
        .ok_or_else(|| invalid(format!("`{key}` must be a finite number above zero")))
}

fn non_negative(root: &toml::value::Table, key: &str) -> Result<f64, DataError> {
    finite(get(root, key)?)
        .filter(|n| *n >= 0.0)
        .ok_or_else(|| invalid(format!("`{key}` must be a finite number, zero or above")))
}

/// A whole number of blocks that fits in `i32`, zero or above.
fn blocks(root: &toml::value::Table, key: &str) -> Result<i32, DataError> {
    get(root, key)?
        .as_integer()
        .and_then(|n| i32::try_from(n).ok())
        .filter(|n| *n >= 0)
        .ok_or_else(|| {
            invalid(format!(
                "`{key}` must be a whole number of blocks, zero or above"
            ))
        })
}

fn height_params(root: &toml::value::Table) -> Result<WindHeightParams, DataError> {
    let p = WindHeightParams {
        cell: blocks(root, "height_cell")?,
        step: blocks(root, "height_step")?,
        radius: blocks(root, "height_radius")?,
        radius_wide: blocks(root, "height_radius_wide")?,
    };
    if p.step == 0 || p.step % 4 != 0 {
        return Err(invalid("`height_step` must be a multiple of 4".into()));
    }
    if p.cell == 0 || p.cell % p.step != 0 {
        return Err(invalid(
            "`height_cell` must be a multiple of `height_step`".into(),
        ));
    }
    if p.radius % p.step != 0 {
        return Err(invalid(
            "`height_radius` must be a multiple of `height_step`".into(),
        ));
    }
    if p.radius_wide % p.step != 0 {
        return Err(invalid(
            "`height_radius_wide` must be a multiple of `height_step`".into(),
        ));
    }
    if p.radius_wide < p.radius {
        return Err(invalid("need height_radius <= height_radius_wide".into()));
    }
    Ok(p)
}

fn exposure_params(root: &toml::value::Table) -> Result<ExposureParams, DataError> {
    let p = ExposureParams {
        gain: positive(root, "exposure_gain")?,
        floor: positive(root, "exposure_floor")?,
        ceiling: positive(root, "exposure_ceiling")?,
        ease_len: non_negative(root, "exposure_ease_len")?,
    };
    if p.floor > p.ceiling {
        return Err(invalid("need exposure_floor <= exposure_ceiling".into()));
    }
    Ok(p)
}

impl WindConfig {
    /// The shipped `data/wind.toml`.
    pub fn default_from_data() -> Result<WindConfig, DataError> {
        WindConfig::parse(DEFAULT_WIND)
    }

    pub fn parse(text: &str) -> Result<WindConfig, DataError> {
        let root: toml::value::Table = toml::from_str(text)
            .map_err(|e: toml::de::Error| err(DataErrorKind::Parse(e.to_string())))?;
        if let Some(key) = root
            .keys()
            .find(|k| !KEYS.contains(&k.as_str()) && !ROUGHNESS_KEYS.contains(&k.as_str()))
        {
            return Err(invalid(format!("unknown key `{key}`")));
        }
        let front_drift = get(&root, "front_drift")?
            .as_array()
            .filter(|a| a.len() == 2)
            .and_then(|a| Some([finite(&a[0])?, finite(&a[1])?]))
            .ok_or_else(|| invalid("`front_drift` must be [u, v], both finite".into()))?;
        let config = WindConfig {
            front_size: positive(&root, "front_size")?,
            front_drift,
            speed_scale: positive(&root, "speed_scale")?,
            gust_size: positive(&root, "gust_size")?,
            gust_time_scale: positive(&root, "gust_time_scale")?,
            gust_strength: non_negative(&root, "gust_strength")?,
            direction_wobble: non_negative(&root, "direction_wobble")?,
            speed_floor: non_negative(&root, "speed_floor")?,
            speed_ceiling: positive(&root, "speed_ceiling")?,
            height: height_params(&root)?,
            exposure: exposure_params(&root)?,
        };
        if config.speed_floor > config.speed_ceiling {
            return Err(invalid("need speed_floor <= speed_ceiling".into()));
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_error::test_support::*;

    fn parse(text: &str) -> Result<WindConfig, DataError> {
        WindConfig::parse(text)
    }

    #[test]
    fn shipped_file_has_design_defaults() {
        let c = WindConfig::default_from_data().unwrap();
        // Fronts are hundreds to a few thousand blocks and take minutes to pass.
        assert!((100.0..=5000.0).contains(&c.front_size));
        let drift = c.front_drift[0].hypot(c.front_drift[1]);
        assert!(drift > 0.0);
        assert!(
            c.front_size / drift >= 60.0,
            "front passes in under a minute"
        );
        assert!(c.gust_size < c.front_size);
        assert!(c.speed_scale > 0.0 && c.gust_time_scale > 0.0);
        assert!(c.gust_strength >= 0.0 && c.direction_wobble >= 0.0);
        assert!(c.speed_floor <= c.speed_ceiling);
    }

    #[test]
    fn rejects_missing_keys() {
        for key in KEYS {
            let text: String = DEFAULT_WIND
                .lines()
                .filter(|l| !l.starts_with(&format!("{key} ")))
                .map(|l| format!("{l}\n"))
                .collect();
            assert_missing_key(parse(&text).unwrap_err(), DataFile::Wind, key);
        }
    }

    #[test]
    fn rejects_invalid_values() {
        let cases = [
            ("front_size = 1500", "front_size = 0"),
            ("front_size = 1500", "front_size = -1500"),
            ("front_size = 1500", "front_size = inf"),
            ("front_size = 1500", "front_size = nan"),
            ("front_size = 1500", "front_size = \"big\""),
            ("front_drift = [4.0, 1.5]", "front_drift = [4.0]"),
            ("front_drift = [4.0, 1.5]", "front_drift = [4.0, nan]"),
            ("front_drift = [4.0, 1.5]", "front_drift = [-inf, 1.5]"),
            ("front_drift = [4.0, 1.5]", "front_drift = 4.0"),
            ("speed_scale = 6.0", "speed_scale = 0.0"),
            ("gust_size = 48", "gust_size = -48"),
            ("gust_time_scale = 3.0", "gust_time_scale = 0"),
            ("gust_strength = 0.5", "gust_strength = -0.5"),
            ("gust_strength = 0.5", "gust_strength = inf"),
            ("direction_wobble = 0.2", "direction_wobble = -0.2"),
            ("speed_floor = 0.0", "speed_floor = -1.0"),
            ("speed_floor = 0.0", "speed_floor = nan"),
            ("speed_ceiling = 12.0", "speed_ceiling = 0"),
            ("speed_ceiling = 12.0", "speed_ceiling = inf"),
        ];
        for (from, to) in cases {
            assert!(DEFAULT_WIND.contains(from), "{from}");
            let key = from.split(' ').next().unwrap();
            let e = parse(&DEFAULT_WIND.replace(from, to)).unwrap_err();
            assert_eq!(e.file, DataFile::Wind);
            assert!(e.to_string().starts_with("wind.toml: "), "{e}");
            assert_err_contains(Err::<(), _>(e), &format!("`{key}` must be"));
        }
    }

    #[test]
    fn shipped_terrain_keys_build_params() {
        let c = WindConfig::default_from_data().unwrap();
        assert_eq!(
            c.height,
            WindHeightParams {
                cell: 64,
                step: 8,
                radius: 16,
                radius_wide: 48
            }
        );
        assert_eq!(c.exposure.gain, 0.02);
        assert!(c.exposure.floor < 1.0 && 1.0 < c.exposure.ceiling);
    }

    #[test]
    fn rejects_bad_terrain_values() {
        let cases = [
            (
                "height_cell = 64",
                "height_cell = 60",
                "`height_cell` must be a multiple",
            ),
            (
                "height_cell = 64",
                "height_cell = 0",
                "`height_cell` must be a multiple",
            ),
            (
                "height_cell = 64",
                "height_cell = 64.0",
                "`height_cell` must be a whole",
            ),
            (
                "height_cell = 64",
                "height_cell = -64",
                "`height_cell` must be a whole",
            ),
            (
                "height_step = 8",
                "height_step = 6",
                "`height_step` must be a multiple of 4",
            ),
            (
                "height_step = 8",
                "height_step = 0",
                "`height_step` must be a multiple of 4",
            ),
            (
                "height_radius = 16",
                "height_radius = 12",
                "`height_radius` must be a multiple",
            ),
            (
                "height_radius_wide = 48",
                "height_radius_wide = 44",
                "`height_radius_wide` must be a multiple",
            ),
            (
                "height_radius_wide = 48",
                "height_radius_wide = 8",
                "need height_radius <= height_radius_wide",
            ),
            (
                "exposure_gain = 0.02",
                "exposure_gain = 0",
                "`exposure_gain` must be",
            ),
            (
                "exposure_gain = 0.02",
                "exposure_gain = inf",
                "`exposure_gain` must be",
            ),
            (
                "exposure_floor = 0.6",
                "exposure_floor = nan",
                "`exposure_floor` must be",
            ),
            (
                "exposure_ceiling = 1.5",
                "exposure_ceiling = -1",
                "`exposure_ceiling` must be",
            ),
            (
                "exposure_ceiling = 1.5",
                "exposure_ceiling = 0.5",
                "need exposure_floor <= exposure_ceiling",
            ),
            (
                "exposure_ease_len = 4.0",
                "exposure_ease_len = -1",
                "`exposure_ease_len` must be",
            ),
            (
                "exposure_ease_len = 4.0",
                "exposure_ease_len = inf",
                "`exposure_ease_len` must be",
            ),
        ];
        for (from, to, msg) in cases {
            assert!(DEFAULT_WIND.contains(from), "{from}");
            assert_err_contains(parse(&DEFAULT_WIND.replace(from, to)), msg);
        }
    }

    #[test]
    fn negative_drift_is_allowed() {
        let text = DEFAULT_WIND.replace("front_drift = [4.0, 1.5]", "front_drift = [-4, 0]");
        assert_eq!(parse(&text).unwrap().front_drift, [-4.0, 0.0]);
    }

    #[test]
    fn rejects_floor_above_ceiling() {
        let text = DEFAULT_WIND.replace("speed_floor = 0.0", "speed_floor = 13.0");
        assert_err_contains(parse(&text), "need speed_floor <= speed_ceiling");
        let text = DEFAULT_WIND.replace("speed_floor = 0.0", "speed_floor = 12.0");
        assert_eq!(parse(&text).unwrap().speed_floor, 12.0);
    }

    #[test]
    fn rejects_bad_shape() {
        assert_parse_error(parse("front_size =").unwrap_err(), DataFile::Wind);
        let text = format!("colour = 3\n{DEFAULT_WIND}");
        assert_err_contains(parse(&text), "unknown key `colour`");
    }
}
