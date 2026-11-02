//! Tool and item definitions loaded from `data/items.toml`, and the player's
//! belt and hands. See docs/design/basic-gameplay.md section 6.
//!
//! Names must match the `held` strings in `data/interactions.toml`;
//! `Items::check_interactions` verifies that.

use crate::data_error::{DataError, DataErrorKind, DataFile};
use crate::interactions::{Held, HeldThing, Interactions};
use std::fmt;
use terrain::Block;

const DEFAULT_ITEMS: &str = include_str!("../../../data/items.toml");

/// Number of tool belt slots.
pub const BELT_SLOTS: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Def {
    pub name: String,
    pub label: String,
    /// Attack damage in half-hearts; only tools may set it.
    pub damage: Option<u32>,
}

/// The tool and item tables, in file order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Items {
    pub tools: Vec<Def>,
    pub items: Vec<Def>,
    /// Attack damage with the hand, and with any tool that sets none.
    pub hand_damage: u32,
    /// Pickable block name -> item name, in file order.
    pub pickable: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemsError {
    Parse(String),
    /// `kind` is `tool` or `item`; `index` is the 1-based position.
    BadEntry {
        kind: &'static str,
        index: usize,
        problem: String,
    },
    Duplicate {
        kind: &'static str,
        name: String,
    },
    /// A `held` string in the interaction table names something not defined.
    UnknownHeld {
        held: String,
    },
    /// A belt entry names something not defined.
    UnknownName {
        kind: &'static str,
        name: String,
    },
}

impl From<ItemsError> for DataError {
    fn from(e: ItemsError) -> DataError {
        match e {
            ItemsError::Parse(msg) => DataError::new(DataFile::Items, DataErrorKind::Parse(msg)),
            ItemsError::BadEntry {
                kind,
                index,
                problem,
            } => DataError::new(
                DataFile::Items,
                DataErrorKind::BadEntry {
                    entry: format!("{kind} {index}"),
                    problem,
                },
            ),
            ItemsError::Duplicate { kind, name } => DataError::new(
                DataFile::Items,
                DataErrorKind::Duplicate {
                    kind: kind.into(),
                    name,
                },
            ),
            ItemsError::UnknownHeld { held } => DataError::new(
                DataFile::Interactions,
                DataErrorKind::Invalid(format!("`{held}` is not in {}", DataFile::Items.name())),
            ),
            ItemsError::UnknownName { kind, name } => DataError::new(
                DataFile::Items,
                DataErrorKind::Invalid(format!("unknown {kind} `{name}`")),
            ),
        }
    }
}

impl fmt::Display for ItemsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // A belt name is not a file problem, so it keeps no file prefix.
            ItemsError::UnknownName { kind, name } => write!(f, "unknown {kind} `{name}`"),
            other => DataError::from(other.clone()).fmt(f),
        }
    }
}

impl std::error::Error for ItemsError {}

fn parse_defs(table: &toml::value::Table, kind: &'static str) -> Result<Vec<Def>, ItemsError> {
    let bad = |index: usize, problem: &str| ItemsError::BadEntry {
        kind,
        index,
        problem: problem.to_string(),
    };
    let Some(list) = table.get(kind) else {
        return Ok(Vec::new());
    };
    let list = list
        .as_array()
        .ok_or_else(|| bad(0, "not an array of tables"))?;
    let mut out: Vec<Def> = Vec::new();
    for (i, entry) in list.iter().enumerate() {
        let index = i + 1;
        let entry = entry.as_table().ok_or_else(|| bad(index, "not a table"))?;
        for field in entry.keys() {
            if field != "name" && field != "label" && !(kind == "tool" && field == "damage") {
                return Err(bad(index, &format!("unknown field `{field}`")));
            }
        }
        let text = |field: &str| {
            entry
                .get(field)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .ok_or_else(|| bad(index, &format!("`{field}` must be a non-empty string")))
        };
        let name = text("name")?;
        if name.contains(':') {
            return Err(bad(index, "`name` must not contain `:`"));
        }
        if out.iter().any(|d| d.name == name) {
            return Err(ItemsError::Duplicate { kind, name });
        }
        out.push(Def {
            name,
            label: text("label")?,
            damage: match entry.get("damage") {
                None => None,
                Some(v) => Some(
                    v.as_integer()
                        .filter(|d| (1..=i64::from(u32::MAX)).contains(d))
                        .ok_or_else(|| bad(index, "`damage` must be a positive integer"))?
                        as u32,
                ),
            },
        });
    }
    Ok(out)
}

