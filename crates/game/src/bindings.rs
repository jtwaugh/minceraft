//! Input bindings loaded from `data/input.toml`.
//! See docs/design/basic-gameplay.md section 2.1.

use std::collections::HashMap;
use std::fmt;

use crate::data_error::{DataError, DataErrorKind, DataFile};

const DEFAULT_INPUT: &str = include_str!("../../../data/input.toml");

/// Every bindable action, in the order of the design table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    MoveForward,
    MoveLeft,
    MoveBack,
    MoveRight,
    Look,
    Jump,
    Sneak,
    Primary,
    Secondary,
    Interact,
    Drop,
    SelectTool,
    Inventory,
}

impl Action {
    pub const ALL: [Action; 13] = [
        Action::MoveForward,
        Action::MoveLeft,
        Action::MoveBack,
        Action::MoveRight,
        Action::Look,
        Action::Jump,
        Action::Sneak,
        Action::Primary,
        Action::Secondary,
        Action::Interact,
        Action::Drop,
        Action::SelectTool,
        Action::Inventory,
    ];

    /// The name used in `data/input.toml`.
    pub fn name(self) -> &'static str {
        match self {
            Action::MoveForward => "move_forward",
            Action::MoveLeft => "move_left",
            Action::MoveBack => "move_back",
            Action::MoveRight => "move_right",
            Action::Look => "look",
            Action::Jump => "jump",
            Action::Sneak => "sneak",
            Action::Primary => "primary",
            Action::Secondary => "secondary",
            Action::Interact => "interact",
            Action::Drop => "drop",
            Action::SelectTool => "select_tool",
            Action::Inventory => "inventory",
        }
    }

    fn from_name(name: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.name() == name)
    }
}

/// How an action's input is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// True while the key is down.
    Hold,
    /// Latched once per press.
    Edge,
    /// Analogue, sampled every frame.
    Continuous,
}

/// The keys and behaviour of one action.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub keys: Vec<String>,
    pub mode: Mode,
    /// Keep firing while held (secondary use).
    pub hold_repeat: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bindings {
    /// Radians per pixel of mouse motion (plain linear scale).
    pub sensitivity: f32,
    pub fov_degrees: f32,
    /// Double-tap window for sprinting, in sim ticks.
    pub double_tap_ticks: u32,
    actions: HashMap<Action, Binding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingsError {
    Parse(String),
    UnknownAction(String),
    MissingAction(&'static str),
    DuplicateKey {
        key: String,
        first: &'static str,
        second: &'static str,
    },
    Invalid(String),
}

impl From<BindingsError> for DataError {
    fn from(e: BindingsError) -> DataError {
        let kind = match e {
            BindingsError::Parse(e) => DataErrorKind::Parse(e),
            BindingsError::UnknownAction(a) => {
                DataErrorKind::Invalid(format!("unknown action `{a}`"))
            }
            BindingsError::MissingAction(a) => {
                DataErrorKind::Invalid(format!("missing action `{a}`"))
            }
            BindingsError::DuplicateKey { key, first, second } if first == second => {
                DataErrorKind::Invalid(format!("key `{key}` listed twice for action `{first}`"))
            }
            BindingsError::DuplicateKey { key, first, second } => DataErrorKind::Invalid(format!(
                "key `{key}` bound to both `{first}` and `{second}`"
            )),
            BindingsError::Invalid(e) => DataErrorKind::Invalid(e),
        };
        DataError::new(DataFile::Input, kind)
    }
}

impl fmt::Display for BindingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        DataError::from(self.clone()).fmt(f)
    }
}

impl std::error::Error for BindingsError {}

impl Bindings {
    /// The shipped defaults from `data/input.toml`.
    pub fn default_from_data() -> Result<Bindings, BindingsError> {
        Bindings::parse(DEFAULT_INPUT)
    }

