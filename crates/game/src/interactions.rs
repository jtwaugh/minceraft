//! Interaction rules loaded from `data/interactions.toml`.
//! See docs/design/basic-gameplay.md section 7.
//!
//! This module parses the table and resolves a press against it (first match
//! wins), calling stub actions until Phases 6 and 7.
//!
//! The table is parsed here. Block names and entity kinds are kept
//! as written and not checked against the block table, because some of them
//! (`farmland`, `crop`, `planter_dirt`, animals) arrive in later phases.

use crate::input::TickInput;
use crate::targeting::{self, EntityId, Face};
use std::fmt;

const DEFAULT_INTERACTIONS: &str = include_str!("../../../data/interactions.toml");

const RULE_FIELDS: [&str; 7] = [
    "held",
    "button",
    "target",
    "target_face",
    "require_air_above",
    "require_state",
    "action",
];

/// What the player must be holding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Held {
    Empty,
    Any,
    AnyTool,
    Tool(String),
    Item(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Primary,
    Secondary,
}

/// What the crosshair is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Block(String),
    Entity(String),
    None,
}

/// The function a matching rule runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Attack,
    Till,
    Plant,
    Harvest,
    Forage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub held: Held,
    pub button: Button,
    pub target: Target,
    pub target_face: Option<String>,
    pub require_air_above: bool,
    pub require_state: Option<String>,
    pub action: ActionKind,
}

/// The rule list, in file order (first match wins).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interactions {
    pub rules: Vec<Rule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InteractionsError {
    Parse(String),
    /// `rule` is the 1-based position in the file; 0 means the top level.
    UnknownField {
        rule: usize,
        field: String,
    },
    MissingField {
        rule: usize,
        field: &'static str,
    },
    WrongType {
        rule: usize,
        field: &'static str,
    },
    UnknownHeld {
        rule: usize,
        value: String,
    },
    UnknownButton {
        rule: usize,
        value: String,
    },
    UnknownTarget {
        rule: usize,
        value: String,
    },
    UnknownAction {
        rule: usize,
        value: String,
    },
}

impl fmt::Display for InteractionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InteractionsError::Parse(e) => write!(f, "interactions.toml: {e}"),
            InteractionsError::UnknownField { rule, field } => {
                write!(f, "interactions.toml: rule {rule}: unknown field `{field}`")
            }
            InteractionsError::MissingField { rule, field } => {
                write!(f, "interactions.toml: rule {rule}: missing field `{field}`")
            }
            InteractionsError::WrongType { rule, field } => {
                write!(
                    f,
                    "interactions.toml: rule {rule}: wrong type for `{field}`"
                )
            }
            InteractionsError::UnknownHeld { rule, value } => {
                write!(f, "interactions.toml: rule {rule}: unknown held `{value}`")
            }
            InteractionsError::UnknownButton { rule, value } => {
                write!(
                    f,
                    "interactions.toml: rule {rule}: unknown button `{value}`"
                )
            }
            InteractionsError::UnknownTarget { rule, value } => {
                write!(
                    f,
                    "interactions.toml: rule {rule}: unknown target `{value}`"
                )
            }
            InteractionsError::UnknownAction { rule, value } => {
                write!(
                    f,
                    "interactions.toml: rule {rule}: unknown action `{value}`"
                )
            }
        }
    }
}

impl std::error::Error for InteractionsError {}

/// A `kind:name` value with a non-empty name.
fn split_named(value: &str) -> Option<(&str, &str)> {
    value.split_once(':').filter(|(_, name)| !name.is_empty())
}

fn parse_held(value: &str) -> Option<Held> {
    match value {
        "empty" => Some(Held::Empty),
        "any" => Some(Held::Any),
        "any_tool" => Some(Held::AnyTool),
        _ => match split_named(value)? {
            ("tool", name) => Some(Held::Tool(name.to_string())),
            ("item", name) => Some(Held::Item(name.to_string())),
            _ => None,
        },
    }
}

fn parse_button(value: &str) -> Option<Button> {
    match value {
        "primary" => Some(Button::Primary),
        "secondary" => Some(Button::Secondary),
        _ => None,
    }
}

fn parse_target(value: &str) -> Option<Target> {
    if value == "none" {
        return Some(Target::None);
    }
    match split_named(value)? {
        ("block", name) => Some(Target::Block(name.to_string())),
        ("entity", kind) => Some(Target::Entity(kind.to_string())),
        _ => None,
    }
}

