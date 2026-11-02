//! Chest slot state (docs/design/cooking-stations.md Phase C, sections 7, 7.2).
//!
//! Each chest has a fixed number of slots, each holding one item with no
//! stacking. Cells are plain `[i32; 3]` keys, as in `stations.rs`.

use std::collections::BTreeMap;

use crate::data_error::{DataError, DataErrorKind, DataFile};
use crate::items::ItemInstance;

const DEFAULT_CHESTS: &str = include_str!("../../../data/chests.toml");

/// Slots per chest, from `data/chests.toml`.
pub fn slot_count_from_data() -> Result<usize, DataError> {
    slot_count(DEFAULT_CHESTS)
}

/// Parse chest text: a positive integer `slots`.
pub fn slot_count(text: &str) -> Result<usize, DataError> {
    let err = |kind| DataError::new(DataFile::Chests, kind);
    let root: toml::Value = text
        .parse()
        .map_err(|e: toml::de::Error| err(DataErrorKind::Parse(e.to_string())))?;
    let slots = root
        .get("slots")
        .ok_or_else(|| err(DataErrorKind::MissingKey("slots".into())))?
        .as_integer()
        .ok_or_else(|| err(DataErrorKind::Invalid("`slots` must be an integer".into())))?;
    if slots < 1 {
        return Err(err(DataErrorKind::Invalid(
            "`slots` must be at least 1".into(),
        )));
    }
    usize::try_from(slots).map_err(|_| err(DataErrorKind::Invalid("`slots` is too large".into())))
}

/// The contents of every chest, keyed by raft-local cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chests {
    slots_per_chest: usize,
    chests: BTreeMap<[i32; 3], Vec<Option<ItemInstance>>>,
}

impl Chests {
    /// No chest holds anything; each has `slots_per_chest` slots.
    pub fn new(slots_per_chest: usize) -> Chests {
        Chests {
            slots_per_chest,
            chests: BTreeMap::new(),
        }
    }

    /// Put `item` in the first free slot of the chest at `cell` and return the
    /// slot index. If the chest is full, nothing changes and the item is
    /// handed back.
    pub fn deposit(&mut self, cell: [i32; 3], item: ItemInstance) -> Result<usize, ItemInstance> {
        let n = self.slots_per_chest;
        let slots = self.chests.entry(cell).or_insert_with(|| vec![None; n]);
        match slots.iter().position(Option::is_none) {
            Some(i) => {
                slots[i] = Some(item);
                Ok(i)
            }
            None => Err(item),
        }
    }

    /// Empty one slot and return its item, if it had one.
    pub fn take_slot(&mut self, cell: [i32; 3], slot: usize) -> Option<ItemInstance> {
        self.chests.get_mut(&cell)?.get_mut(slot)?.take()
    }

    /// The slots of the chest at `cell`; empty if nothing was ever stored there.
    pub fn slots(&self, cell: [i32; 3]) -> &[Option<ItemInstance>] {
        self.chests.get(&cell).map_or(&[], |s| s.as_slice())
    }

    /// Chests that have been stored in, and their slots, sorted by cell.
    pub fn iter(&self) -> impl Iterator<Item = ([i32; 3], &[Option<ItemInstance>])> {
        self.chests.iter().map(|(c, s)| (*c, s.as_slice()))
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
    fn deposit_uses_first_free_slot_and_reuses_emptied() {
        let mut c = Chests::new(3);
        assert_eq!(c.deposit([0, 0, 0], item("a")), Ok(0));
        assert_eq!(c.deposit([0, 0, 0], item("b")), Ok(1));
        assert_eq!(c.deposit([0, 0, 0], item("c")), Ok(2));
        assert_eq!(c.take_slot([0, 0, 0], 1), Some(item("b")));
        assert_eq!(c.take_slot([0, 0, 0], 1), None);
        assert_eq!(c.deposit([0, 0, 0], item("d")), Ok(1));
        assert_eq!(c.slots([0, 0, 0])[1], Some(item("d")));
    }

    #[test]
    fn full_chest_hands_item_back_unchanged() {
        let mut c = Chests::new(1);
        c.deposit([1, 1, 1], item("a")).unwrap();
        let before = c.clone();
        assert_eq!(c.deposit([1, 1, 1], item("b")), Err(item("b")));
        assert_eq!(c, before);
        assert_eq!(c.deposit([2, 1, 1], item("b")), Ok(0));
    }

    #[test]
    fn reads_and_bad_takes() {
        let mut c = Chests::new(2);
        assert!(c.slots([5, 5, 5]).is_empty());
        assert_eq!(c.take_slot([5, 5, 5], 0), None);
        c.deposit([5, 5, 5], item("a")).unwrap();
        assert_eq!(c.slots([5, 5, 5]), &[Some(item("a")), None]);
        assert_eq!(c.take_slot([5, 5, 5], 9), None);
    }

    #[test]
    fn iteration_is_sorted_by_cell() {
        let mut c = Chests::new(2);
        for cell in [[2, 0, 0], [-1, 5, 0], [0, 0, 0]] {
            c.deposit(cell, item("a")).unwrap();
        }
        let cells: Vec<_> = c.iter().map(|(cell, _)| cell).collect();
        assert_eq!(cells, [[-1, 5, 0], [0, 0, 0], [2, 0, 0]]);
    }

    #[test]
    fn shipped_slot_count_loads() {
        assert_eq!(slot_count_from_data().unwrap(), 18);
    }

    #[test]
    fn bad_files_are_named() {
        assert_parse_error(slot_count("slots = ").unwrap_err(), DataFile::Chests);
        assert_missing_key(slot_count("").unwrap_err(), DataFile::Chests, "slots");
        for text in ["slots = 0", "slots = -2", "slots = \"x\""] {
            let e = slot_count(text).unwrap_err();
            assert_eq!(e.file, DataFile::Chests);
            assert!(e.to_string().starts_with("chests.toml: "), "{e}");
        }
    }
}