fn parse_pickable(
    table: &toml::value::Table,
    items: &[Def],
) -> Result<Vec<(String, String)>, ItemsError> {
    let bad = |index: usize, problem: &str| ItemsError::BadEntry {
        kind: "pickable",
        index,
        problem: problem.to_string(),
    };
    let Some(list) = table.get("pickable") else {
        return Ok(Vec::new());
    };
    let list = list
        .as_array()
        .ok_or_else(|| bad(0, "not an array of tables"))?;
    let mut out: Vec<(String, String)> = Vec::new();
    for (i, entry) in list.iter().enumerate() {
        let index = i + 1;
        let entry = entry.as_table().ok_or_else(|| bad(index, "not a table"))?;
        if let Some(field) = entry.keys().find(|f| *f != "block" && *f != "item") {
            return Err(bad(index, &format!("unknown field `{field}`")));
        }
        let text = |field: &str| {
            entry
                .get(field)
                .and_then(|v| v.as_str())
                .ok_or_else(|| bad(index, &format!("`{field}` must be a string")))
        };
        let (block, item) = (text("block")?, text("item")?);
        if !Block::ALL.iter().any(|b| b.info().name == block) {
            return Err(ItemsError::UnknownName {
                kind: "block",
                name: block.to_string(),
            });
        }
        if !items.iter().any(|d| d.name == item) {
            return Err(ItemsError::UnknownName {
                kind: "item",
                name: item.to_string(),
            });
        }
        if out.iter().any(|(b, _)| b == block) {
            return Err(ItemsError::Duplicate {
                kind: "pickable",
                name: block.to_string(),
            });
        }
        out.push((block.to_string(), item.to_string()));
    }
    Ok(out)
}

impl Items {
    /// The tables shipped in `data/items.toml`.
    pub fn default_items() -> Items {
        Items::from_toml(DEFAULT_ITEMS).expect("data/items.toml is valid")
    }

    pub fn from_toml(text: &str) -> Result<Items, ItemsError> {
        let table: toml::value::Table =
            toml::from_str(text).map_err(|e| ItemsError::Parse(e.to_string()))?;
        let hand_damage = table
            .get("hand_damage")
            .and_then(|v| v.as_integer())
            .filter(|d| (1..=i64::from(u32::MAX)).contains(d))
            .ok_or_else(|| ItemsError::Parse("`hand_damage` must be a positive integer".into()))?
            as u32;
        let items = parse_defs(&table, "item")?;
        let pickable = parse_pickable(&table, &items)?;
        Ok(Items {
            tools: parse_defs(&table, "tool")?,
            items,
            hand_damage,
            pickable,
        })
    }

    /// The item picking a `block` gives, or `None` if it can't be picked.
    pub fn pick_item(&self, block: &str) -> Option<&str> {
        self.pickable
            .iter()
            .find(|(b, _)| b == block)
            .map(|(_, i)| i.as_str())
    }

    /// Attack damage in half-hearts with `tool` in hand, or with the bare
    /// hand when `None` (section 8.1).
    pub fn damage(&self, tool: Option<&str>) -> u32 {
        tool.and_then(|t| self.tools.iter().find(|d| d.name == t))
            .and_then(|d| d.damage)
            .unwrap_or(self.hand_damage)
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|d| d.name == name)
    }

    pub fn has_item(&self, name: &str) -> bool {
        self.items.iter().any(|d| d.name == name)
    }

    /// Every `tool:` and `item:` named by a rule must be defined here.
    pub fn check_interactions(&self, rules: &Interactions) -> Result<(), ItemsError> {
        for rule in &rules.rules {
            let (known, held) = match &rule.held {
                Held::Tool(n) => (self.has_tool(n), format!("tool:{n}")),
                Held::Item(n) => (self.has_item(n), format!("item:{n}")),
                _ => continue,
            };
            if !known {
                return Err(ItemsError::UnknownHeld { held });
            }
        }
        Ok(())
    }
}

/// A tool on the belt. `tool` names an entry in the tool table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolInstance {
    pub tool: String,
}

/// A carried item. `item` names an entry in the item table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemInstance {
    pub item: String,
}

/// The tool belt and hands (design section 6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerState {
    pub tool_belt: [Option<ToolInstance>; BELT_SLOTS],
    pub selected: u8,
    pub hands: Option<ItemInstance>,
}

impl PlayerState {
    /// The v0 belt: empty hand, sword, hoe, fishing rod, and one reserved slot
    /// (the bucket). Fails if the tables lack the starting tools.
    pub fn new(items: &Items) -> Result<PlayerState, ItemsError> {
        let tool = |name: &str| {
            if items.has_tool(name) {
                Ok(Some(ToolInstance {
                    tool: name.to_string(),
                }))
            } else {
                Err(ItemsError::UnknownName {
                    kind: "tool",
                    name: name.to_string(),
                })
            }
        };
        Ok(PlayerState {
            tool_belt: [
                None,
                tool("sword")?,
                tool("hoe")?,
                tool("fishing_rod")?,
                None,
            ],
            selected: 0,
            hands: None,
        })
    }