fn parse_action(value: &str) -> Option<ActionKind> {
    match value {
        "attack" => Some(ActionKind::Attack),
        "till" => Some(ActionKind::Till),
        "plant" => Some(ActionKind::Plant),
        "harvest" => Some(ActionKind::Harvest),
        "forage" => Some(ActionKind::Forage),
        _ => None,
    }
}

fn parse_rule(rule: usize, table: &toml::value::Table) -> Result<Rule, InteractionsError> {
    if let Some(field) = table.keys().find(|k| !RULE_FIELDS.contains(&k.as_str())) {
        return Err(InteractionsError::UnknownField {
            rule,
            field: field.clone(),
        });
    }
    let optional_str = |field: &'static str| -> Result<Option<String>, InteractionsError> {
        match table.get(field) {
            None => Ok(None),
            Some(toml::Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(InteractionsError::WrongType { rule, field }),
        }
    };
    let required_str = |field: &'static str| -> Result<String, InteractionsError> {
        optional_str(field)?.ok_or(InteractionsError::MissingField { rule, field })
    };

    let held = required_str("held")?;
    let button = required_str("button")?;
    let target = required_str("target")?;
    let action = required_str("action")?;
    let require_air_above = match table.get("require_air_above") {
        None => false,
        Some(toml::Value::Boolean(b)) => *b,
        Some(_) => {
            return Err(InteractionsError::WrongType {
                rule,
                field: "require_air_above",
            })
        }
    };

    Ok(Rule {
        held: parse_held(&held).ok_or(InteractionsError::UnknownHeld { rule, value: held })?,
        button: parse_button(&button).ok_or(InteractionsError::UnknownButton {
            rule,
            value: button,
        })?,
        target: parse_target(&target).ok_or(InteractionsError::UnknownTarget {
            rule,
            value: target,
        })?,
        target_face: optional_str("target_face")?,
        require_air_above,
        require_state: optional_str("require_state")?,
        action: parse_action(&action).ok_or(InteractionsError::UnknownAction {
            rule,
            value: action,
        })?,
    })
}

impl Interactions {
    /// The rules shipped in `data/interactions.toml`.
    pub fn default_rules() -> Interactions {
        Interactions::from_toml(DEFAULT_INTERACTIONS)
            .expect("data/interactions.toml must be a valid interaction table")
    }

    pub fn from_toml(text: &str) -> Result<Interactions, InteractionsError> {
        let root: toml::Value = text
            .parse()
            .map_err(|e: toml::de::Error| InteractionsError::Parse(e.to_string()))?;
        let root = root
            .as_table()
            .ok_or_else(|| InteractionsError::Parse("expected a table".into()))?;
        if let Some(key) = root.keys().find(|k| k.as_str() != "rule") {
            return Err(InteractionsError::UnknownField {
                rule: 0,
                field: key.clone(),
            });
        }
        let list = match root.get("rule") {
            None => return Ok(Interactions { rules: Vec::new() }),
            Some(toml::Value::Array(a)) => a,
            Some(_) => return Err(InteractionsError::Parse("`rule` must be an array".into())),
        };
        let rules = list
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let table = v
                    .as_table()
                    .ok_or_else(|| InteractionsError::Parse("`rule` must hold tables".into()))?;
                parse_rule(i + 1, table)
            })
            .collect::<Result<_, _>>()?;
        Ok(Interactions { rules })
    }
}

/// What the player holds, as far as rules care.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeldThing {
    Empty,
    Tool(String),
    Item(String),
}

/// What is known about the targeted thing. Names are as written in the
/// rules (`grass`, `crop`, `animal`); the face comes from the target itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetFacts {
    pub block: Option<String>,
    pub entity: Option<String>,
    pub air_above: bool,
    pub crop_state: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolveCtx {
    pub held: HeldThing,
    pub button: Button,
    pub target: targeting::Target,
    pub facts: TargetFacts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub rule_index: usize,
    pub action: ActionKind,
}

fn face_name(face: Face) -> &'static str {
    match face {
        Face::Bottom => "bottom",
        Face::Top => "top",
        Face::North => "north",
        Face::South => "south",
        Face::West => "west",
        Face::East => "east",
    }
}

fn held_matches(rule: &Held, held: &HeldThing) -> bool {
    match (rule, held) {
        (Held::Any, _) => true,
        (Held::Empty, HeldThing::Empty) => true,
        (Held::AnyTool, HeldThing::Tool(_)) => true,
        (Held::Tool(a), HeldThing::Tool(b)) => a == b,
        (Held::Item(a), HeldThing::Item(b)) => a == b,
        _ => false,
    }
}

