//! Fish bodies loaded from `data/fish_bodies.toml`: box, swimming, school
//! size and the raw item a catch becomes. See docs/design/fish-and-fishing.md
//! section 2 and reconciliation note 6.
//!
//! Every species in `data/fish.toml` has exactly one body, and every raw item
//! is in `data/items.toml`.

use crate::data_error::{DataError, DataErrorKind, DataFile};
use crate::items::Items;
use layout::Fish;

const DEFAULT_FISH_BODIES: &str = include_str!("../../../data/fish_bodies.toml");

#[derive(Clone, Debug, PartialEq)]
pub struct FishBody {
    pub species: String,
    /// Box width (x and z) and height (y), in blocks.
    pub box_size: [f64; 2],
    /// Blocks per second.
    pub swim_speed: f64,
    /// Degrees per second.
    pub turn_rate: f64,
    /// Inclusive range of fish in one school.
    pub school: [u32; 2],
    pub raw_item: String,
}

/// The bodies, in file order.
#[derive(Clone, Debug, PartialEq)]
pub struct FishBodies {
    pub bodies: Vec<FishBody>,
}

fn err(kind: DataErrorKind) -> DataError {
    DataError::new(DataFile::FishBodies, kind)
}

fn bad_entry(entry: &str, problem: &str) -> DataError {
    let (entry, problem) = (entry.to_string(), problem.to_string());
    err(DataErrorKind::BadEntry { entry, problem })
}

fn positive(v: &toml::Value) -> Option<f64> {
    v.as_float()
        .or_else(|| v.as_integer().map(|i| i as f64))
        .filter(|n| n.is_finite() && *n > 0.0)
}

fn parse_body(n: usize, value: &toml::Value) -> Result<FishBody, DataError> {
    let table = value
        .as_table()
        .ok_or_else(|| err(DataErrorKind::Invalid(format!("body {n} is not a table"))))?;
    let species = table
        .get("species")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad_entry(&format!("body {n}"), "`species` must be a non-empty string"))?
        .to_string();
    let bad = |problem: &str| bad_entry(&species, problem);
    let fields = [
        "species",
        "box",
        "swim_speed",
        "turn_rate",
        "school",
        "raw_item",
    ];
    if let Some(field) = table.keys().find(|k| !fields.contains(&k.as_str())) {
        return Err(bad(&format!("unknown field `{field}`")));
    }
    let pair = |field: &str| {
        table
            .get(field)
            .and_then(|v| v.as_array())
            .filter(|a| a.len() == 2)
    };
    let box_size = pair("box")
        .and_then(|a| Some([positive(&a[0])?, positive(&a[1])?]))
        .ok_or_else(|| bad("`box` must be [width, height], both above zero"))?;
    let speed = |field: &str| {
        table
            .get(field)
            .and_then(positive)
            .ok_or_else(|| bad(&format!("`{field}` must be above zero")))
    };
    let (swim_speed, turn_rate) = (speed("swim_speed")?, speed("turn_rate")?);
    let school = pair("school")
        .and_then(|a| {
            let n = |v: &toml::Value| {
                v.as_integer()
                    .filter(|c| (1..=i64::from(u32::MAX)).contains(c))
            };
            let (lo, hi) = (n(&a[0])?, n(&a[1])?);
            (lo <= hi).then_some([lo as u32, hi as u32])
        })
        .ok_or_else(|| bad("`school` must be [min, max] fish, 1 <= min <= max"))?;
    let raw_item = table
        .get("raw_item")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("`raw_item` must be a non-empty string"))?
        .to_string();
    Ok(FishBody {
        species,
        box_size,
        swim_speed,
        turn_rate,
        school,
        raw_item,
    })
}

impl FishBodies {
    /// The shipped `data/fish_bodies.toml`, checked against the shipped
    /// species and items.
    pub fn default_from_data() -> Result<FishBodies, DataError> {
        let fish = Fish::shipped();
        let species: Vec<&str> = fish.species.iter().map(|s| s.id.as_str()).collect();
        FishBodies::parse(DEFAULT_FISH_BODIES, &species, &Items::default_items())
    }

