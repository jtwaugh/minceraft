//! Debug console command core: registry and parser (docs/design/debug-console.md
//! Phase 1). A command is one [`Command`] entry plus a handler that gets the
//! world `W` it acts on and returns its result lines.

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
/// line and changes nothing. `cheated` is set when a cheat command runs.
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
    if cmd.cheat {
        *cheated = true;
    }
    let mut ctx = Ctx { world, registry };
    match (cmd.handler)(&mut ctx, &args) {
        Ok(lines) => lines,
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