fn rule_matches(rule: &Rule, ctx: &ResolveCtx) -> bool {
    if rule.button != ctx.button || !held_matches(&rule.held, &ctx.held) {
        return false;
    }
    let face = match (&rule.target, &ctx.target) {
        (Target::None, targeting::Target::None) => None,
        (Target::Block(name), targeting::Target::Block { face, .. })
            if ctx.facts.block.as_deref() == Some(name) =>
        {
            Some(*face)
        }
        (Target::Entity(kind), targeting::Target::Entity { .. })
            if ctx.facts.entity.as_deref() == Some(kind) =>
        {
            None
        }
        _ => return false,
    };
    if let Some(want) = &rule.target_face {
        if face.map(face_name) != Some(want.as_str()) {
            return false;
        }
    }
    if rule.require_air_above && !ctx.facts.air_above {
        return false;
    }
    if let Some(want) = &rule.require_state {
        if ctx.facts.crop_state.as_ref() != Some(want) {
            return false;
        }
    }
    true
}

/// The first rule, in file order, that matches `ctx`.
pub fn resolve(rules: &[Rule], ctx: &ResolveCtx) -> Option<Resolved> {
    rules
        .iter()
        .position(|r| rule_matches(r, ctx))
        .map(|rule_index| Resolved {
            rule_index,
            action: rules[rule_index].action,
        })
}

/// An attack that landed on an entity, for the caller to apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub id: EntityId,
    pub damage: u32,
}

/// Attack the targeted entity for `damage`. Swings at blocks or nothing do
/// nothing (section 8.1).
pub fn attack(target: &targeting::Target, damage: u32) -> Option<Hit> {
    match target {
        targeting::Target::Entity { id } => Some(Hit { id: *id, damage }),
        _ => None,
    }
}

// Stub action: real behaviour comes in Phase 7. It reports that it ran.
// Till, plant and harvest run in `walk.rs` through `till::Tiller`,
// `plant::plant` and `harvest::harvest`.
pub fn forage() -> ActionKind {
    ActionKind::Forage
}

fn run_action(
    action: ActionKind,
    ctx: &ResolveCtx,
    damage: u32,
    hits: &mut Vec<Hit>,
) -> ActionKind {
    match action {
        ActionKind::Attack => hits.extend(attack(&ctx.target, damage)),
        ActionKind::Till | ActionKind::Plant | ActionKind::Harvest => {}
        ActionKind::Forage => {
            forage();
        }
    }
    action
}

/// Per-tick dispatch state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dispatcher {
    /// Rule matched by the most recent press edge; `None` if that press
    /// matched nothing. Unchanged on ticks with no press. For the overlay.
    pub last_rule: Option<usize>,
    /// Attacks that landed in the most recent `dispatch` call.
    pub hits: Vec<Hit>,
}

