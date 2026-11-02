//! Station state (docs/design/cooking-stations.md Phase B, section 3).
//!
//! At most one item sits on a station cell. Cells are plain `[i32; 3]` keys:
//! a raft-local mask cell now, a world position later.

use std::collections::BTreeMap;

use crate::data_error::{DataError, DataErrorKind, DataFile};
use crate::items::ItemInstance;

const DEFAULT_STATIONS: &str = include_str!("../../../data/stations.toml");

/// One station block from `data/stations.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StationKind {
    pub name: String,
    /// Holds one item on top.
    pub surface: bool,
}

/// The station block table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StationKinds {
    kinds: Vec<StationKind>,
}

impl StationKinds {
    /// The shipped table from `data/stations.toml`.
    pub fn default_from_data() -> Result<StationKinds, DataError> {
        StationKinds::parse(DEFAULT_STATIONS)
    }

    /// Parse station text: `[[station]]` entries with `name` and `surface`.
    pub fn parse(text: &str) -> Result<StationKinds, DataError> {
        let err = |kind| DataError::new(DataFile::Stations, kind);
        let bad = |entry: &str, problem: &str| {
            err(DataErrorKind::BadEntry {
                entry: entry.into(),
                problem: problem.into(),
            })
        };
        let root: toml::Value = text
            .parse()
            .map_err(|e: toml::de::Error| err(DataErrorKind::Parse(e.to_string())))?;
        let tables = root
            .get("station")
            .and_then(|v| v.as_array())
            .ok_or_else(|| err(DataErrorKind::MissingKey("station".into())))?;
        let mut kinds: Vec<StationKind> = Vec::new();
        for (i, t) in tables.iter().enumerate() {
            let t = t
                .as_table()
                .ok_or_else(|| bad(&format!("station {i}"), "must be a table"))?;
            let name = match t.get("name") {
                Some(v) => v
                    .as_str()
                    .ok_or_else(|| bad(&format!("station {i}"), "`name` must be a string"))?,
                None => return Err(bad(&format!("station {i}"), "missing `name`")),
            };
            if let Some(key) = t.keys().find(|k| *k != "name" && *k != "surface") {
                return Err(bad(name, &format!("unknown field `{key}`")));
            }
            let surface = match t.get("surface") {
                Some(v) => v
                    .as_bool()
                    .ok_or_else(|| bad(name, "`surface` must be true or false"))?,
                None => return Err(bad(name, "missing `surface`")),
            };
            if kinds.iter().any(|k| k.name == name) {
                return Err(err(DataErrorKind::Duplicate {
                    kind: "station".into(),
                    name: name.into(),
                }));
            }
            kinds.push(StationKind {
                name: name.into(),
                surface,
            });
        }
        Ok(StationKinds { kinds })
    }

    /// Whether `name` is a station block.
    pub fn is_station(&self, name: &str) -> bool {
        self.kinds.iter().any(|k| k.name == name)
    }

    /// Whether `name` is a station that holds one item on top.
    pub fn holds_on_top(&self, name: &str) -> bool {
        self.kinds.iter().any(|k| k.name == name && k.surface)
    }
}

/// The items sitting on station cells, one per cell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stations {
    items: BTreeMap<[i32; 3], ItemInstance>,
}

impl Stations {
    pub fn new() -> Stations {
        Stations::default()
    }

    /// Put `item` on `cell`. If the cell is occupied, nothing changes and the
    /// item is handed back.
    pub fn put_down(&mut self, cell: [i32; 3], item: ItemInstance) -> Result<(), ItemInstance> {
        if self.items.contains_key(&cell) {
            return Err(item);
        }
        self.items.insert(cell, item);
        Ok(())
    }

    /// Remove and return the item on `cell`.
    pub fn take(&mut self, cell: [i32; 3]) -> Option<ItemInstance> {
        self.items.remove(&cell)
    }

    /// The item on `cell`, if any.
    pub fn get(&self, cell: [i32; 3]) -> Option<&ItemInstance> {
        self.items.get(&cell)
    }

    /// Occupied cells and their items, sorted by cell.
    pub fn iter(&self) -> impl Iterator<Item = ([i32; 3], &ItemInstance)> {
        self.items.iter().map(|(c, i)| (*c, i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_error::test_support::*;

    fn item(name: &str) -> ItemInstance {
        ItemInstance { item: name.into() }
    }

    #[test]
    fn put_down_refuses_second_item() {
        let mut s = Stations::new();
        assert_eq!(s.put_down([1, 2, 3], item("fish")), Ok(()));
        assert_eq!(s.put_down([1, 2, 3], item("stick")), Err(item("stick")));
        assert_eq!(s.get([1, 2, 3]), Some(&item("fish")));
    }

    #[test]
    fn take_empties_the_cell() {
        let mut s = Stations::new();
        assert_eq!(s.take([0, 0, 0]), None);
        s.put_down([0, 0, 0], item("fish")).unwrap();
        assert_eq!(s.take([0, 0, 0]), Some(item("fish")));
        assert_eq!(s.get([0, 0, 0]), None);
        assert_eq!(s.take([0, 0, 0]), None);
        assert_eq!(s.put_down([0, 0, 0], item("stick")), Ok(()));
    }

    #[test]
    fn iteration_ignores_insertion_order() {
        let cells = [[2, 0, 0], [-1, 5, 0], [2, -3, 1], [0, 0, 0]];
        let mut a = Stations::new();
        let mut b = Stations::new();
        for c in cells {
            a.put_down(c, item("fish")).unwrap();
        }
        for c in cells.iter().rev() {
            b.put_down(*c, item("fish")).unwrap();
        }
        let order: Vec<_> = a.iter().map(|(c, _)| c).collect();
        assert_eq!(order, b.iter().map(|(c, _)| c).collect::<Vec<_>>());
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(order, sorted);
        assert_eq!(order.len(), 4);
    }

    #[test]
    fn shipped_table_loads() {
        let k = StationKinds::default_from_data().unwrap();
        for n in ["counter", "butcher_block", "griddle"] {
            assert!(k.is_station(n) && k.holds_on_top(n), "{n}");
        }
        for n in ["chest", "plate_rack"] {
            assert!(k.is_station(n) && !k.holds_on_top(n), "{n}");
        }
        assert!(!k.is_station("dirt") && !k.holds_on_top("dirt"));
    }

    #[test]
    fn bad_files_are_named() {
        let p = StationKinds::parse;
        assert_parse_error(p("station = [").unwrap_err(), DataFile::Stations);
        assert_missing_key(p("").unwrap_err(), DataFile::Stations, "station");
        assert_bad_entry(
            p("[[station]]\nname = \"a\"").unwrap_err(),
            DataFile::Stations,
            "a",
            "missing `surface`",
        );
        assert_bad_entry(
            p("[[station]]\nsurface = true").unwrap_err(),
            DataFile::Stations,
            "station 0",
            "missing `name`",
        );
        assert_bad_entry(
            p("[[station]]\nname = \"a\"\nsurface = true\nheat = 1").unwrap_err(),
            DataFile::Stations,
            "a",
            "unknown field `heat`",
        );
        let dup =
            "[[station]]\nname = \"a\"\nsurface = true\n[[station]]\nname = \"a\"\nsurface = false";
        assert_err_contains(p(dup), "duplicate station `a`");
    }
}