    /// Parse bindings text; rejects unknown actions, missing actions and
    /// keys that are bound more than once.
    pub fn parse(text: &str) -> Result<Bindings, BindingsError> {
        let root: toml::Value = text
            .parse()
            .map_err(|e: toml::de::Error| BindingsError::Parse(e.to_string()))?;
        let look = root
            .get("look")
            .and_then(|v| v.as_table())
            .ok_or_else(|| BindingsError::Invalid("missing [look] table".into()))?;
        let sensitivity = number(look, "sensitivity")?;
        let fov_degrees = number(look, "fov_degrees")?;
        if sensitivity <= 0.0 {
            return Err(BindingsError::Invalid(
                "sensitivity must be positive".into(),
            ));
        }
        if !(1.0..180.0).contains(&fov_degrees) {
            return Err(BindingsError::Invalid(
                "fov_degrees must be in 1..180".into(),
            ));
        }
        let sprint = root
            .get("sprint")
            .and_then(|v| v.as_table())
            .ok_or_else(|| BindingsError::Invalid("missing [sprint] table".into()))?;
        let double_tap_ticks = match sprint.get("double_tap_ticks") {
            Some(toml::Value::Integer(i)) if (1..=i64::from(u32::MAX)).contains(i) => *i as u32,
            _ => {
                return Err(BindingsError::Invalid(
                    "sprint.double_tap_ticks must be a whole number of at least 1".into(),
                ))
            }
        };

        let table = root
            .get("actions")
            .and_then(|v| v.as_table())
            .ok_or_else(|| BindingsError::Invalid("missing [actions] table".into()))?;
        let mut actions = HashMap::new();
        let mut owner: HashMap<String, &'static str> = HashMap::new();
        for (name, value) in table {
            let action = Action::from_name(name)
                .ok_or_else(|| BindingsError::UnknownAction(name.clone()))?;
            let t = value
                .as_table()
                .ok_or_else(|| BindingsError::Invalid(format!("`{name}` must be a table")))?;
            let keys = t
                .get("keys")
                .and_then(|v| v.as_array())
                .ok_or_else(|| BindingsError::Invalid(format!("`{name}` needs a keys array")))?
                .iter()
                .map(|k| {
                    k.as_str().map(str::to_owned).ok_or_else(|| {
                        BindingsError::Invalid(format!("`{name}` keys must be strings"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if keys.is_empty() {
                return Err(BindingsError::Invalid(format!("`{name}` has no keys")));
            }
            for key in &keys {
                if let Some(first) = owner.insert(key.clone(), action.name()) {
                    return Err(BindingsError::DuplicateKey {
                        key: key.clone(),
                        first,
                        second: action.name(),
                    });
                }
            }
            let mode = match t.get("mode").and_then(|v| v.as_str()) {
                Some("hold") => Mode::Hold,
                Some("edge") => Mode::Edge,
                Some("continuous") => Mode::Continuous,
                other => {
                    return Err(BindingsError::Invalid(format!(
                        "`{name}` has bad mode {other:?}"
                    )))
                }
            };
            let hold_repeat = match t.get("hold_repeat") {
                None => false,
                Some(v) => v.as_bool().ok_or_else(|| {
                    BindingsError::Invalid(format!("`{name}` hold_repeat must be a bool"))
                })?,
            };
            actions.insert(
                action,
                Binding {
                    keys,
                    mode,
                    hold_repeat,
                },
            );
        }
        for a in Action::ALL {
            if !actions.contains_key(&a) {
                return Err(BindingsError::MissingAction(a.name()));
            }
        }
        Ok(Bindings {
            sensitivity,
            fov_degrees,
            double_tap_ticks,
            actions,
        })
    }

    pub fn get(&self, action: Action) -> &Binding {
        &self.actions[&action]
    }

    /// The action a key or pseudo-key is bound to, if any.
    pub fn action_for_key(&self, key: &str) -> Option<Action> {
        Action::ALL
            .into_iter()
            .find(|a| self.actions[a].keys.iter().any(|k| k == key))
    }
}

fn number(t: &toml::value::Table, key: &str) -> Result<f32, BindingsError> {
    match t.get(key) {
        Some(toml::Value::Float(f)) if f.is_finite() => Ok(*f as f32),
        Some(toml::Value::Integer(i)) => Ok(*i as f32),
        _ => Err(BindingsError::Invalid(format!(
            "look.{key} must be a finite number"
        ))),
    }
}

#[cfg(test)]
mod tests;
