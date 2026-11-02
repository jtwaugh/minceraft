//! HUD message text loaded from `data/messages.toml`.
//! See docs/design/action-feedback.md section 2.

use crate::bindings::{Action, Bindings};

const DEFAULT_MESSAGES: &str = include_str!("../../../data/messages.toml");

/// Events that get a message, with the fields each carries (as
/// `Walk::drain_events` writes them). Refusals are keyed by reason instead.
pub const EVENT_FIELDS: &[(&str, &[&str])] = &[
    ("item_picked_up", &["item", "player"]),
    ("block_tilled", &["position", "from", "to"]),
    ("crop_planted", &["position", "crop"]),
    ("crop_harvested", &["position", "crop", "yields"]),
    ("foraged", &["position", "yields"]),
];

/// The fields of `action_refused`, which every reason string may use.
pub const REFUSAL_FIELDS: &[&str] = &["action", "reason"];

/// The placeholder every string may use besides its event's fields.
pub const DROP_KEY: &str = "drop_key";

/// Message templates and timing, each table sorted by key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Messages {
    /// Ticks a message shows at full strength.
    pub hold_ticks: u32,
    /// Ticks it then takes to fade out.
    pub fade_ticks: u32,
    /// Event name -> template.
    pub events: Vec<(String, String)>,
    /// Refusal reason name -> template.
    pub reasons: Vec<(String, String)>,
}

impl Messages {
    /// The shipped strings from `data/messages.toml`.
    pub fn default_messages() -> Messages {
        Messages::parse(DEFAULT_MESSAGES).expect("data/messages.toml is valid")
    }

    /// Parse messages text; every key shown in the shipped file is required.
    pub fn parse(text: &str) -> Result<Messages, String> {
        let root: toml::value::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        let table = |name: &str| {
            root.get(name)
                .and_then(|v| v.as_table())
                .ok_or_else(|| format!("messages.toml: `[{name}]` must be a table"))
        };
        let ticks = |key: &str| {
            table("timing")?
                .get(key)
                .and_then(|v| v.as_integer())
                .and_then(|n| u32::try_from(n).ok())
                .filter(|&n| n > 0)
                .ok_or_else(|| format!("messages.toml: `timing.{key}` must be a positive integer"))
        };
        let strings = |name: &str| -> Result<Vec<(String, String)>, String> {
            table(name)?
                .iter()
                .map(|(k, v)| match v.as_str() {
                    Some(s) if !s.is_empty() => Ok((k.clone(), s.to_string())),
                    _ => Err(format!(
                        "messages.toml: `{name}.{k}` must be a non-empty string"
                    )),
                })
                .collect()
        };
        Ok(Messages {
            hold_ticks: ticks("hold_ticks")?,
            fade_ticks: ticks("fade_ticks")?,
            events: strings("events")?,
            reasons: strings("reasons")?,
        })
    }
}

/// The `{name}` placeholders in a template, in order.
pub fn placeholders(template: &str) -> Vec<&str> {
    template
        .split('{')
        .skip(1)
        .filter_map(|s| s.split_once('}').map(|(name, _)| name))
        .collect()
}

/// How a key shows in text: `KeyQ` is `Q`, `Digit1` is `1`, anything else
/// is its `KeyboardEvent.code` name.
pub fn key_label(code: &str) -> &str {
    code.strip_prefix("Key")
        .or_else(|| code.strip_prefix("Digit"))
        .unwrap_or(code)
}

/// The label of the first key bound to `drop`, for `{drop_key}`.
pub fn drop_key(bindings: &Bindings) -> String {
    bindings
        .get(Action::Drop)
        .keys
        .first()
        .map_or_else(String::new, |k| key_label(k).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Reason;

    /// Every `Reason`; the match stops compiling when one is added.
    fn all_reasons() -> Vec<Reason> {
        let all = vec![
            Reason::TooFar,
            Reason::HandsFull,
            Reason::ToolsStowed,
            Reason::NothingHere,
            Reason::ChestFull,
        ];
        for r in &all {
            match r {
                Reason::TooFar
                | Reason::HandsFull
                | Reason::ToolsStowed
                | Reason::NothingHere
                | Reason::ChestFull => {}
            }
        }
        all
    }

    fn lookup<'a>(table: &'a [(String, String)], key: &str) -> Option<&'a str> {
        table
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn every_listed_event_and_reason_has_a_string_with_known_placeholders() {
        let m = Messages::default_messages();
        for (event, fields) in EVENT_FIELDS {
            let text = lookup(&m.events, event).unwrap_or_else(|| panic!("no text for {event}"));
            for p in placeholders(text) {
                assert!(
                    fields.contains(&p) || p == DROP_KEY,
                    "{event}: unknown placeholder {{{p}}}"
                );
            }
        }
        for r in all_reasons() {
            let name = r.name();
            let text = lookup(&m.reasons, name).unwrap_or_else(|| panic!("no text for {name}"));
            for p in placeholders(text) {
                assert!(
                    REFUSAL_FIELDS.contains(&p) || p == DROP_KEY,
                    "{name}: unknown placeholder {{{p}}}"
                );
            }
        }
        assert_eq!(m.events.len(), EVENT_FIELDS.len(), "unlisted event");
        assert_eq!(m.reasons.len(), all_reasons().len(), "unknown reason");
    }

    #[test]
    fn hands_full_takes_the_drop_key_from_the_bindings() {
        let m = Messages::default_messages();
        let text = lookup(&m.reasons, "hands_full").unwrap();
        assert_eq!(placeholders(text), vec![DROP_KEY]);
        assert!(!text.contains('Q'));
        assert_eq!(drop_key(&Bindings::default_from_data().unwrap()), "Q");
    }

    #[test]
    fn timing_is_about_two_seconds() {
        let m = Messages::default_messages();
        let total = m.hold_ticks + m.fade_ticks;
        assert!((30..=50).contains(&total), "{total} ticks");
    }

    #[test]
    fn placeholders_and_key_labels() {
        assert_eq!(placeholders("a {x} b {y}"), vec!["x", "y"]);
        assert!(placeholders("none").is_empty());
        assert_eq!(key_label("KeyQ"), "Q");
        assert_eq!(key_label("Digit3"), "3");
        assert_eq!(key_label("Space"), "Space");
    }

    #[test]
    fn bad_files_are_refused() {
        let ok = "[timing]\nhold_ticks = 1\nfade_ticks = 1\n[events]\n[reasons]\n";
        assert!(Messages::parse(ok).is_ok());
        assert!(Messages::parse(&ok.replace("hold_ticks = 1", "hold_ticks = 0")).is_err());
        assert!(Messages::parse(&ok.replace("[reasons]\n", "")).is_err());
        assert!(Messages::parse(&format!("{ok}too_far = 3\n")).is_err());
    }
}
