//! Command line and output for `mince-seeds` (docs/design/world-layout.md
//! Phase F, stage 4; 4.3): argument parsing and the level file text for the
//! best seed, with the scores as `#` comment lines.

use crate::LevelClimate;
use crate::{
    LevelFile, LevelFileError, LevelRoute, RouteParams, Scored, SearchParams, WorldConfig,
};

pub const USAGE: &str =
    "usage: mince-seeds --levels N --candidates N --top N --out PATH [--start SEED]\n  \
--levels N      levels in the route (N >= 1)\n  \
--candidates N  seeds to try, start..start+N (N >= 1)\n  \
--top N         seeds to report (N >= 1)\n  \
--out PATH      level file to write\n  \
--start SEED    first seed (default 0)";

/// Parsed command line.
#[derive(Clone, Debug, PartialEq)]
pub struct Args {
    pub levels: usize,
    pub candidates: u64,
    pub top: usize,
    pub out: String,
    pub start: u64,
}

impl Args {
    /// Parse the arguments after the program name.
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Self, String> {
        let (mut levels, mut candidates, mut top, mut out, mut start) =
            (None, None, None, None, 0u64);
        let mut it = args.into_iter();
        while let Some(flag) = it.next() {
            let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
            let num = |v: &str| {
                v.parse::<u64>()
                    .map_err(|_| format!("{flag}: `{v}` is not a non-negative integer"))
            };
            match flag.as_str() {
                "--levels" => levels = Some(num(&value)?),
                "--candidates" => candidates = Some(num(&value)?),
                "--top" => top = Some(num(&value)?),
                "--start" => start = num(&value)?,
                "--out" => out = Some(value),
                _ => return Err(format!("unknown argument `{flag}`")),
            }
        }
        let need = |v: Option<u64>, name: &str| match v {
            Some(n) if n >= 1 => Ok(n),
            Some(_) => Err(format!("--{name} must be at least 1")),
            None => Err(format!("missing --{name}")),
        };
        Ok(Self {
            levels: need(levels, "levels")? as usize,
            candidates: need(candidates, "candidates")?,
            top: need(top, "top")? as usize,
            out: out.ok_or("missing --out")?,
            start,
        })
    }

    pub fn search_params(&self) -> SearchParams {
        SearchParams {
            start: self.start,
            candidates: self.candidates,
            top: self.top,
            k: self.levels - 1,
            workers: None,
        }
    }
}

/// The level file for `best`; `[climate]` records the shipped bias.
pub fn level_file(best: &Scored) -> LevelFile {
    let params = RouteParams::shipped();
    LevelFile {
        world: WorldConfig { seed: best.seed },
        levels: best.levels.clone(),
        route: Some(LevelRoute {
            points: best.levels.iter().map(|l| l.anchorage).collect(),
        }),
        climate: Some(LevelClimate {
            temp_start: params.temp_start,
            temp_end: params.temp_end,
        }),
    }
}

/// The file text: comment lines (seed, total and per-term scores), then the
/// TOML.
pub fn render(best: &Scored) -> Result<String, LevelFileError> {
    let t = &best.terms;
    Ok(format!(
        "# mince-seeds: seed {}\n# score {:.4}\n# spacing_regularity {:.4}\n\
         # island_quality {:.4}\n# coastline_interest {:.4}\n# extra_islands {:.4}\n\
         # climate_gradient {:.4}\n# biome_variety {:.4}\n{}",
        best.seed,
        best.score,
        t.spacing_regularity,
        t.island_quality,
        t.coastline_interest,
        t.extra_islands,
        t.climate_gradient,
        t.biome_variety,
        level_file(best).to_toml()?
    ))
}
