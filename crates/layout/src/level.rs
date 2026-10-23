//! Level file (docs/design/world-layout.md 4.2, 4.3): `Level`, `WorldConfig`
//! and a TOML loader and writer. `[route]` and `[climate]` are optional;
//! `[climate]` records the temperature bias the levels were built with (#846).

#[cfg(feature = "toml-io")]
use std::collections::HashSet;
use std::fmt;
#[cfg(feature = "toml-io")]
use std::path::Path;

use terrain::SEA_LEVEL;

/// Per-level metadata for the game.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "toml-io", derive(serde::Serialize, serde::Deserialize))]
pub struct Level {
    pub id: String,
    pub week: u32,
    /// Where the raft sits (world x, z).
    pub anchorage: [i32; 2],
    pub island_center: [i32; 2],
    pub spawn_water_radius: f64,
}

/// The `[world]` table. `sea_level` is not stored: it must equal
/// `terrain::SEA_LEVEL` and is written back as that.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldConfig {
    pub seed: u64,
}

/// The `[route] points` table: one (x, z) per level, the first being level 1's
/// anchorage.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "toml-io", derive(serde::Serialize, serde::Deserialize))]
pub struct Route {
    pub points: Vec<[i32; 2]>,
}

/// The `[climate]` table: the `AlongPath` temperature offsets at the first and
/// last anchorage. A record of what the bias used, not an input.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "toml-io", derive(serde::Serialize, serde::Deserialize))]
pub struct Climate {
    pub temp_start: f64,
    pub temp_end: f64,
}

/// A loaded level file.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelFile {
    pub world: WorldConfig,
    pub levels: Vec<Level>,
    pub route: Option<Route>,
    pub climate: Option<Climate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LevelFileError {
    /// Malformed TOML, or a missing or mistyped field (the message names it).
    Parse(String),
    /// `[world] sea_level` differs from `terrain::SEA_LEVEL`.
    SeaLevelMismatch {
        found: i64,
    },
    DuplicateLevelId(String),
    /// `[route]` has a different number of points than there are levels.
    RoutePointCount {
        points: usize,
        levels: usize,
    },
    /// The first `[route]` point is not level 1's anchorage.
    RouteStartMismatch {
        found: [i32; 2],
        anchorage: [i32; 2],
    },
    /// Writing failed (e.g. a seed above `i64::MAX` is not a TOML integer).
    Write(String),
}

impl fmt::Display for LevelFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(m) => write!(f, "invalid level file: {m}"),
            Self::SeaLevelMismatch { found } => {
                write!(f, "sea_level is {found} but must be {SEA_LEVEL}")
            }
            Self::DuplicateLevelId(id) => write!(f, "duplicate level id `{id}`"),
            Self::RoutePointCount { points, levels } => {
                write!(
                    f,
                    "route has {points} points but the file has {levels} levels"
                )
            }
            Self::RouteStartMismatch { found, anchorage } => write!(
                f,
                "route starts at {found:?} but level 1's anchorage is {anchorage:?}"
            ),
            Self::Write(m) => write!(f, "cannot write level file: {m}"),
        }
    }
}

impl std::error::Error for LevelFileError {}

#[cfg(feature = "toml-io")]
#[derive(serde::Serialize, serde::Deserialize)]
struct RawWorld {
    seed: u64,
    sea_level: i64,
}

#[cfg(feature = "toml-io")]
#[derive(serde::Serialize, serde::Deserialize)]
struct Raw {
    world: RawWorld,
    #[serde(default, rename = "level")]
    levels: Vec<Level>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    route: Option<Route>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    climate: Option<Climate>,
}

#[cfg(feature = "toml-io")]
fn check_route(r: &Route, levels: &[Level]) -> Result<(), LevelFileError> {
    if r.points.len() != levels.len() {
        return Err(LevelFileError::RoutePointCount {
            points: r.points.len(),
            levels: levels.len(),
        });
    }
    if let (Some(&found), Some(l)) = (r.points.first(), levels.first()) {
        if found != l.anchorage {
            return Err(LevelFileError::RouteStartMismatch {
                found,
                anchorage: l.anchorage,
            });
        }
    }
    Ok(())
}

/// Reading and writing level files needs the `toml-io` feature.
#[cfg(feature = "toml-io")]
impl LevelFile {
    pub fn from_toml(text: &str) -> Result<Self, LevelFileError> {
        let raw: Raw = toml::from_str(text).map_err(|e| LevelFileError::Parse(e.to_string()))?;
        if raw.world.sea_level != SEA_LEVEL as i64 {
            return Err(LevelFileError::SeaLevelMismatch {
                found: raw.world.sea_level,
            });
        }
        let mut seen = HashSet::new();
        for l in &raw.levels {
            if !seen.insert(l.id.as_str()) {
                return Err(LevelFileError::DuplicateLevelId(l.id.clone()));
            }
        }
        if let Some(r) = &raw.route {
            check_route(r, &raw.levels)?;
        }
        Ok(Self {
            world: WorldConfig {
                seed: raw.world.seed,
            },
            levels: raw.levels,
            route: raw.route,
            climate: raw.climate,
        })
    }

    pub fn to_toml(&self) -> Result<String, LevelFileError> {
        if let Some(r) = &self.route {
            check_route(r, &self.levels)?;
        }
        let raw = Raw {
            world: RawWorld {
                seed: self.world.seed,
                sea_level: SEA_LEVEL as i64,
            },
            levels: self.levels.clone(),
            route: self.route.clone(),
            climate: self.climate,
        };
        toml::to_string(&raw).map_err(|e| LevelFileError::Write(e.to_string()))
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, LevelFileError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| LevelFileError::Parse(format!("{}: {e}", path.display())))?;
        Self::from_toml(&text)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), LevelFileError> {
        std::fs::write(path, self.to_toml()?).map_err(|e| LevelFileError::Write(e.to_string()))
    }
}