impl Dispatcher {
    /// Resolves each press edge in `input` (primary, then secondary) against
    /// `ctx` (its `button` is overridden) and runs the matched action; an
    /// attack deals `damage` and lands in `self.hits`. Returns the actions
    /// that ran: at most one per edge, none without a press.
    pub fn dispatch(
        &mut self,
        rules: &Interactions,
        input: &TickInput,
        ctx: &ResolveCtx,
        damage: u32,
    ) -> Vec<ActionKind> {
        let mut ran = Vec::new();
        self.hits.clear();
        for (pressed, button) in [
            (input.primary, Button::Primary),
            (input.secondary, Button::Secondary),
        ] {
            if !pressed {
                continue;
            }
            let ctx = ResolveCtx {
                button,
                ..ctx.clone()
            };
            let resolved = resolve(&rules.rules, &ctx);
            self.last_rule = resolved.map(|r| r.rule_index);
            if let Some(r) = resolved {
                ran.push(run_action(r.action, &ctx, damage, &mut self.hits));
            }
        }
        ran
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(held: HeldThing, button: Button, block: &str, face: Face) -> ResolveCtx {
        ResolveCtx {
            held,
            button,
            target: targeting::Target::Block {
                pos: [0, 0, 0],
                face,
            },
            facts: TargetFacts {
                block: Some(block.into()),
                air_above: true,
                ..TargetFacts::default()
            },
        }
    }

    fn animal(held: HeldThing) -> ResolveCtx {
        ResolveCtx {
            held,
            button: Button::Primary,
            target: targeting::Target::Entity { id: 1 },
            facts: TargetFacts {
                entity: Some("animal".into()),
                ..TargetFacts::default()
            },
        }
    }

    fn resolved(c: &ResolveCtx) -> Option<Resolved> {
        resolve(&Interactions::default_rules().rules, c)
    }

    fn action(c: &ResolveCtx) -> Option<ActionKind> {
        resolved(c).map(|r| r.action)
    }

    fn tool(n: &str) -> HeldThing {
        HeldThing::Tool(n.into())
    }

    #[test]
    fn specific_rules_beat_general_ones() {
        let c = ctx(tool("hoe"), Button::Secondary, "grass", Face::Top);
        let r = resolved(&c).unwrap();
        assert_eq!((r.rule_index, r.action), (1, ActionKind::Till));
        // The sword rule comes before the punch fallback.
        assert_eq!(resolved(&animal(tool("sword"))).unwrap().rule_index, 0);
    }

    #[test]
    fn punch_fallback_matches_with_no_tool() {
        let r = resolved(&animal(HeldThing::Empty)).unwrap();
        assert_eq!((r.rule_index, r.action), (8, ActionKind::Attack));
    }

    #[test]
    fn air_above_blocks_till() {
        let mut c = ctx(tool("hoe"), Button::Secondary, "grass", Face::Top);
        c.facts.air_above = false;
        assert_eq!(action(&c), None);
        c.facts.block = Some("planter_dirt".into());
        assert_eq!(action(&c), Some(ActionKind::Till));
    }

    #[test]
    fn top_face_required_to_plant() {
        let seeds = HeldThing::Item("seeds".into());
        let mut c = ctx(seeds, Button::Secondary, "farmland", Face::Top);
        assert_eq!(action(&c), Some(ActionKind::Plant));
        c.target = targeting::Target::Block {
            pos: [0, 0, 0],
            face: Face::North,
        };
        assert_eq!(action(&c), None);
    }

    #[test]
    fn harvest_needs_mature_crop() {
        let mut c = ctx(HeldThing::Empty, Button::Secondary, "crop", Face::Top);
        assert_eq!(action(&c), None);
        c.facts.crop_state = Some("growing".into());
        assert_eq!(action(&c), None);
        c.facts.crop_state = Some("mature".into());
        assert_eq!(action(&c), Some(ActionKind::Harvest));
        c.held = tool("sword");
        assert_eq!(action(&c), Some(ActionKind::Harvest));
        c.held = HeldThing::Item("seeds".into());
        assert_eq!(action(&c), None);
    }

    #[test]
    fn button_and_target_must_match() {
        let c = ctx(HeldThing::Empty, Button::Primary, "tall_grass", Face::Top);
        assert_eq!(action(&c), None);
        let c = ResolveCtx {
            target: targeting::Target::None,
            ..c
        };
        assert_eq!(action(&c), None);
    }

    #[test]
    fn dispatch_runs_once_per_press_edge() {
        let rules = Interactions::default_rules();
        let mut d = Dispatcher::default();
        let c = animal(tool("sword"));
        let idle = TickInput::default();
        assert!(d.dispatch(&rules, &idle, &c, 4).is_empty());
        assert_eq!(d.last_rule, None);
        let press = TickInput {
            primary: true,
            ..TickInput::default()
        };
        assert_eq!(d.dispatch(&rules, &press, &c, 4), vec![ActionKind::Attack]);
        assert_eq!(d.last_rule, Some(0));
        assert_eq!(d.hits, vec![Hit { id: 1, damage: 4 }]);
        // No new press: nothing runs, the overlay keeps the last match.
        assert!(d.dispatch(&rules, &idle, &c, 4).is_empty());
        assert_eq!(d.last_rule, Some(0));
        // A press that matches nothing clears it.
        let none = ResolveCtx {
            target: targeting::Target::None,
            ..c
        };
        assert!(d.dispatch(&rules, &press, &none, 4).is_empty());
        assert_eq!(d.last_rule, None);
    }

    const BASE: &str =
        "held = \"empty\"\nbutton = \"secondary\"\ntarget = \"none\"\naction = \"forage\"\n";

    fn with_base(text: &str) -> Result<Interactions, InteractionsError> {
        Interactions::from_toml(&format!("[[rule]]\n{text}"))
    }

    fn rule(held: Held, button: Button, target: Target, action: ActionKind) -> Rule {
        Rule {
            held,
            button,
            target,
            target_face: None,
            require_air_above: false,
            require_state: None,
            action,
        }
    }

    #[test]
    fn shipped_file_holds_section_7_in_order() {
        let block = |n: &str| Target::Block(n.to_string());
        let animal = Target::Entity("animal".to_string());
        let hoe = || Held::Tool("hoe".into());
        let mut expected = vec![
            rule(
                Held::Tool("sword".into()),
                Button::Primary,
                animal.clone(),
                ActionKind::Attack,
            ),
            rule(hoe(), Button::Secondary, block("grass"), ActionKind::Till),
            rule(hoe(), Button::Secondary, block("dirt"), ActionKind::Till),
            rule(
                hoe(),
                Button::Secondary,
                block("planter_dirt"),
                ActionKind::Till,
            ),
            rule(
                Held::Item("seeds".into()),
                Button::Secondary,
                block("farmland"),
                ActionKind::Plant,
            ),
            rule(
                Held::Empty,
                Button::Secondary,
                block("crop"),
                ActionKind::Harvest,
            ),
            rule(
                Held::AnyTool,
                Button::Secondary,
                block("crop"),
                ActionKind::Harvest,
            ),
            rule(
                Held::Empty,
                Button::Secondary,
                block("tall_grass"),
                ActionKind::Forage,
            ),
            rule(Held::Any, Button::Primary, animal, ActionKind::Attack),
        ];
        expected[1].require_air_above = true;
        expected[2].require_air_above = true;
        expected[4].require_air_above = true;
        expected[4].target_face = Some("top".into());
        expected[5].require_state = Some("mature".into());
        expected[6].require_state = Some("mature".into());
        assert_eq!(Interactions::default_rules().rules, expected);
    }

    #[test]
    fn base_rule_loads_with_defaults() {
        let r = with_base(BASE).unwrap().rules;
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].target, Target::None);
        assert!(!r[0].require_air_above);
        assert_eq!(r[0].target_face, None);
    }

    #[test]
    fn empty_file_has_no_rules() {
        assert!(Interactions::from_toml("").unwrap().rules.is_empty());
    }

    #[test]
    fn unknown_action_is_rejected() {
        let err = with_base(&BASE.replace("forage", "dance")).unwrap_err();
        assert_eq!(
            err,
            InteractionsError::UnknownAction {
                rule: 1,
                value: "dance".into()
            }
        );
        assert!(err.to_string().contains("unknown action `dance`"));
    }

    #[test]
    fn bad_button_is_rejected() {
        let err = with_base(&BASE.replace("secondary", "middle")).unwrap_err();
        assert!(matches!(
            err,
            InteractionsError::UnknownButton { rule: 1, .. }
        ));
    }

    #[test]
    fn missing_action_is_rejected() {
        let err = with_base(&BASE.replace("action = \"forage\"\n", "")).unwrap_err();
        assert_eq!(
            err,
            InteractionsError::MissingField {
                rule: 1,
                field: "action"
            }
        );
    }

    #[test]
    fn unknown_field_is_rejected() {
        let err = with_base(&format!("{BASE}colour = \"red\"\n")).unwrap_err();
        assert_eq!(
            err,
            InteractionsError::UnknownField {
                rule: 1,
                field: "colour".into()
            }
        );
    }

    #[test]
    fn bad_held_and_target_are_rejected() {
        for bad in ["tool:", "weapon:sword", "sword", ""] {
            let err = with_base(&BASE.replace("\"empty\"", &format!("\"{bad}\""))).unwrap_err();
            assert!(
                matches!(err, InteractionsError::UnknownHeld { .. }),
                "{bad}"
            );
        }
        for bad in ["block:", "mob:cow", "block"] {
            let err = with_base(&BASE.replace("\"none\"", &format!("\"{bad}\""))).unwrap_err();
            assert!(
                matches!(err, InteractionsError::UnknownTarget { .. }),
                "{bad}"
            );
        }
    }

    #[test]
    fn wrong_types_are_rejected() {
        let err = with_base(&format!("{BASE}require_air_above = \"yes\"\n")).unwrap_err();
        assert!(matches!(err, InteractionsError::WrongType { .. }));
        let err = with_base(&format!("{BASE}target_face = 3\n")).unwrap_err();
        assert!(matches!(err, InteractionsError::WrongType { .. }));
    }

    #[test]
    fn syntax_errors_and_stray_keys_are_rejected() {
        assert!(matches!(
            Interactions::from_toml("[[rule"),
            Err(InteractionsError::Parse(_))
        ));
        assert!(matches!(
            Interactions::from_toml("version = 1"),
            Err(InteractionsError::UnknownField { .. })
        ));
    }

    #[test]
    fn error_reports_the_failing_rule_number() {
        let text = format!("[[rule]]\n{BASE}[[rule]]\n{}", BASE.replace("forage", "x"));
        let err = Interactions::from_toml(&text).unwrap_err();
        assert!(matches!(
            err,
            InteractionsError::UnknownAction { rule: 2, .. }
        ));
    }
}
