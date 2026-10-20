//! Console aliases loaded from `data/commands.toml`: a name that expands to
//! one or more command lines. See docs/design/debug-console.md section 6.
//!
//! Running an alias is not done here; this module only loads and validates.

use crate::commands::Registry;
use crate::data_error::{DataError, DataErrorKind, DataFile};

const DEFAULT_COMMANDS: &str = include_str!("../../../data/commands.toml");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alias {
    pub name: String,
    /// Command lines, run in order.
    pub expands: Vec<String>,
}

/// The alias table, in file order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aliases {
    pub aliases: Vec<Alias>,
}

fn err(kind: DataErrorKind) -> DataError {
    DataError::new(DataFile::Commands, kind)
}

fn bad(entry: &str, problem: &str) -> DataError {
    err(DataErrorKind::BadEntry {
        entry: entry.into(),
        problem: problem.into(),
    })
}

fn is_snake_case(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase())
        && !name.ends_with('_')
        && !name.contains("__")
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl Aliases {
    /// The table shipped in `data/commands.toml`, checked against `registry`.
    pub fn default_aliases<W>(registry: &Registry<W>) -> Result<Aliases, DataError> {
        Aliases::from_toml(DEFAULT_COMMANDS, registry)
    }

    /// Parses and validates `text`. No alias may equal a command name or
    /// command alias in `registry` (which includes `help`).
    pub fn from_toml<W>(text: &str, registry: &Registry<W>) -> Result<Aliases, DataError> {
        let table: toml::value::Table =
            toml::from_str(text).map_err(|e| err(DataErrorKind::Parse(e.to_string())))?;
        for key in table.keys() {
            if key != "alias" {
                return Err(err(DataErrorKind::Invalid(format!("unknown key `{key}`"))));
            }
        }
        let Some(list) = table.get("alias") else {
            return Ok(Aliases {
                aliases: Vec::new(),
            });
        };
        let list = list
            .as_array()
            .ok_or_else(|| bad("alias", "not an array of tables"))?;
        let mut out: Vec<Alias> = Vec::new();
        for (i, entry) in list.iter().enumerate() {
            let at = format!("alias {}", i + 1);
            let entry = entry.as_table().ok_or_else(|| bad(&at, "not a table"))?;
            for field in entry.keys() {
                if field != "name" && field != "expands" {
                    return Err(bad(&at, &format!("unknown field `{field}`")));
                }
            }
            let name = entry
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| bad(&at, "`name` must be a non-empty string"))?;
            if !is_snake_case(name) {
                return Err(bad(name, "name must be snake_case"));
            }
            if registry.get(name).is_some() {
                return Err(bad(name, "shadows a built-in command name or alias"));
            }
            if out.iter().any(|a| a.name == name) {
                return Err(err(DataErrorKind::Duplicate {
                    kind: "alias".into(),
                    name: name.into(),
                }));
            }
            let lines = entry
                .get("expands")
                .and_then(|v| v.as_array())
                .filter(|a| !a.is_empty())
                .ok_or_else(|| bad(name, "`expands` must be a non-empty list of command lines"))?;
            let mut expands = Vec::new();
            for line in lines {
                let line = line
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| bad(name, "`expands` lines must be non-empty strings"))?;
                expands.push(line.to_string());
            }
            out.push(Alias {
                name: name.into(),
                expands,
            });
        }
        Ok(Aliases { aliases: out })
    }

    pub fn get(&self, name: &str) -> Option<&Alias> {
        self.aliases.iter().find(|a| a.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{default_registry, parse};
    use crate::data_error::test_support::{
        assert_bad_entry, assert_err_contains, assert_parse_error,
    };

    fn load(text: &str) -> Result<Aliases, DataError> {
        Aliases::from_toml(text, &default_registry().unwrap())
    }

    fn one(name: &str, expands: &str) -> String {
        format!("[[alias]]\nname = \"{name}\"\nexpands = {expands}\n")
    }

    #[test]
    fn default_file_loads_and_lines_parse() {
        let registry = default_registry().unwrap();
        let aliases = Aliases::default_aliases(&registry).unwrap();
        let kitchen = aliases.get("kitchen").expect("kitchen alias");
        assert_eq!(kitchen.expands, ["give tool hoe", "spawn seeds 8"]);
        for a in &aliases.aliases {
            for line in &a.expands {
                let parsed = parse(&registry, line);
                assert!(
                    matches!(parsed, Ok(Some(_))),
                    "{}: {line}: parse failed",
                    a.name
                );
            }
        }
    }

    #[test]
    fn valid_load_keeps_order() {
        let text = format!(
            "{}{}",
            one("b_two", r#"["help"]"#),
            one("a1", r#"["help", "help give"]"#)
        );
        let a = load(&text).unwrap();
        let names: Vec<&str> = a.aliases.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["b_two", "a1"]);
        assert_eq!(a.get("a1").unwrap().expands.len(), 2);
    }

    #[test]
    fn shadowing_a_command_is_rejected() {
        for name in ["give", "help", "spawn"] {
            let e = load(&one(name, r#"["help"]"#));
            assert_bad_entry(e.clone().unwrap_err(), DataFile::Commands, name, "shadows");
            assert_err_contains(e, name);
        }
    }

    #[test]
    fn shadowing_a_command_alias_is_rejected() {
        let mut cmd = crate::commands::clear_command();
        cmd.aliases = &["wipe"];
        let registry = Registry::new(vec![cmd]).unwrap();
        let e = Aliases::from_toml(&one("wipe", r#"["clear"]"#), &registry);
        assert_err_contains(e, "wipe");
    }

    #[test]
    fn duplicate_names_are_rejected() {
        let text = format!("{}{}", one("dup", r#"["help"]"#), one("dup", r#"["help"]"#));
        assert_err_contains(load(&text), "duplicate alias `dup`");
    }

    #[test]
    fn empty_expands_is_rejected() {
        assert_bad_entry(
            load(&one("empty", "[]")).unwrap_err(),
            DataFile::Commands,
            "empty",
            "expands",
        );
        assert_bad_entry(
            load(&one("blank", r#"[" "]"#)).unwrap_err(),
            DataFile::Commands,
            "blank",
            "expands",
        );
    }

    #[test]
    fn bad_names_are_rejected() {
        for name in [
            "Kitchen",
            "has-dash",
            "_x",
            "x_",
            "a__b",
            "1up",
            "has space",
        ] {
            assert_bad_entry(
                load(&one(name, r#"["help"]"#)).unwrap_err(),
                DataFile::Commands,
                name,
                "snake_case",
            );
        }
        assert_bad_entry(
            load(&one("", r#"["help"]"#)).unwrap_err(),
            DataFile::Commands,
            "alias 1",
            "name",
        );
    }

    #[test]
    fn malformed_file_is_rejected() {
        assert_parse_error(load("[[alias").unwrap_err(), DataFile::Commands);
        assert_err_contains(load("[[alias]]\nname = 3\n"), "commands.toml");
        assert_err_contains(load("extra = 1\n"), "unknown key `extra`");
        assert_err_contains(
            load("[[alias]]\nname = \"x\"\nexpands = [\"help\"]\nfoo = 1\n"),
            "unknown field `foo`",
        );
    }
}
