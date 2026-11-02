//! Roughness tunables from the `[roughness]` table and drag keys of
//! `data/wind.toml`. See docs/design/biome-roughness-wind.md Phase A.

use crate::data_error::{DataError, DataErrorKind, DataFile};
use crate::wind_roughness::RoughnessParams;

const DEFAULT_WIND: &str = include_str!("../../../data/wind.toml");

/// Top-level keys and the table this file owns; `WindConfig` must accept them.
pub(crate) const ROUGHNESS_KEYS: [&str; 8] = [
    "roughness",
    "roughness_drag",
    "roughness_floor",
    "blend_height",
    "gust_damping",
    "roughness_cell_size",
    "roughness_step",
    "roughness_radius",
];

#[derive(Clone, Debug, PartialEq)]
pub struct RoughnessConfig {
    /// Roughness per biome, ordered like `terrain::BIOME_INFO`.
    pub biome: Vec<f64>,
    pub water: f64,
    /// How much full roughness slows the wind, 0..1.
    pub roughness_drag: f64,
    /// Least speed multiplier, 0..1.
    pub roughness_floor: f64,
    /// Blocks over which roughness fades out with height.
    pub blend_height: f64,
    /// Extra gust damping by roughness, 0..1.
    pub gust_damping: f64,
    pub cell_size: i32,
    pub step: i32,
    pub radius: i32,
}

fn invalid(msg: String) -> DataError {
    DataError::new(DataFile::Wind, DataErrorKind::Invalid(msg))
}

fn get<'a>(root: &'a toml::value::Table, key: &str) -> Result<&'a toml::Value, DataError> {
    root.get(key)
        .ok_or_else(|| DataError::new(DataFile::Wind, DataErrorKind::MissingKey(key.into())))
}

fn number(v: &toml::Value) -> Option<f64> {
    v.as_float()
        .or_else(|| v.as_integer().map(|i| i as f64))
        .filter(|n| n.is_finite())
}

fn unit(v: &toml::Value, name: &str) -> Result<f64, DataError> {
    number(v)
        .filter(|n| (0.0..=1.0).contains(n))
        .ok_or_else(|| invalid(format!("`{name}` must be a finite number from 0 to 1")))
}

fn blocks(root: &toml::value::Table, key: &str) -> Result<i32, DataError> {
    get(root, key)?
        .as_integer()
        .and_then(|i| i32::try_from(i).ok())
        .ok_or_else(|| invalid(format!("`{key}` must be a whole number of blocks")))
}

impl RoughnessConfig {
    /// The shipped `data/wind.toml`.
    pub fn default_from_data() -> Result<RoughnessConfig, DataError> {
        RoughnessConfig::parse(DEFAULT_WIND)
    }

    pub fn parse(text: &str) -> Result<RoughnessConfig, DataError> {
        let root: toml::value::Table = toml::from_str(text).map_err(|e: toml::de::Error| {
            DataError::new(DataFile::Wind, DataErrorKind::Parse(e.to_string()))
        })?;
        let table = get(&root, "roughness")?
            .as_table()
            .ok_or_else(|| invalid("`roughness` must be a table".into()))?;
        if let Some(name) = table
            .keys()
            .find(|k| *k != "water" && !terrain::BIOME_INFO.iter().any(|b| b.name == k.as_str()))
        {
            return Err(invalid(format!("`roughness` names unknown biome `{name}`")));
        }
        let entry = |name: &str| unit(get(table, name)?, name);
        let biome = terrain::BIOME_INFO
            .iter()
            .map(|b| entry(b.name))
            .collect::<Result<Vec<_>, _>>()?;
        let blend_height = number(get(&root, "blend_height")?)
            .filter(|n| *n > 0.0)
            .ok_or_else(|| invalid("`blend_height` must be a finite number above zero".into()))?;
        let config = RoughnessConfig {
            biome,
            water: entry("water")?,
            roughness_drag: unit(get(&root, "roughness_drag")?, "roughness_drag")?,
            roughness_floor: unit(get(&root, "roughness_floor")?, "roughness_floor")?,
            blend_height,
            gust_damping: unit(get(&root, "gust_damping")?, "gust_damping")?,
            cell_size: blocks(&root, "roughness_cell_size")?,
            step: blocks(&root, "roughness_step")?,
            radius: blocks(&root, "roughness_radius")?,
        };
        if config.step <= 0 || config.step % 4 != 0 {
            return Err(invalid(
                "`roughness_step` must be a positive multiple of 4".into(),
            ));
        }
        if config.cell_size <= 0 || config.cell_size % config.step != 0 {
            return Err(invalid(
                "`roughness_cell_size` must be a positive multiple of `roughness_step`".into(),
            ));
        }
        if config.radius < 0 || config.radius % config.step != 0 {
            return Err(invalid(
                "`roughness_radius` must be a multiple of `roughness_step`".into(),
            ));
        }
        Ok(config)
    }