    /// Select belt slot `slot` (0-based). Slots beyond the belt are ignored.
    pub fn select(&mut self, slot: u8) {
        if usize::from(slot) < BELT_SLOTS {
            self.selected = slot;
        }
    }

    /// The first empty belt slot after slot 0, which is the empty hand.
    pub fn first_empty_belt_slot(&self) -> Option<usize> {
        (1..BELT_SLOTS).find(|&i| self.tool_belt[i].is_none())
    }

    /// Put tool `name` in the first empty belt slot; false if the belt is full.
    pub fn stow_tool(&mut self, name: String) -> bool {
        match self.first_empty_belt_slot() {
            Some(i) => {
                self.tool_belt[i] = Some(ToolInstance { tool: name });
                true
            }
            None => false,
        }
    }

    /// What interaction rules see: the carried item if the hands are
    /// non-empty (tools are stowed), else the selected tool, else `Empty`.
    pub fn held_thing(&self) -> HeldThing {
        if let Some(item) = &self.hands {
            return HeldThing::Item(item.item.clone());
        }
        match self.tool_belt.get(usize::from(self.selected)) {
            Some(Some(t)) => HeldThing::Tool(t.tool.clone()),
            _ => HeldThing::Empty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_error::test_support::assert_err_contains;

    #[test]
    fn errors_convert_to_data_error_with_same_text() {
        let cases = [
            (ItemsError::Parse("bad".into()), "items.toml: bad"),
            (
                ItemsError::BadEntry {
                    kind: "tool",
                    index: 2,
                    problem: "no name".into(),
                },
                "items.toml: tool 2: no name",
            ),
            (
                ItemsError::Duplicate {
                    kind: "item",
                    name: "stick".into(),
                },
                "items.toml: duplicate item `stick`",
            ),
            (
                ItemsError::UnknownHeld { held: "x".into() },
                "interactions.toml: `x` is not in items.toml",
            ),
        ];
        for (e, text) in cases {
            assert_eq!(e.to_string(), text);
            assert_eq!(DataError::from(e).to_string(), text);
        }
        let e = ItemsError::UnknownName {
            kind: "tool",
            name: "x".into(),
        };
        assert_eq!(e.to_string(), "unknown tool `x`");
        assert_eq!(
            DataError::from(e).to_string(),
            "items.toml: unknown tool `x`"
        );
    }

    #[test]
    fn default_file_loads() {
        let items = Items::default_items();
        assert!(items.has_tool("sword") && items.has_tool("hoe"));
        assert!(items.has_item("seeds"));
    }

    #[test]
    fn fish_catches_load_and_can_be_carried() {
        use crate::commands::{give_command, Registry};
        use crate::input::TickInput;
        let items = Items::default_items();
        for name in ["raw_salmon", "raw_tuna", "raw_mackerel", "raw_squid"] {
            assert!(items.has_item(name), "{name}");
            let mut w = crate::walk::Walker::new([0.5, 5.0, 0.5]).unwrap();
            w.set_commands(Registry::new(vec![give_command()]).unwrap());
            w.submit_command(&format!("give item {name}"));
            w.tick(&TickInput::default());
            assert_eq!(w.drain_command_output(), vec![format!("gave item {name}")]);
            assert_eq!(w.hands().unwrap().item, name);
        }
    }

    #[test]
    fn flowers_are_pickable_and_nothing_else_is() {
        let items = Items::default_items();
        assert_eq!(items.pick_item("dandelion"), Some("dandelion_flower"));
        assert_eq!(items.pick_item("rose"), Some("rose_flower"));
        for block in ["tall_grass", "crop", "grass", "stone", "air", "nope"] {
            assert_eq!(items.pick_item(block), None, "{block}");
        }
    }

    #[test]
    fn rejects_bad_pickable() {
        let head = "hand_damage=1\n[[item]]\nname='f'\nlabel='F'\n";
        let ok = format!("{head}[[pickable]]\nblock='rose'\nitem='f'\n");
        assert_eq!(Items::from_toml(&ok).unwrap().pick_item("rose"), Some("f"));
        let unknown_block = format!("{head}[[pickable]]\nblock='nope'\nitem='f'\n");
        assert!(matches!(
            Items::from_toml(&unknown_block),
            Err(ItemsError::UnknownName { kind: "block", .. })
        ));
        let unknown_item = format!("{head}[[pickable]]\nblock='rose'\nitem='g'\n");
        assert!(matches!(
            Items::from_toml(&unknown_item),
            Err(ItemsError::UnknownName { kind: "item", .. })
        ));
        let dup = format!("{ok}[[pickable]]\nblock='rose'\nitem='f'\n");
        assert!(matches!(
            Items::from_toml(&dup),
            Err(ItemsError::Duplicate {
                kind: "pickable",
                ..
            })
        ));
        let extra = format!("{head}[[pickable]]\nblock='rose'\nitem='f'\nx=1\n");
        assert_err_contains(Items::from_toml(&extra), "items.toml");
    }

    #[test]
    fn default_names_match_interactions() {
        Items::default_items()
            .check_interactions(&Interactions::default_rules())
            .unwrap();
    }

    #[test]
    fn starting_belt() {
        let p = PlayerState::new(&Items::default_items()).unwrap();
        let name = |i: usize| p.tool_belt[i].as_ref().map(|t| t.tool.as_str());
        assert_eq!(name(0), None);
        assert_eq!(name(1), Some("sword"));
        assert_eq!(name(2), Some("hoe"));
        assert_eq!(name(3), Some("fishing_rod"));
        assert_eq!(name(4), None);
        assert_eq!(p.selected, 0);
        assert_eq!(p.hands, None);
    }

    #[test]
    fn rejects_duplicates() {
        let t = "hand_damage=1\n[[tool]]\nname='a'\nlabel='A'\n[[tool]]\nname='a'\nlabel='B'\n";
        assert!(matches!(
            Items::from_toml(t),
            Err(ItemsError::Duplicate { kind: "tool", .. })
        ));
        let i = "hand_damage=1\n[[item]]\nname='a'\nlabel='A'\n[[item]]\nname='a'\nlabel='B'\n";
        assert!(matches!(
            Items::from_toml(i),
            Err(ItemsError::Duplicate { kind: "item", .. })
        ));
    }

    #[test]
    fn rejects_bad_entries() {
        for bad in [
            "[[tool]]\nlabel='A'\n",
            "[[tool]]\nname=''\nlabel='A'\n",
            "[[tool]]\nname='a:b'\nlabel='A'\n",
            "[[tool]]\nname='a'\nlabel='A'\nextra=1\n",
            "[[item]]\nname=3\nlabel='A'\n",
            "tool = 3\n",
            "hand_damage=1\n[[tool]]\nname='a'\nlabel='A'\ndamage=0\n",
            "hand_damage=1\n[[item]]\nname='a'\nlabel='A'\ndamage=2\n",
            "hand_damage=0\n",
            "[[tool]]\nname='a'\nlabel='A'\n",
            "not toml [",
        ] {
            assert_err_contains(Items::from_toml(bad), "items.toml");
        }
    }

    #[test]
    fn unknown_held_names_rejected() {
        let rules = Interactions::from_toml(
            "[[rule]]\nheld='tool:axe'\nbutton='primary'\ntarget='none'\naction='attack'\n",
        )
        .unwrap();
        assert!(matches!(
            Items::default_items().check_interactions(&rules),
            Err(ItemsError::UnknownHeld { .. })
        ));
    }

    #[test]
    fn belt_needs_starting_tools() {
        let items = Items::from_toml("hand_damage=1\n[[tool]]\nname='sword'\nlabel='S'\n").unwrap();
        assert!(matches!(
            PlayerState::new(&items),
            Err(ItemsError::UnknownName { .. })
        ));
    }

    fn state() -> PlayerState {
        PlayerState::new(&Items::default_items()).unwrap()
    }

    #[test]
    fn select_changes_slot_and_ignores_out_of_range() {
        let mut p = state();
        p.select(2);
        assert_eq!(p.selected, 2);
        p.select(BELT_SLOTS as u8);
        p.select(200);
        assert_eq!(p.selected, 2);
    }

    #[test]
    fn held_thing_follows_selection() {
        let mut p = state();
        assert_eq!(p.held_thing(), HeldThing::Empty);
        p.select(1);
        assert_eq!(p.held_thing(), HeldThing::Tool("sword".into()));
        p.select(2);
        assert_eq!(p.held_thing(), HeldThing::Tool("hoe".into()));
        p.select(3);
        assert_eq!(p.held_thing(), HeldThing::Tool("fishing_rod".into()));
        p.select(4);
        assert_eq!(p.held_thing(), HeldThing::Empty);
    }

    #[test]
    fn carried_item_overrides_selection() {
        let mut p = state();
        p.hands = Some(ItemInstance {
            item: "seeds".into(),
        });
        for slot in 0..BELT_SLOTS as u8 {
            p.select(slot);
            assert_eq!(p.held_thing(), HeldThing::Item("seeds".into()));
        }
    }
}
