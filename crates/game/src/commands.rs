//! Debug console command core: registry and parser (docs/design/debug-console.md
//! Phase 1). A command is one [`Command`] entry plus a handler that gets the
//! world `W` it acts on and returns its result lines.

use crate::items::{ItemInstance, ToolInstance};
use crate::walk::Walker;
use std::fmt;

/// The game states a command can be restricted to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameState {
    Walking,
    Flying,
}

impl fmt::Display for GameState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            GameState::Walking => "walking",
            GameState::Flying => "flying",
        })
    }
}

pub const ALL_STATES: &[GameState] = &[GameState::Walking, GameState::Flying];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    Word,
    Int,
}

#[derive(Clone, Copy, Debug)]
pub struct ArgSpec {
    pub name: &'static str,
    pub kind: ArgKind,
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    Word(String),
    Int(i64),
}

/// What a handler receives: the world to act on and the registry (for `help`).
pub struct Ctx<'a, W> {
    pub world: &'a mut W,
    pub registry: &'a Registry<W>,
}

/// A handler returns its result lines, or a message for an error line.
pub type Handler<W> = fn(&mut Ctx<'_, W>, &[Arg]) -> Result<Vec<String>, String>;

pub struct Command<W> {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub args: &'static [ArgSpec],
    pub help: &'static str,
    pub states: &'static [GameState],
    pub cheat: bool,
    pub handler: Handler<W>,
}

impl<W> Command<W> {
    /// `name <required> [optional]`.
    /// Whether `word` is this command's name or one of its aliases; the one
    /// place name matching is defined.
    pub fn answers_to(&self, word: &str) -> bool {
        self.name == word || self.aliases.contains(&word)
    }

    pub fn usage(&self) -> String {
        let mut s = self.name.to_string();
        for a in self.args {
            let (open, close) = if a.optional { ('[', ']') } else { ('<', '>') };
            s.push_str(&format!(" {open}{}{close}", a.name));
        }
        s
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateName {
        name: String,
    },
    /// An alias equal to a command name or to another alias.
    AliasShadows {
        alias: String,
        command: String,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::DuplicateName { name } => {
                write!(f, "commands: duplicate command name '{name}'")
            }
            RegistryError::AliasShadows { alias, command } => {
                write!(
                    f,
                    "commands: alias '{alias}' of '{command}' shadows another name"
                )
            }
        }
    }
}

impl std::error::Error for RegistryError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    Unknown(String),
    WrongArgCount {
        usage: String,
        got: usize,
    },
    BadArg {
        usage: String,
        arg: &'static str,
        value: String,
    },
    /// The command is not allowed in the current game state.
    WrongState {
        command: &'static str,
        state: GameState,
    },
    /// The handler ran and refused, with its message.
    Failed(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::Unknown(name) => {
                write!(f, "unknown command '{name}' (try 'help')")
            }
            CommandError::WrongArgCount { usage, got } => {
                write!(f, "wrong number of arguments ({got}); usage: {usage}")
            }
            CommandError::BadArg { usage, arg, value } => {
                write!(f, "bad value '{value}' for <{arg}>; usage: {usage}")
            }
            CommandError::WrongState { command, state } => {
                write!(f, "'{command}' can't be used while {state}")
            }
            CommandError::Failed(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for CommandError {}

pub struct Registry<W> {
    commands: Vec<Command<W>>,
}

impl<W> Registry<W> {
    /// Rejects a duplicate name, and an alias equal to any name or alias.
    pub fn new(commands: Vec<Command<W>>) -> Result<Self, RegistryError> {
        let mut seen: Vec<&str> = Vec::new();
        for c in &commands {
            if seen.contains(&c.name) {
                return Err(RegistryError::DuplicateName {
                    name: c.name.into(),
                });
            }
            seen.push(c.name);
        }
        for c in &commands {
            for a in c.aliases {
                if seen.contains(a) {
                    return Err(RegistryError::AliasShadows {
                        alias: (*a).into(),
                        command: c.name.into(),
                    });
                }
                seen.push(a);
            }
        }
        Ok(Registry { commands })
    }

    pub fn get(&self, name: &str) -> Option<&Command<W>> {
        self.commands.iter().find(|c| c.answers_to(name))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Command<W>> {
        self.commands.iter()
    }
}

/// Splits a line into a command and its typed arguments. `Ok(None)` for a
/// blank line.
pub fn parse<'r, W>(
    registry: &'r Registry<W>,
    line: &str,
) -> Result<Option<(&'r Command<W>, Vec<Arg>)>, CommandError> {
    let mut words = line.split_whitespace();
    let Some(name) = words.next() else {
        return Ok(None);
    };
    let cmd = registry
        .get(name)
        .ok_or_else(|| CommandError::Unknown(name.into()))?;
    let words: Vec<&str> = words.collect();
    let required = cmd.args.iter().filter(|a| !a.optional).count();
    if words.len() < required || words.len() > cmd.args.len() {
        return Err(CommandError::WrongArgCount {
            usage: cmd.usage(),
            got: words.len(),
        });
    }
    let mut args = Vec::new();
    for (spec, w) in cmd.args.iter().zip(words) {
        args.push(match spec.kind {
            ArgKind::Word => Arg::Word(w.into()),
            ArgKind::Int => Arg::Int(w.parse().map_err(|_| CommandError::BadArg {
                usage: cmd.usage(),
                arg: spec.name,
                value: w.into(),
            })?),
        });
    }
    Ok(Some((cmd, args)))
}

/// Runs one line against `world` in game state `state` and returns its output
/// lines. A parse error, or a command not valid in `state`, gives one error
/// line and changes nothing. `cheated` is set when a cheat command succeeds.
pub fn run_line<W>(
    registry: &Registry<W>,
    world: &mut W,
    state: GameState,
    line: &str,
    cheated: &mut bool,
) -> Vec<String> {
    let (cmd, args) = match parse(registry, line) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return Vec::new(),
        Err(e) => return vec![e.to_string()],
    };
    if !cmd.states.contains(&state) {
        return vec![format!("'{}' can't be used in this state", cmd.name)];
    }
    let mut ctx = Ctx { world, registry };
    match (cmd.handler)(&mut ctx, &args) {
        Ok(lines) => {
            if cmd.cheat {
                *cheated = true;
            }
            lines
        }
        Err(msg) => vec![msg],
    }
}

/// One line of console output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Result(String),
    Error(String),
}

/// The one definition of the built-in `help`, used by its listing line, by
/// `help help` and by argument checking.
fn help_command<W>() -> Command<W> {
    Command {
        name: "help",
        aliases: &[],
        args: &[ArgSpec {
            name: "command",
            kind: ArgKind::Word,
            optional: true,
        }],
        help: "show this list or one command",
        states: ALL_STATES,
        cheat: false,
        handler: |_, _| Ok(Vec::new()),
    }
}

/// A registry plus the session state around it: the outcome buffer and the
/// `cheated` flag. The built-in `help` is handled here, not in the registry.
pub struct Console<W> {
    registry: Registry<W>,
    help: Command<W>,
    outcome: Vec<Outcome>,
    cheated: bool,
}

impl<W> Console<W> {
    /// Rejects a registry that already uses the name `help`.
    pub fn new(registry: Registry<W>) -> Result<Self, RegistryError> {
        let help = help_command();
        if registry.get(help.name).is_some() {
            return Err(RegistryError::DuplicateName {
                name: help.name.into(),
            });
        }
        Ok(Console {
            registry,
            help,
            outcome: Vec::new(),
            cheated: false,
        })
    }

    pub fn cheated(&self) -> bool {
        self.cheated
    }

    /// Takes the buffered output lines, oldest first.
    pub fn drain(&mut self) -> Vec<Outcome> {
        std::mem::take(&mut self.outcome)
    }

    /// Runs one line: parse, state check, handler. Output goes to the buffer.
    /// `cheated` is set only when a cheat command's handler succeeds.
    pub fn run(&mut self, world: &mut W, state: GameState, line: &str) {
        match self.execute(world, state, line) {
            Ok(lines) => self.outcome.extend(lines.into_iter().map(Outcome::Result)),
            Err(e) => self.outcome.push(Outcome::Error(e.to_string())),
        }
    }

    fn execute(
        &mut self,
        world: &mut W,
        state: GameState,
        line: &str,
    ) -> Result<Vec<String>, CommandError> {
        let mut words = line.split_whitespace();
        if words.next().is_some_and(|w| self.help.answers_to(w)) {
            let rest: Vec<&str> = words.collect();
            if rest.len() > self.help.args.len() {
                return Err(CommandError::WrongArgCount {
                    usage: self.help.usage(),
                    got: rest.len(),
                });
            }
            return self.help(rest.first().copied());
        }
        let Some((cmd, args)) = parse(&self.registry, line)? else {
            return Ok(Vec::new());
        };
        if !cmd.states.contains(&state) {
            return Err(CommandError::WrongState {
                command: cmd.name,
                state,
            });
        }
        let mut ctx = Ctx {
            world,
            registry: &self.registry,
        };
        let lines = (cmd.handler)(&mut ctx, &args).map_err(CommandError::Failed)?;
        if cmd.cheat {
            self.cheated = true;
        }
        Ok(lines)
    }

    fn help(&self, topic: Option<&str>) -> Result<Vec<String>, CommandError> {
        let Some(topic) = topic else {
            let mut lines = vec![format!("{}: {}", self.help.usage(), self.help.help)];
            lines.extend(
                self.registry
                    .iter()
                    .map(|c| format!("{}: {}", c.usage(), c.help)),
            );
            return Ok(lines);
        };
        let c = if self.help.answers_to(topic) {
            &self.help
        } else {
            self.registry
                .get(topic)
                .ok_or_else(|| CommandError::Unknown(topic.into()))?
        };
        let mut lines = vec![format!("usage: {}", c.usage()), c.help.to_string()];
        if !c.aliases.is_empty() {
            lines.push(format!("aliases: {}", c.aliases.join(", ")));
        }
        if c.cheat {
            lines.push("cheat: sets the cheated flag".to_string());
        }
        Ok(lines)
    }
}

/// The commands a player session starts with: `help`, `give`, `spawn` and
/// `clear`. `Walker` runs lines with [`run_line`], so `help` is a registry
/// entry here rather than [`Console`]'s built-in.
pub fn default_registry() -> Result<Registry<Walker>, RegistryError> {
    let mut help = help_command();
    help.handler = help_handler;
    Registry::new(vec![
        help,
        give_command(),
        spawn_command(),
        clear_command(),
        tp_command(),
        pause_command(),
        resume_command(),
        step_command(),
    ])
}

/// `help` lists every command; `help <name>` describes one.
fn help_handler(ctx: &mut Ctx<'_, Walker>, args: &[Arg]) -> Result<Vec<String>, String> {
    match args.first() {
        None => {
            let mut lines: Vec<String> = ctx
                .registry
                .iter()
                .map(|c| format!("{}: {}", c.usage(), c.help))
                .collect();
            lines.extend(
                ctx.world
                    .aliases()
                    .aliases
                    .iter()
                    .map(|a| format!("{}: runs {}", a.name, a.expands.join("; "))),
            );
            Ok(lines)
        }
        Some(Arg::Word(name)) => {
            if let Some(a) = ctx.world.aliases().get(name) {
                return Ok(vec![format!("{name}: runs {}", a.expands.join("; "))]);
            }
            let c = ctx
                .registry
                .get(name)
                .ok_or_else(|| format!("unknown command '{name}' (try 'help')"))?;
            let mut lines = vec![format!("usage: {}", c.usage()), c.help.to_string()];
            if c.cheat {
                lines.push("cheat: sets the cheated flag".to_string());
            }
            Ok(lines)
        }
        Some(_) => Err("bad command name".into()),
    }
}

/// `tp <x> <z>`: stand on the surface of that column (docs/design/debug-console.md
/// Phase 3, decided in #423).
pub fn tp_command() -> Command<Walker> {
    Command {
        name: "tp",
        aliases: &[],
        args: &[
            ArgSpec {
                name: "x",
                kind: ArgKind::Int,
                optional: false,
            },
            ArgSpec {
                name: "z",
                kind: ArgKind::Int,
                optional: false,
            },
        ],
        help: "teleport onto the surface at column x z",
        states: ALL_STATES,
        cheat: true,
        handler: tp,
    }
}

fn tp(ctx: &mut Ctx<'_, Walker>, args: &[Arg]) -> Result<Vec<String>, String> {
    let (Some(Arg::Int(x)), Some(Arg::Int(z))) = (args.first(), args.get(1)) else {
        return Err("missing x or z".into());
    };
    let coord = |v: i64| i32::try_from(v).map_err(|_| format!("{v} is out of range"));
    let feet = ctx.world.teleport_to_surface(coord(*x)?, coord(*z)?)?;
    Ok(vec![format!(
        "teleported to {} {} {}",
        feet[0].floor(),
        feet[1],
        feet[2].floor()
    )])
}

/// `pause`, `resume` and `step [n]` (docs/design/debug-console.md Phase 3,
/// decided in #423, #421, #434 and #443). Not cheats.
pub const STEP_MAX: i64 = 1000;

pub fn pause_command() -> Command<Walker> {
    Command {
        name: "pause",
        aliases: &[],
        args: &[],
        help: "stop the game ticking; resume or step to continue",
        states: ALL_STATES,
        cheat: false,
        handler: pause,
    }
}

fn pause(ctx: &mut Ctx<'_, Walker>, _args: &[Arg]) -> Result<Vec<String>, String> {
    if ctx.world.paused() {
        return Ok(vec!["already paused".into()]);
    }
    ctx.world.set_paused(true);
    Ok(vec!["paused".into()])
}

pub fn resume_command() -> Command<Walker> {
    Command {
        name: "resume",
        aliases: &[],
        args: &[],
        help: "let a paused game tick again",
        states: ALL_STATES,
        cheat: false,
        handler: resume,
    }
}

fn resume(ctx: &mut Ctx<'_, Walker>, _args: &[Arg]) -> Result<Vec<String>, String> {
    if !ctx.world.paused() {
        return Ok(vec!["not paused".into()]);
    }
    ctx.world.set_paused(false);
    Ok(vec!["resumed".into()])
}

pub fn step_command() -> Command<Walker> {
    Command {
        name: "step",
        aliases: &[],
        args: &[ArgSpec {
            name: "n",
            kind: ArgKind::Int,
            optional: true,
        }],
        help: "while paused, advance n ticks (1 to 1000, default 1)",
        states: ALL_STATES,
        cheat: false,
        handler: step,
    }
}

fn step(ctx: &mut Ctx<'_, Walker>, args: &[Arg]) -> Result<Vec<String>, String> {
    let n = match args.first() {
        Some(Arg::Int(n)) => *n,
        _ => 1,
    };
    if !(1..=STEP_MAX).contains(&n) {
        return Err(format!("n must be between 1 and {STEP_MAX}"));
    }
    if !ctx.world.paused() {
        return Err("step needs the game paused".into());
    }
    ctx.world.request_steps(n as u32);
    Ok(vec![format!("stepping {n}")])
}

/// `give tool|item <name>`: one unit, following the belt and hands rules
/// (docs/design/debug-console.md sections 1 and 3; full cases from #315, #378).
pub fn give_command() -> Command<Walker> {
    Command {
        name: "give",
        aliases: &[],
        args: &[
            ArgSpec {
                name: "kind",
                kind: ArgKind::Word,
                optional: false,
            },
            ArgSpec {
                name: "name",
                kind: ArgKind::Word,
                optional: false,
            },
        ],
        help: "give tool <name> (first empty belt slot, selected) or item <name> (hands)",
        states: &[GameState::Walking],
        cheat: true,
        handler: give,
    }
}

fn give(ctx: &mut Ctx<'_, Walker>, args: &[Arg]) -> Result<Vec<String>, String> {
    let (Some(Arg::Word(kind)), Some(Arg::Word(name))) = (args.first(), args.get(1)) else {
        return Err("missing kind or name".into());
    };
    let walker = &mut *ctx.world;
    let feet = walker.player().pos;
    let at_feet = [feet[0], feet[1] + 0.5, feet[2]];
    let instance = || ItemInstance { item: name.clone() };
    match kind.as_str() {
        "tool" => {
            if !walker.knows_tool(name) {
                return Err(format!("unknown tool '{name}'"));
            }
            let state = walker.player_state_mut();
            // Slot 0 is the empty hand by design (basic-gameplay v0 belt).
            match (1..state.tool_belt.len()).find(|&i| state.tool_belt[i].is_none()) {
                Some(i) => {
                    state.tool_belt[i] = Some(ToolInstance { tool: name.clone() });
                    state.select(i as u8);
                    Ok(vec![format!("gave tool {name} (slot {})", i + 1)])
                }
                None => {
                    walker.spawn_item(instance(), at_feet);
                    Ok(vec![format!("belt full: dropped {name} at your feet")])
                }
            }
        }
        "item" => {
            if !walker.knows_item(name) {
                return Err(format!("unknown item '{name}'"));
            }
            let state = walker.player_state_mut();
            if state.hands.is_none() {
                state.hands = Some(instance());
                Ok(vec![format!("gave item {name}")])
            } else {
                walker.spawn_item(instance(), at_feet);
                Ok(vec![format!("hands full: dropped {name} at your feet")])
            }
        }
        _ => Err(format!("unknown kind '{kind}' (tool or item)")),
    }
}

/// Most entities one `spawn` may add.
pub const SPAWN_MAX: i64 = 64;
/// How far ahead of the player the scatter is centred, and its radius.
const SPAWN_AHEAD: f64 = 2.0;
const SPAWN_RADIUS: f64 = 2.0;

/// `spawn <name> [count]`: loose items or animals, scattered ahead of the
/// player with the sim RNG (docs/design/debug-console.md section 3).
pub fn spawn_command() -> Command<Walker> {
    Command {
        name: "spawn",
        aliases: &[],
        args: &[
            ArgSpec {
                name: "name",
                kind: ArgKind::Word,
                optional: false,
            },
            ArgSpec {
                name: "count",
                kind: ArgKind::Int,
                optional: true,
            },
        ],
        help: "spawn loose items or animals ahead of you (count 1 to 64)",
        states: &[GameState::Walking],
        cheat: true,
        handler: spawn,
    }
}

fn spawn(ctx: &mut Ctx<'_, Walker>, args: &[Arg]) -> Result<Vec<String>, String> {
    let (Some(Arg::Word(name)), count) = (args.first(), args.get(1)) else {
        return Err("missing name".into());
    };
    let count = match count {
        Some(Arg::Int(n)) => *n,
        _ => 1,
    };
    if !(1..=SPAWN_MAX).contains(&count) {
        return Err(format!("count must be 1 to {SPAWN_MAX}, got {count}"));
    }
    let walker = &mut *ctx.world;
    let is_item = walker.knows_item(name);
    let feet = walker.player().pos;
    let yaw = walker.look_yaw() as f64;
    let centre = [
        feet[0] + SPAWN_AHEAD * yaw.sin(),
        feet[2] - SPAWN_AHEAD * yaw.cos(),
    ];
    for _ in 0..count {
        let rng = walker.spawn_rng_mut();
        let angle = rng.angle() as f64;
        let r = SPAWN_RADIUS * (rng.next_u32() as f64 / (u32::MAX as f64 + 1.0)).sqrt();
        let facing = rng.angle() as f64;
        let pos = [
            centre[0] + r * angle.cos(),
            feet[1] + 0.5,
            centre[1] + r * angle.sin(),
        ];
        if is_item {
            walker.spawn_item(ItemInstance { item: name.clone() }, pos);
        } else if walker.spawn_animal(name, pos, facing).is_none() {
            // Only the first iteration can fail, before anything is added.
            return Err(format!("unknown item or animal '{name}'"));
        }
    }
    Ok(vec![format!("spawned {count} {name}")])
}

/// `clear belt|hands|items|animals`: each target removes what it names and
/// nothing else (docs/design/debug-console.md section 3).
pub fn clear_command() -> Command<Walker> {
    Command {
        name: "clear",
        aliases: &[],
        args: &[ArgSpec {
            name: "target",
            kind: ArgKind::Word,
            optional: false,
        }],
        help: "clear belt (restore v0 belt), hands, items (loose) or animals",
        states: &[GameState::Walking],
        cheat: true,
        handler: clear,
    }
}

fn clear(ctx: &mut Ctx<'_, Walker>, args: &[Arg]) -> Result<Vec<String>, String> {
    let Some(Arg::Word(target)) = args.first() else {
        return Err("missing target".into());
    };
    let walker = &mut *ctx.world;
    match target.as_str() {
        "belt" => walker.reset_belt().map_err(|e| e.to_string())?,
        "hands" => walker.clear_hands(),
        "items" => walker.clear_items(),
        "animals" => walker.clear_animals(),
        _ => {
            return Err(format!(
                "unknown target '{target}' (belt, hands, items, animals)"
            ))
        }
    }
    Ok(vec![format!("cleared {target}")])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct World {
        count: i64,
    }

    fn add(ctx: &mut Ctx<'_, World>, args: &[Arg]) -> Result<Vec<String>, String> {
        let Some(Arg::Int(n)) = args.first() else {
            return Err("missing".into());
        };
        ctx.world.count += n;
        Ok(vec![format!("count is {}", ctx.world.count)])
    }

    fn cmd(name: &'static str, aliases: &'static [&'static str]) -> Command<World> {
        Command {
            name,
            aliases,
            args: &[ArgSpec {
                name: "n",
                kind: ArgKind::Int,
                optional: false,
            }],
            help: "add n to the count",
            states: ALL_STATES,
            cheat: false,
            handler: add,
        }
    }

    fn registry() -> Registry<World> {
        let mut cheat = cmd("cheat", &["ch"]);
        cheat.cheat = true;
        Registry::new(vec![cmd("add", &["plus"]), cheat]).unwrap()
    }

    #[test]
    fn registry_rejects_duplicates_and_shadowing() {
        let dup = Registry::new(vec![cmd("a", &[]), cmd("a", &[])]);
        assert_eq!(
            dup.err(),
            Some(RegistryError::DuplicateName { name: "a".into() })
        );
        let shadow = Registry::new(vec![cmd("a", &[]), cmd("b", &["a"])]);
        assert!(matches!(
            shadow.err(),
            Some(RegistryError::AliasShadows { .. })
        ));
        let shadow_alias = Registry::new(vec![cmd("a", &["x"]), cmd("b", &["x"])]);
        assert!(shadow_alias.is_err());
    }

    #[test]
    fn lookup_by_name_and_alias() {
        let c = registry();
        assert_eq!(c.get("add").unwrap().name, "add");
        assert_eq!(c.get("plus").unwrap().name, "add");
        assert!(c.get("nothing").is_none());
    }

    #[test]
    fn parser_splits_and_types_arguments() {
        let c = registry();
        let (cmd, args) = parse(&c, "  plus   -3 ").unwrap().unwrap();
        assert_eq!(cmd.name, "add");
        assert_eq!(args, vec![Arg::Int(-3)]);
        assert!(parse(&c, "   ").unwrap().is_none());
    }

    #[test]
    fn parser_errors_are_readable() {
        let c = registry();
        let e = |l| parse(&c, l).err().unwrap().to_string();
        assert_eq!(e("fly"), "unknown command 'fly' (try 'help')");
        assert_eq!(e("add"), "wrong number of arguments (0); usage: add <n>");
        assert_eq!(
            e("add 1 2"),
            "wrong number of arguments (2); usage: add <n>"
        );
        assert_eq!(e("add x"), "bad value 'x' for <n>; usage: add <n>");
    }

    fn console() -> Console<World> {
        let mut flying = cmd("fly", &[]);
        flying.states = &[GameState::Flying];
        let mut bad = cmd("bad", &[]);
        bad.cheat = true;
        bad.handler = |_, _| Err("nope".into());
        let mut reg = registry();
        reg.commands.push(flying);
        reg.commands.push(bad);
        Console::new(reg).unwrap()
    }

    #[test]
    fn help_lists_and_describes_without_cheating() {
        let (mut c, mut w) = (console(), World::default());
        c.run(&mut w, GameState::Walking, "help");
        let out = c.drain();
        assert!(out.len() > 3);
        assert!(out.contains(&Outcome::Result("add <n>: add n to the count".into())));
        c.run(&mut w, GameState::Walking, "help ch");
        let out = c.drain();
        assert_eq!(out[0], Outcome::Result("usage: cheat <n>".into()));
        assert!(out.contains(&Outcome::Result("aliases: ch".into())));
        assert!(out
            .iter()
            .any(|o| matches!(o, Outcome::Result(l) if l.starts_with("cheat:"))));
        c.run(&mut w, GameState::Walking, "help nothing");
        assert!(matches!(c.drain()[0], Outcome::Error(_)));
        assert!(!c.cheated());
    }

    #[test]
    fn help_rejects_extra_args_and_describes_itself() {
        let (mut c, mut w) = (console(), World::default());
        c.run(&mut w, GameState::Walking, "help add cheat");
        assert_eq!(
            c.drain(),
            vec![Outcome::Error(
                "wrong number of arguments (2); usage: help [command]".into()
            )]
        );
        c.run(&mut w, GameState::Walking, "help");
        let listing = c.drain();
        c.run(&mut w, GameState::Walking, "help help");
        let own = c.drain();
        assert_eq!(own[0], Outcome::Result("usage: help [command]".into()));
        assert_eq!(
            own[1],
            Outcome::Result("show this list or one command".into())
        );
        assert_eq!(
            listing[0],
            Outcome::Result("help [command]: show this list or one command".into())
        );
        assert!(!c.cheated());
    }

    #[test]
    fn cheat_refused_for_state_leaves_cheated_false() {
        let mut w = World::default();
        let mut fly_cheat = cmd("fc", &[]);
        fly_cheat.states = &[GameState::Flying];
        fly_cheat.cheat = true;
        let mut reg = registry();
        reg.commands.push(fly_cheat);
        let mut c2 = Console::new(reg).unwrap();
        c2.run(&mut w, GameState::Walking, "fc 1");
        assert!(matches!(c2.drain()[0], Outcome::Error(_)));
        assert!(!c2.cheated());
        assert_eq!(w.count, 0);
        c2.run(&mut w, GameState::Flying, "fc 1");
        assert!(c2.cheated());
    }

    #[test]
    fn user_help_command_is_rejected() {
        let reg = Registry::new(vec![cmd("help", &[])]).unwrap();
        assert!(Console::new(reg).is_err());
        let reg = Registry::new(vec![cmd("a", &["help"])]).unwrap();
        assert!(Console::new(reg).is_err());
    }

    #[test]
    fn cheated_set_on_first_successful_cheat_only() {
        let (mut c, mut w) = (console(), World::default());
        c.run(&mut w, GameState::Walking, "add 1");
        c.run(&mut w, GameState::Walking, "bad 1");
        c.run(&mut w, GameState::Walking, "cheat zzz");
        assert!(!c.cheated());
        c.run(&mut w, GameState::Walking, "cheat 2");
        assert!(c.cheated());
        assert_eq!(w.count, 3);
    }

    #[test]
    fn wrong_state_errors_and_changes_nothing() {
        let (mut c, mut w) = (console(), World::default());
        c.run(&mut w, GameState::Walking, "fly 5");
        assert_eq!(w.count, 0);
        assert_eq!(
            c.drain(),
            vec![Outcome::Error("'fly' can't be used while walking".into())]
        );
        c.run(&mut w, GameState::Flying, "fly 5");
        assert_eq!(w.count, 5);
    }

    #[test]
    fn outcome_buffer_drains() {
        let (mut c, mut w) = (console(), World::default());
        c.run(&mut w, GameState::Walking, "add 2");
        c.run(&mut w, GameState::Walking, "bad 1");
        c.run(&mut w, GameState::Walking, "");
        assert_eq!(
            c.drain(),
            vec![
                Outcome::Result("count is 2".into()),
                Outcome::Error("nope".into())
            ]
        );
        assert!(c.drain().is_empty());
    }
}

#[cfg(test)]
mod default_registry_tests {
    use super::*;

    #[test]
    fn default_registry_has_exactly_the_eight_commands() {
        let reg = default_registry().unwrap();
        let names: Vec<&str> = reg.iter().map(|c| c.name).collect();
        assert_eq!(
            names,
            vec!["help", "give", "spawn", "clear", "tp", "pause", "resume", "step"]
        );
        assert!(!reg.get("help").unwrap().cheat);
    }
}

#[cfg(test)]
mod spawn_tests {
    use super::*;
    use crate::input::TickInput;

    fn walker() -> Walker {
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        w.set_commands(Registry::new(vec![spawn_command()]).unwrap());
        w
    }

    fn run(w: &mut Walker, lines: &[&str]) -> Vec<String> {
        for l in lines {
            w.submit_command(l);
        }
        w.tick(&TickInput::default());
        w.drain_command_output()
    }

    #[test]
    fn spawn_items_and_animals_by_name() {
        let mut w = walker();
        run(&mut w, &["spawn seeds 5", "spawn cow", "spawn cow 2"]);
        assert_eq!(w.items().len(), 5);
        assert_eq!(w.animals().len(), 3);
        assert!(w.cheated());
    }

    #[test]
    fn spawns_land_within_the_scatter_ahead_of_the_player() {
        let mut w = walker();
        run(&mut w, &["spawn seeds 64"]);
        // Yaw 0 faces -z: centre is (0.5, -1.5), radius 2.
        for e in w.items().iter() {
            let (dx, dz) = (e.pos[0] - 0.5, e.pos[2] + 1.5);
            assert!((dx * dx + dz * dz).sqrt() <= 2.0 + 1e-6);
        }
    }

    #[test]
    fn same_commands_give_the_same_positions() {
        let positions = || {
            let mut w = walker();
            run(&mut w, &["spawn seeds 8", "spawn cow 4"]);
            let mut p: Vec<[f64; 3]> = w.items().iter().map(|e| e.pos).collect();
            p.extend(w.animals().iter().map(|a| a.pos));
            p
        };
        assert_eq!(positions(), positions());
    }

    #[test]
    fn bad_names_and_counts_change_nothing() {
        let mut w = walker();
        let out = run(
            &mut w,
            &[
                "spawn nothing 3",
                "spawn seeds 65",
                "spawn seeds 0",
                "spawn seeds -1",
            ],
        );
        assert_eq!(out.len(), 4);
        assert!(out[0].contains("unknown item or animal 'nothing'"));
        assert!(out[1].contains("count must be 1 to 64"));
        assert!(out[2].contains("count must be 1 to 64"));
        assert!(out[3].contains("count must be 1 to 64"));
        assert_eq!(w.items().len(), 0);
        assert_eq!(w.animals().len(), 0);
        assert!(!w.cheated());
        assert_eq!(run(&mut w, &["spawn seeds 64"]), vec!["spawned 64 seeds"]);
        assert_eq!(w.items().len(), 64);
    }
}

#[cfg(test)]
mod clear_tests {
    use super::*;
    use crate::input::TickInput;
    use crate::items::{PlayerState, ToolInstance};

    fn walker() -> Walker {
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        w.set_commands(Registry::new(vec![spawn_command(), clear_command()]).unwrap());
        w
    }

    fn run(w: &mut Walker, lines: &[&str]) -> Vec<String> {
        for l in lines {
            w.submit_command(l);
        }
        w.tick(&TickInput::default());
        w.drain_command_output()
    }

    fn v0() -> PlayerState {
        PlayerState::new(&crate::items::Items::default_items()).unwrap()
    }

    #[test]
    fn clear_belt_restores_v0_belt_and_selection() {
        let mut w = walker();
        let s = w.player_state_mut();
        s.tool_belt[0] = Some(ToolInstance { tool: "hoe".into() });
        s.tool_belt[1] = None;
        s.selected = 3;
        s.hands = Some(ItemInstance {
            item: "seeds".into(),
        });
        assert_eq!(run(&mut w, &["clear belt"]), vec!["cleared belt"]);
        let s = w.player_state();
        assert_eq!(s.tool_belt, v0().tool_belt);
        assert_eq!(s.selected, 0);
        assert!(s.hands.is_some(), "belt leaves the hands alone");
        assert!(w.cheated());
    }

    #[test]
    fn each_target_clears_only_what_it_names() {
        let mut w = walker();
        let fill = |w: &mut Walker| {
            run(w, &["spawn seeds 3", "spawn cow 2"]);
            w.player_state_mut().hands = Some(ItemInstance {
                item: "seeds".into(),
            });
        };
        fill(&mut w);
        let (items, animals) = (w.items().len(), w.animals().len());
        run(&mut w, &["clear hands"]);
        assert!(w.hands().is_none());
        assert_eq!((w.items().len(), w.animals().len()), (items, animals));
        assert_eq!(w.player_state().tool_belt, v0().tool_belt);
        run(&mut w, &["clear items"]);
        assert_eq!((w.items().len(), w.animals().len()), (0, animals));
        fill(&mut w);
        run(&mut w, &["clear animals"]);
        assert_eq!(w.animals().len(), 0);
        assert!(w.items().len() >= 3);
        assert!(w.hands().is_some());
    }

    #[test]
    fn unknown_target_errors_and_changes_nothing() {
        let mut w = walker();
        run(&mut w, &["spawn seeds 2"]);
        w.player_state_mut().selected = 2;
        let out = run(&mut w, &["clear everything"]);
        assert!(out[0].contains("unknown target 'everything'"));
        assert_eq!(w.items().len(), 2);
        assert_eq!(w.player_state().selected, 2);
        assert!(w.cheated(), "the spawn cheated, not the failed clear");
        let mut w2 = walker();
        run(&mut w2, &["clear everything"]);
        assert!(!w2.cheated());
    }
}

#[cfg(test)]
mod give_tests {
    use super::*;
    use crate::input::TickInput;

    fn walker() -> Walker {
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        w.set_commands(Registry::new(vec![give_command()]).unwrap());
        w
    }

    fn run(w: &mut Walker, lines: &[&str]) -> Vec<String> {
        for l in lines {
            w.submit_command(l);
        }
        w.tick(&TickInput::default());
        w.drain_command_output()
    }

    fn tool(name: &str) -> Option<ToolInstance> {
        Some(ToolInstance { tool: name.into() })
    }

    #[test]
    fn give_tool_uses_first_empty_slot_and_selects_it() {
        let mut w = walker();
        run(&mut w, &["give tool hoe"]);
        let s = w.player_state();
        assert_eq!(s.tool_belt[3], tool("hoe"));
        assert_eq!(s.selected, 3);
        assert_eq!(s.tool_belt[0], None, "slot 0 stays the empty hand");
        run(&mut w, &["give tool sword"]);
        assert_eq!(w.player_state().tool_belt[4], tool("sword"));
        assert_eq!(w.player_state().selected, 4);
        assert!(w.cheated());
    }

    #[test]
    fn give_item_fills_empty_hands() {
        let mut w = walker();
        assert_eq!(run(&mut w, &["give item seeds"]), vec!["gave item seeds"]);
        assert_eq!(w.hands().unwrap().item, "seeds");
        assert_eq!(w.items().len(), 0);
        assert!(w.cheated());
    }

    #[test]
    fn give_item_with_full_hands_drops_at_feet() {
        let mut w = walker();
        run(&mut w, &["give item seeds", "give item wheat"]);
        assert_eq!(w.hands().unwrap().item, "seeds");
        let dropped: Vec<_> = w.items().iter().collect();
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].item.item, "wheat");
        assert!((dropped[0].pos[0] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn give_tool_with_full_belt_drops_item_named_after_tool() {
        let mut w = walker();
        run(&mut w, &["give tool hoe", "give tool hoe"]);
        let before = w.player_state().clone();
        let out = run(&mut w, &["give tool sword"]);
        assert!(out[0].contains("belt full"));
        assert_eq!(*w.player_state(), before);
        let dropped: Vec<_> = w.items().iter().collect();
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].item.item, "sword");
    }

    #[test]
    fn unknown_names_and_kinds_error_and_change_nothing() {
        let mut w = walker();
        let before = w.player_state().clone();
        for line in [
            "give tool banana",
            "give item hoe",
            "give tool seeds",
            "give thing x",
        ] {
            let out = run(&mut w, &[line]);
            assert!(out[0].contains("unknown"), "{line}: {out:?}");
        }
        assert_eq!(*w.player_state(), before);
        assert_eq!(w.items().len(), 0);
        assert!(!w.cheated());
    }
}

#[cfg(test)]
mod tp_tests {
    use super::*;
    use crate::input::TickInput;
    use layout::Layout;

    fn walker() -> Walker {
        let layout = Layout {
            seed: 25,
            ..Layout::default()
        };
        let mut w = Walker::with_layout(layout, [64.5, 120.0, 64.5]).unwrap();
        w.set_commands(default_registry().unwrap());
        w
    }

    fn run(w: &mut Walker, line: &str) -> Vec<String> {
        w.submit_command(line);
        w.tick(&TickInput::default());
        w.drain_command_output()
    }

    #[test]
    fn tp_stands_on_the_surface_with_no_velocity_and_the_eye_follows() {
        let mut w = walker();
        // Several regions away, so the destination must be streamed in.
        let out = run(&mut w, "tp 700 -300");
        assert!(out[0].starts_with("teleported to 700 "), "{out:?}");
        assert!(w.cheated());
        let p = w.player();
        assert_eq!((p.pos[0], p.pos[2]), (700.5, -299.5));
        let top = (0..terrain::HEIGHT as i32)
            .rev()
            .find(|&y| w.block_at(700, y, -300) != terrain::Block::Air)
            .unwrap();
        // One tick of gravity at most separates it from the surface.
        assert!((p.pos[1] - (top as f64 + 1.0)).abs() < 0.5, "{p:?} {top}");
        assert_eq!(p.vel[0], 0.0);
        assert_eq!(p.vel[2], 0.0);
        let eye = w.eye(1.0);
        assert!(
            (eye[0] - 700.5).abs() < 1e-9 && eye[1] > p.pos[1],
            "{eye:?}"
        );
    }

    #[test]
    fn bad_arguments_error_and_change_nothing() {
        let mut w = walker();
        let before = w.player().pos;
        for line in [
            "tp",
            "tp 5",
            "tp 1 2 3",
            "tp a b",
            "tp 1.5 2",
            "tp 99999999999 0",
        ] {
            let out = run(&mut w, line);
            assert!(
                out[0].contains("usage")
                    || out[0].contains("bad value")
                    || out[0].contains("range"),
                "{line}: {out:?}"
            );
            assert!(!w.cheated(), "{line}");
        }
        assert_eq!(w.player().pos[0], before[0]);
        assert_eq!(w.player().pos[2], before[2]);
    }

    #[test]
    fn tp_outside_any_loaded_world_fails_without_cheating() {
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        w.set_commands(default_registry().unwrap());
        run(&mut w, "tp 1000 1000");
        assert!(!w.cheated());
    }
}

#[cfg(test)]
mod pause_tests {
    use super::*;
    use crate::input::TickInput;

    fn walker() -> Walker {
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        w.set_commands(default_registry().unwrap());
        w
    }

    fn run(w: &mut Walker, lines: &[&str]) -> Vec<String> {
        for l in lines {
            w.submit_command(l);
        }
        w.run_commands();
        w.drain_command_output()
    }

    #[test]
    fn pause_resume_are_idempotent_and_not_cheats() {
        let mut w = walker();
        assert_eq!(run(&mut w, &["resume"]), vec!["not paused"]);
        assert_eq!(
            run(&mut w, &["pause", "pause"]),
            vec!["paused", "already paused"]
        );
        assert!(w.paused());
        assert_eq!(run(&mut w, &["resume"]), vec!["resumed"]);
        assert!(!w.paused());
        assert!(!w.cheated());
    }

    #[test]
    fn step_requests_ticks_only_while_paused() {
        let mut w = walker();
        assert_eq!(run(&mut w, &["step"]), vec!["step needs the game paused"]);
        assert_eq!(w.take_steps(), 0);
        run(&mut w, &["pause", "step 3", "step"]);
        assert_eq!(w.take_steps(), 4);
        assert_eq!(w.take_steps(), 0);
        assert!(w.paused(), "still paused");
        assert!(!w.cheated());
    }

    #[test]
    fn bad_step_counts_change_nothing() {
        let mut w = walker();
        run(&mut w, &["pause"]);
        for l in ["step 0", "step -2", "step 1001", "step x"] {
            let out = run(&mut w, &[l]);
            assert_eq!(out.len(), 1, "{l}");
            assert!(!out[0].starts_with("stepping"), "{l}: {out:?}");
        }
        assert_eq!(w.take_steps(), 0);
        run(&mut w, &["step 1000"]);
        assert_eq!(w.take_steps(), 1000);
    }

    #[test]
    fn resume_before_step_in_one_batch_leaves_step_an_error() {
        let mut w = walker();
        run(&mut w, &["pause"]);
        let out = run(&mut w, &["resume", "step 2"]);
        assert_eq!(out[0], "resumed");
        assert_eq!(out[1], "step needs the game paused");
        assert_eq!(w.take_steps(), 0);
    }

    #[test]
    fn commands_alone_do_not_tick() {
        let mut w = walker();
        run(&mut w, &["pause"]);
        assert_eq!(w.sim_tick(), 0);
        run(&mut w, &["step 2"]);
        assert_eq!(w.sim_tick(), 0, "commands never advance the sim");
        w.tick(&TickInput::default());
        assert_eq!(w.sim_tick(), 1, "tick itself never checks the flag");
    }
}