    pub fn params(&self) -> RoughnessParams {
        RoughnessParams {
            biome: self.biome.clone(),
            water: self.water,
            cell_size: self.cell_size,
            step: self.step,
            radius: self.radius,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_error::test_support::*;
    use crate::wind_config::WindConfig;

    fn parse(text: &str) -> Result<RoughnessConfig, DataError> {
        RoughnessConfig::parse(text)
    }

    fn index(name: &str) -> usize {
        terrain::BIOME_INFO
            .iter()
            .position(|b| b.name == name)
            .unwrap()
    }

    #[test]
    fn shipped_file_parses_and_follows_the_guide() {
        let c = RoughnessConfig::default_from_data().unwrap();
        assert!(WindConfig::default_from_data().is_ok());
        assert_eq!(c.biome.len(), terrain::BIOME_INFO.len());
        assert_eq!(c.gust_damping, 0.0);
        let r = |n: &str| c.biome[index(n)];
        assert!(r("Rainforest") > r("Plains"));
        assert!(r("Forest") > r("Plains"));
        assert!(r("Plains") > r("Desert"));
        assert!(r("Desert") > c.water);
        assert!(r("Ice Desert") > c.water);
        let p = c.params();
        assert_eq!(p.biome, c.biome);
        assert_eq!(p.window_samples() as i32, p.window_size() / p.step + 1);
    }

    #[test]
    fn every_biome_has_an_entry() {
        for b in terrain::BIOME_INFO.iter() {
            let line = format!("\"{}\" = ", b.name);
            assert!(DEFAULT_WIND.contains(&line), "no roughness for {}", b.name);
            let text: String = DEFAULT_WIND
                .lines()
                .filter(|l| !l.starts_with(&line))
                .map(|l| format!("{l}\n"))
                .collect();
            assert_missing_key(parse(&text).unwrap_err(), DataFile::Wind, b.name);
        }
    }

    #[test]
    fn rejects_missing_keys() {
        for key in ROUGHNESS_KEYS.iter().skip(1).chain(&["water"]) {
            let text: String = DEFAULT_WIND
                .lines()
                .filter(|l| !l.starts_with(&format!("{key} ")))
                .map(|l| format!("{l}\n"))
                .collect();
            assert_missing_key(parse(&text).unwrap_err(), DataFile::Wind, key);
        }
    }

    #[test]
    fn rejects_unknown_biome() {
        let text = format!("{DEFAULT_WIND}\"Moonbase\" = 0.5\n");
        assert_err_contains(parse(&text), "unknown biome `Moonbase`");
    }

    #[test]
    fn rejects_invalid_values() {
        let cases = [
            ("\"Forest\" = 0.8", "\"Forest\" = 1.5", "Forest"),
            ("\"Forest\" = 0.8", "\"Forest\" = -0.1", "Forest"),
            ("\"Forest\" = 0.8", "\"Forest\" = nan", "Forest"),
            ("water = 0.02", "water = inf", "water"),
            (
                "roughness_drag = 0.5",
                "roughness_drag = 2",
                "roughness_drag",
            ),
            (
                "roughness_floor = 0.3",
                "roughness_floor = -1",
                "roughness_floor",
            ),
            ("gust_damping = 0.0", "gust_damping = 1.5", "gust_damping"),
            ("blend_height = 12", "blend_height = 0", "blend_height"),
            ("roughness_step = 8", "roughness_step = 6", "roughness_step"),
            (
                "roughness_cell_size = 64",
                "roughness_cell_size = 60",
                "roughness_cell_size",
            ),
            (
                "roughness_radius = 32",
                "roughness_radius = 12",
                "roughness_radius",
            ),
        ];
        for (from, to, key) in cases {
            assert!(DEFAULT_WIND.contains(from), "{from}");
            let e = parse(&DEFAULT_WIND.replace(from, to)).unwrap_err();
            assert_eq!(e.file, DataFile::Wind);
            assert_err_contains(Err::<(), _>(e), &format!("`{key}` must be"));
        }
    }
}
