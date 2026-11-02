//! Rod numbers loaded from `data/fishing.toml`.
//! See docs/design/fish-and-fishing.md sections 4.2 to 4.4.

use crate::data_error::{DataError, DataErrorKind, DataFile};

const DEFAULT_FISHING: &str = include_str!("../../../data/fishing.toml");

#[derive(Clone, Debug, PartialEq)]
pub struct FishingConfig {
    /// Bobber launch speed along the look direction, blocks per second.
    pub cast_speed: f64,
    /// Extra upward launch speed, blocks per second.
    pub cast_upward_kick: f64,
    /// Blocks from the player beyond which the bobber is reeled in.
    pub max_distance: f64,
    /// Inclusive range of ticks before a bite.
    pub bite_delay_ticks: [u32; 2],
    /// Ticks the reel window stays open after a bite.
    pub reel_window_ticks: u32,
    /// Blocks from the bobber within which swimming fish are considered.
    pub near_radius: f64,
}

const KEYS: [&str; 6] = [
    "cast_speed",
    "cast_upward_kick",
    "max_distance",
    "bite_delay_ticks",
    "reel_window_ticks",
    "near_radius",
];

fn err(kind: DataErrorKind) -> DataError {
    DataError::new(DataFile::Fishing, kind)
}

fn invalid(msg: String) -> DataError {
    err(DataErrorKind::Invalid(msg))
}

fn get<'a>(root: &'a toml::value::Table, key: &str) -> Result<&'a toml::Value, DataError> {
    root.get(key)
        .ok_or_else(|| err(DataErrorKind::MissingKey(key.into())))
}

fn count(v: &toml::Value) -> Option<u32> {
    u32::try_from(v.as_integer()?).ok().filter(|n| *n > 0)
}

fn whole(root: &toml::value::Table, key: &str) -> Result<u32, DataError> {
    count(get(root, key)?)
        .ok_or_else(|| invalid(format!("`{key}` must be a whole number above zero")))
}

fn amount(root: &toml::value::Table, key: &str) -> Result<f64, DataError> {
    let v = get(root, key)?;
    v.as_float()
        .or_else(|| v.as_integer().map(|i| i as f64))
        .filter(|n| n.is_finite() && *n > 0.0)
        .ok_or_else(|| invalid(format!("`{key}` must be above zero")))
}

impl FishingConfig {
    /// The shipped `data/fishing.toml`.
    pub fn default_from_data() -> Result<FishingConfig, DataError> {
        FishingConfig::parse(DEFAULT_FISHING)
    }

    pub fn parse(text: &str) -> Result<FishingConfig, DataError> {
        let root: toml::value::Table = toml::from_str(text)
            .map_err(|e: toml::de::Error| err(DataErrorKind::Parse(e.to_string())))?;
        if let Some(key) = root.keys().find(|k| !KEYS.contains(&k.as_str())) {
            return Err(invalid(format!("unknown key `{key}`")));
        }
        let bite_delay_ticks = get(&root, "bite_delay_ticks")?
            .as_array()
            .filter(|a| a.len() == 2)
            .and_then(|a| Some([count(&a[0])?, count(&a[1])?]))
            .filter(|[lo, hi]| lo <= hi)
            .ok_or_else(|| {
                invalid("`bite_delay_ticks` must be [min, max], 1 <= min <= max".into())
            })?;
        Ok(FishingConfig {
            cast_speed: amount(&root, "cast_speed")?,
            cast_upward_kick: amount(&root, "cast_upward_kick")?,
            max_distance: amount(&root, "max_distance")?,
            bite_delay_ticks,
            reel_window_ticks: whole(&root, "reel_window_ticks")?,
            near_radius: amount(&root, "near_radius")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_error::test_support::*;

    fn parse(text: &str) -> Result<FishingConfig, DataError> {
        FishingConfig::parse(text)
    }

    #[test]
    fn shipped_file_has_design_defaults() {
        let c = FishingConfig::default_from_data().unwrap();
        assert_eq!(c.max_distance, 24.0);
        assert_eq!(c.bite_delay_ticks, [60, 160]);
        assert_eq!(c.reel_window_ticks, 20);
        assert!(c.cast_speed > 0.0 && c.cast_upward_kick > 0.0 && c.near_radius > 0.0);
    }

    #[test]
    fn rejects_missing_keys() {
        for key in KEYS {
            let text: String = DEFAULT_FISHING
                .lines()
                .filter(|l| !l.starts_with(&format!("{key} ")))
                .map(|l| format!("{l}\n"))
                .collect();
            assert_missing_key(parse(&text).unwrap_err(), DataFile::Fishing, key);
        }
    }

    #[test]
    fn rejects_bad_values() {
        let cases = [
            ("cast_speed = 12.0", "cast_speed = 0"),
            ("cast_speed = 12.0", "cast_speed = -1.0"),
            ("cast_upward_kick = 4.0", "cast_upward_kick = 0.0"),
            ("cast_upward_kick = 4.0", "cast_upward_kick = -4"),
            ("max_distance = 24", "max_distance = 0"),
            ("max_distance = 24", "max_distance = -24"),
            (
                "bite_delay_ticks = [60, 160]",
                "bite_delay_ticks = [0, 160]",
            ),
            (
                "bite_delay_ticks = [60, 160]",
                "bite_delay_ticks = [-5, 160]",
            ),
            (
                "bite_delay_ticks = [60, 160]",
                "bite_delay_ticks = [160, 60]",
            ),
            ("bite_delay_ticks = [60, 160]", "bite_delay_ticks = 60"),
            ("reel_window_ticks = 20", "reel_window_ticks = 0"),
            ("reel_window_ticks = 20", "reel_window_ticks = -20"),
            ("reel_window_ticks = 20", "reel_window_ticks = 1.5"),
            ("near_radius = 4", "near_radius = 0"),
            ("near_radius = 4", "near_radius = -4"),
        ];
        for (from, to) in cases {
            assert!(DEFAULT_FISHING.contains(from), "{from}");
            let key = from.split(' ').next().unwrap();
            let e = parse(&DEFAULT_FISHING.replace(from, to)).unwrap_err();
            assert_eq!(e.file, DataFile::Fishing);
            assert_err_contains(Err::<(), _>(e), &format!("`{key}` must be"));
        }
    }

    #[test]
    fn accepts_equal_range_ends() {
        let text = DEFAULT_FISHING.replace("[60, 160]", "[60, 60]");
        assert_eq!(parse(&text).unwrap().bite_delay_ticks, [60, 60]);
    }

    #[test]
    fn rejects_bad_shape() {
        assert_parse_error(parse("cast_speed =").unwrap_err(), DataFile::Fishing);
        let text = format!("{DEFAULT_FISHING}colour = 3\n");
        assert_err_contains(parse(&text), "unknown key `colour`");
    }
}