    /// Parse bodies text; `species` is every fish id, all of which need a body.
    pub fn parse(text: &str, species: &[&str], items: &Items) -> Result<FishBodies, DataError> {
        let root: toml::value::Table = toml::from_str(text)
            .map_err(|e: toml::de::Error| err(DataErrorKind::Parse(e.to_string())))?;
        if let Some(key) = root.keys().find(|k| *k != "body") {
            return Err(err(DataErrorKind::Invalid(format!("unknown key `{key}`"))));
        }
        let list = root
            .get("body")
            .ok_or_else(|| err(DataErrorKind::MissingKey("body".into())))?
            .as_array()
            .ok_or_else(|| err(DataErrorKind::Invalid("`body` must be an array".into())))?;
        let mut bodies: Vec<FishBody> = Vec::new();
        for (i, value) in list.iter().enumerate() {
            let b = parse_body(i + 1, value)?;
            if bodies.iter().any(|o| o.species == b.species) {
                return Err(err(DataErrorKind::Duplicate {
                    kind: "species".into(),
                    name: b.species,
                }));
            }
            if !species.contains(&b.species.as_str()) {
                return Err(bad_entry(&b.species, "not a species in fish.toml"));
            }
            if !items.has_item(&b.raw_item) {
                return Err(err(DataErrorKind::UnknownItem {
                    entry: b.species,
                    item: b.raw_item,
                }));
            }
            bodies.push(b);
        }
        let bodies = FishBodies { bodies };
        match species.iter().find(|s| bodies.get(s).is_none()) {
            Some(s) => Err(bad_entry(s, "has no body")),
            None => Ok(bodies),
        }
    }

    pub fn get(&self, species: &str) -> Option<&FishBody> {
        self.bodies.iter().find(|b| b.species == species)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_error::test_support::*;

    const SPECIES: [&str; 2] = ["mackerel", "tuna"];

    fn body(species: &str, extra: &str) -> String {
        format!(
            "[[body]]\nspecies = \"{species}\"\nbox = [0.4, 0.3]\nswim_speed = 1.5\n\
             turn_rate = 180\nschool = [3, 6]\nraw_item = \"raw_{species}\"\n{extra}"
        )
    }

    fn parse(text: &str) -> Result<FishBodies, DataError> {
        FishBodies::parse(text, &SPECIES, &Items::default_items())
    }

    fn both() -> String {
        body("mackerel", "") + &body("tuna", "")
    }

    #[test]
    fn shipped_file_covers_every_species() {
        let b = FishBodies::default_from_data().unwrap();
        let fish = Fish::shipped();
        assert_eq!(b.bodies.len(), fish.species.len());
        for s in &fish.species {
            let body = b.get(&s.id).unwrap();
            assert_eq!(body.raw_item, format!("raw_{}", s.id));
        }
        assert!(b.get("squid").is_none());
    }

    #[test]
    fn small_file_loads() {
        let b = parse(&both()).unwrap();
        let m = b.get("mackerel").unwrap();
        assert_eq!(m.box_size, [0.4, 0.3]);
        assert_eq!((m.swim_speed, m.turn_rate), (1.5, 180.0));
        assert_eq!(m.school, [3, 6]);
    }

    #[test]
    fn rejects_unknown_species() {
        let text = both() + &body("salmon", "");
        assert_bad_entry(
            parse(&text).unwrap_err(),
            DataFile::FishBodies,
            "salmon",
            "not a species",
        );
    }

    #[test]
    fn rejects_unknown_raw_item() {
        let text = body("mackerel", "") + &body("tuna", "").replace("raw_tuna", "raw_whale");
        assert_unknown_item(
            parse(&text).unwrap_err(),
            DataFile::FishBodies,
            "tuna",
            "raw_whale",
        );
    }

    #[test]
    fn rejects_missing_species() {
        let e = parse(&body("mackerel", "")).unwrap_err();
        assert_bad_entry(e, DataFile::FishBodies, "tuna", "has no body");
        assert_missing_key(parse("").unwrap_err(), DataFile::FishBodies, "body");
    }

    #[test]
    fn rejects_duplicates() {
        let text = both() + &body("tuna", "");
        assert_err_contains(parse(&text), "duplicate species `tuna`");
    }

    #[test]
    fn rejects_non_positive_numbers() {
        let cases = [
            ("box = [0.4, 0.3]", "box = [0.0, 0.3]", "`box`"),
            ("box = [0.4, 0.3]", "box = [0.4, -1]", "`box`"),
            ("swim_speed = 1.5", "swim_speed = 0", "`swim_speed`"),
            ("turn_rate = 180", "turn_rate = -5", "`turn_rate`"),
            ("school = [3, 6]", "school = [0, 6]", "`school`"),
            ("school = [3, 6]", "school = [6, 3]", "`school`"),
        ];
        for (from, to, field) in cases {
            let text = body("mackerel", "").replace(from, to) + &body("tuna", "");
            assert_bad_entry(
                parse(&text).unwrap_err(),
                DataFile::FishBodies,
                "mackerel",
                field,
            );
        }
    }

    #[test]
    fn rejects_bad_shape() {
        assert_parse_error(parse("[[body]").unwrap_err(), DataFile::FishBodies);
        let e = parse(&body("mackerel", "colour = 3")).unwrap_err();
        assert_bad_entry(
            e,
            DataFile::FishBodies,
            "mackerel",
            "unknown field `colour`",
        );
        assert_err_contains(
            parse(&format!("speed = 1\n{}", both())),
            "unknown key `speed`",
        );
    }
}
