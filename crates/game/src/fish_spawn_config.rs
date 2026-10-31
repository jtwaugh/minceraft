//! Fish stock, spawn ring and swim numbers loaded from `data/fish_spawn.toml`.
//! See docs/design/fish-and-fishing.md sections 3.1 and 3.2.

use crate::data_error::{DataError, DataErrorKind, DataFile};

const DEFAULT_FISH_SPAWN: &str = include_str!("../../../data/fish_spawn.toml");

#[derive(Clone, Debug, PartialEq)]
pub struct FishSpawnConfig {
    /// Fish per anchorage per day.
    pub daily_stock: u32,
    /// Fish the spawner tops up to at a phase change, within `target_radius`.
    pub target_count: u32,
    /// Blocks from the raft.
    pub target_radius: f64,
    /// Spawn ring, blocks from the raft.
    pub ring_inner: f64,
    pub ring_outer: f64,
    /// Fish farther than this many blocks are removed.
    pub despawn_radius: f64,
    /// Blocks kept below the surface and above the bottom.
    pub clearance: f64,
    /// Blocks from a swimming player at which fish scatter.
    pub scatter_distance: f64,
    /// Swim speed multiplier while scattering.
    pub scatter_speed: f64,
    /// Inclusive range of ticks before the wander picks a new heading.
    pub retarget_ticks: [u32; 2],
}

const KEYS: [&str; 10] = [
    "daily_stock",
    "target_count",
    "target_radius",
    "ring_inner",
    "ring_outer",
    "despawn_radius",
    "clearance",
    "scatter_distance",
    "scatter_speed",
    "retarget_ticks",
];

fn err(kind: DataErrorKind) -> DataError {
    DataError::new(DataFile::FishSpawn, kind)
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

fn blocks(root: &toml::value::Table, key: &str) -> Result<f64, DataError> {
    let v = get(root, key)?;
    v.as_float()
        .or_else(|| v.as_integer().map(|i| i as f64))
        .filter(|n| n.is_finite() && *n > 0.0)
        .ok_or_else(|| invalid(format!("`{key}` must be above zero")))
}

impl FishSpawnConfig {
    /// The shipped `data/fish_spawn.toml`.
    pub fn default_from_data() -> Result<FishSpawnConfig, DataError> {
        FishSpawnConfig::parse(DEFAULT_FISH_SPAWN)
    }

    pub fn parse(text: &str) -> Result<FishSpawnConfig, DataError> {
        let root: toml::value::Table = toml::from_str(text)
            .map_err(|e: toml::de::Error| err(DataErrorKind::Parse(e.to_string())))?;
        if let Some(key) = root.keys().find(|k| !KEYS.contains(&k.as_str())) {
            return Err(invalid(format!("unknown key `{key}`")));
        }
        let retarget_ticks = get(&root, "retarget_ticks")?
            .as_array()
            .filter(|a| a.len() == 2)
            .and_then(|a| Some([count(&a[0])?, count(&a[1])?]))
            .filter(|[lo, hi]| lo <= hi)
            .ok_or_else(|| {
                invalid("`retarget_ticks` must be [min, max], 1 <= min <= max".into())
            })?;
        let config = FishSpawnConfig {
            daily_stock: whole(&root, "daily_stock")?,
            target_count: whole(&root, "target_count")?,
            target_radius: blocks(&root, "target_radius")?,
            ring_inner: blocks(&root, "ring_inner")?,
            ring_outer: blocks(&root, "ring_outer")?,
            despawn_radius: blocks(&root, "despawn_radius")?,
            clearance: blocks(&root, "clearance")?,
            scatter_distance: blocks(&root, "scatter_distance")?,
            scatter_speed: blocks(&root, "scatter_speed")?,
            retarget_ticks,
        };
        if !(config.ring_inner < config.ring_outer && config.ring_outer < config.despawn_radius) {
            return Err(invalid(
                "need ring_inner < ring_outer < despawn_radius".into(),
            ));
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_error::test_support::*;

    fn parse(text: &str) -> Result<FishSpawnConfig, DataError> {
        FishSpawnConfig::parse(text)
    }

    #[test]
    fn shipped_file_has_design_defaults() {
        let c = FishSpawnConfig::default_from_data().unwrap();
        assert_eq!((c.daily_stock, c.target_count), (24, 12));
        assert_eq!(c.target_radius, 24.0);
        assert_eq!((c.ring_inner, c.ring_outer), (12.0, 24.0));
        assert_eq!(c.despawn_radius, 40.0);
        assert_eq!(c.clearance, 1.0);
        assert!(c.scatter_distance > 0.0 && c.scatter_speed > 0.0);
        assert!(c.retarget_ticks[0] <= c.retarget_ticks[1]);
    }

    #[test]
    fn rejects_missing_keys() {
        for key in KEYS {
            let text: String = DEFAULT_FISH_SPAWN
                .lines()
                .filter(|l| !l.starts_with(&format!("{key} ")))
                .map(|l| format!("{l}\n"))
                .collect();
            assert_missing_key(parse(&text).unwrap_err(), DataFile::FishSpawn, key);
        }
    }

    #[test]
    fn rejects_non_positive_values() {
        let cases = [
            ("daily_stock = 24", "daily_stock = 0"),
            ("target_count = 12", "target_count = -1"),
            ("target_count = 12", "target_count = 1.5"),
            ("target_radius = 24", "target_radius = 0"),
            ("ring_inner = 12", "ring_inner = -12"),
            ("clearance = 1", "clearance = 0.0"),
            ("scatter_distance = 4", "scatter_distance = 0"),
            ("scatter_speed = 2.0", "scatter_speed = -2.0"),
            ("retarget_ticks = [40, 100]", "retarget_ticks = [0, 100]"),
            ("retarget_ticks = [40, 100]", "retarget_ticks = [100, 40]"),
            ("retarget_ticks = [40, 100]", "retarget_ticks = 40"),
        ];
        for (from, to) in cases {
            assert!(DEFAULT_FISH_SPAWN.contains(from), "{from}");
            let key = from.split(' ').next().unwrap();
            let e = parse(&DEFAULT_FISH_SPAWN.replace(from, to)).unwrap_err();
            assert_eq!(e.file, DataFile::FishSpawn);
            assert_err_contains(Err::<(), _>(e), &format!("`{key}` must be"));
        }
    }

    #[test]
    fn rejects_ring_outside_despawn_radius() {
        let cases = [
            ("ring_inner = 12", "ring_inner = 24"),
            ("ring_inner = 12", "ring_inner = 30"),
            ("despawn_radius = 40", "despawn_radius = 24"),
            ("despawn_radius = 40", "despawn_radius = 20"),
        ];
        for (from, to) in cases {
            assert!(DEFAULT_FISH_SPAWN.contains(from), "{from}");
            assert_err_contains(
                parse(&DEFAULT_FISH_SPAWN.replace(from, to)),
                "ring_inner < ring_outer < despawn_radius",
            );
        }
    }

    #[test]
    fn rejects_bad_shape() {
        assert_parse_error(parse("daily_stock =").unwrap_err(), DataFile::FishSpawn);
        let text = format!("{DEFAULT_FISH_SPAWN}colour = 3\n");
        assert_err_contains(parse(&text), "unknown key `colour`");
    }
}
