//! Level file (docs/design/world-layout.md 4.2, 4.3): `Level`, `WorldConfig`
//! and a TOML loader and writer. `[route]` and `[climate]` belong to later
//! phases and are ignored.

use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use terrain::SEA_LEVEL;

/// Per-level metadata for the game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

/// A loaded level file.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelFile {
    pub world: WorldConfig,
    pub levels: Vec<Level>,
}

#[derive(Debug)]
pub enum LevelFileError {
    /// Malformed TOML, or a missing or mistyped field (the message names it).
    Parse(String),
    /// `[world] sea_level` differs from `terrain::SEA_LEVEL`.
    SeaLevelMismatch {
        found: i64,
    },
    DuplicateLevelId(String),
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
            Self::Write(m) => write!(f, "cannot write level file: {m}"),
        }
    }
}

impl std::error::Error for LevelFileError {}

#[derive(Serialize, Deserialize)]
struct RawWorld {
    seed: u64,
    sea_level: i64,
}

// Unknown top-level tables (`[route]`, `[climate]`) are ignored by serde.
#[derive(Serialize, Deserialize)]
struct Raw {
    world: RawWorld,
    #[serde(default, rename = "level")]
    levels: Vec<Level>,
}

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
        Ok(Self {
            world: WorldConfig {
                seed: raw.world.seed,
            },
            levels: raw.levels,
        })
    }

    pub fn to_toml(&self) -> Result<String, LevelFileError> {
        let raw = Raw {
            world: RawWorld {
                seed: self.world.seed,
                sea_level: SEA_LEVEL as i64,
            },
            levels: self.levels.clone(),
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
