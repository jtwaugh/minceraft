//! Debug console command core: registry, parser and outcome buffer.
//! See docs/design/debug-console.md sections 1 and 10 (Phase 1).
//!
//! A command is one [`Command`] entry plus its handler. The handler gets the
//! world it acts on (`W`, which will be the walker once commands are wired
//! in) and the registry, and returns its result lines. [`Console`] parses a
//! line, checks the game state, runs the handler and buffers the outcome
//! lines until the front end drains them.

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
    Unknown {
        name: String,
    },
    WrongArgCount {
        usage: String,
        got: usize,
    },
    BadArg {
        usage: String,
        arg: &'static str,
        value: String,
    },
    WrongState {
        command: &'static str,
        state: GameState,
    },
    /// The handler refused; the message is its own.
    Failed {
        command: &'static str,
        message: String,
    },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::Unknown { name } => {
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
            CommandError::Failed { command, message } => write!(f, "{command}: {message}"),
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

    /// Lookup by name or alias.
    pub fn get(&self, name: &str) -> Option<&Command<W>> {
        self.commands
            .iter()
            .find(|c| c.name == name || c.aliases.contains(&name))
    }

    pub fn commands(&self) -> &[Command<W>] {
        &self.commands
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
        .ok_or_else(|| CommandError::Unknown { name: name.into() })?;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Result(String),
    Error(String),
}

/// The registry plus the outcome buffer and the `cheated` flag.
pub struct Console<W> {
    registry: Registry<W>,
    outcomes: Vec<Outcome>,
    cheated: bool,
}

impl<W> Console<W> {
    /// Adds the built-in `help` to `commands`.
    pub fn new(mut commands: Vec<Command<W>>) -> Result<Self, RegistryError> {
        commands.push(help_command());
        Ok(Console {
            registry: Registry::new(commands)?,
            outcomes: Vec::new(),
            cheated: false,
        })
    }

    pub fn registry(&self) -> &Registry<W> {
        &self.registry
    }

    /// True once any cheat command has run successfully.
    pub fn cheated(&self) -> bool {
        self.cheated
    }

    /// Runs one line. Failures go to the buffer as error lines and change
    /// nothing; `cheated` is set only when a cheat handler succeeds.
    pub fn run(&mut self, line: &str, state: GameState, world: &mut W) {
        match self.try_run(line, state, world) {
            Ok(lines) => self.outcomes.extend(lines.into_iter().map(Outcome::Result)),
            Err(e) => self.outcomes.push(Outcome::Error(e.to_string())),
        }
    }

    fn try_run(
        &mut self,
        line: &str,
        state: GameState,
        world: &mut W,
    ) -> Result<Vec<String>, CommandError> {
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
        let lines = (cmd.handler)(&mut ctx, &args).map_err(|message| CommandError::Failed {
            command: cmd.name,
            message,
        })?;
        self.cheated |= cmd.cheat;
        Ok(lines)
    }

    /// Takes the buffered outcome lines, oldest first.
    pub fn drain(&mut self) -> Vec<Outcome> {
        std::mem::take(&mut self.outcomes)
    }
}

fn help_command<W>() -> Command<W> {
    Command {
        name: "help",
        aliases: &[],
        args: &[ArgSpec {
            name: "command",
            kind: ArgKind::Word,
            optional: true,
        }],
        help: "list commands, or show one command's usage",
        states: ALL_STATES,
        cheat: false,
        handler: help_handler::<W>,
    }
}

fn help_handler<W>(ctx: &mut Ctx<'_, W>, args: &[Arg]) -> Result<Vec<String>, String> {
    match args.first() {
        None => Ok(ctx
            .registry
            .commands()
            .iter()
            .map(|c| format!("{} - {}", c.usage(), c.help))
            .collect()),
        Some(Arg::Word(name)) => {
            let c = ctx
                .registry
                .get(name)
                .ok_or_else(|| format!("unknown command '{name}'"))?;
            let mut lines = vec![format!("usage: {}", c.usage()), c.help.to_string()];
            if !c.aliases.is_empty() {
                lines.push(format!("aliases: {}", c.aliases.join(", ")));
            }
            if c.cheat {
                lines.push("cheat: sets the cheated flag".to_string());
            }
            Ok(lines)
        }
        Some(_) => Err("expected a command name".to_string()),
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

    fn fail(_: &mut Ctx<'_, World>, _: &[Arg]) -> Result<Vec<String>, String> {
        Err("nope".into())
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

    fn console() -> Console<World> {
        let mut cheat = cmd("cheat", &["ch"]);
        cheat.cheat = true;
        let mut walk_only = cmd("walkonly", &[]);
        walk_only.states = &[GameState::Walking];
        let mut failing = cmd("failing", &[]);
        failing.cheat = true;
        failing.handler = fail;
        Console::new(vec![cmd("add", &["plus"]), cheat, walk_only, failing]).unwrap()
    }

    fn errors(c: &mut Console<World>) -> Vec<String> {
        c.drain()
            .into_iter()
            .map(|o| match o {
                Outcome::Error(e) => e,
                Outcome::Result(r) => panic!("unexpected result {r}"),
            })
            .collect()
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
        // A user command can't replace the built-in help.
        assert!(Console::new(vec![cmd("help", &[])]).is_err());
    }

    #[test]
    fn lookup_by_name_and_alias() {
        let c = console();
        assert_eq!(c.registry().get("add").unwrap().name, "add");
        assert_eq!(c.registry().get("plus").unwrap().name, "add");
        assert!(c.registry().get("nothing").is_none());
    }

    #[test]
    fn parser_splits_and_types_arguments() {
        let c = console();
        let (cmd, args) = parse(c.registry(), "  plus   -3 ").unwrap().unwrap();
        assert_eq!(cmd.name, "add");
        assert_eq!(args, vec![Arg::Int(-3)]);
        assert!(parse(c.registry(), "   ").unwrap().is_none());
    }

    #[test]
    fn parser_errors_are_readable() {
        let c = console();
        let e = |l| parse(c.registry(), l).err().unwrap().to_string();
        assert_eq!(e("fly"), "unknown command 'fly' (try 'help')");
        assert_eq!(e("add"), "wrong number of arguments (0); usage: add <n>");
        assert_eq!(
            e("add 1 2"),
            "wrong number of arguments (2); usage: add <n>"
        );
        assert_eq!(e("add x"), "bad value 'x' for <n>; usage: add <n>");
    }

    #[test]
    fn outcomes_buffer_and_drain() {
        let mut c = console();
        let mut w = World::default();
        c.run("add 2", GameState::Walking, &mut w);
        c.run("add 3", GameState::Walking, &mut w);
        c.run("bogus", GameState::Walking, &mut w);
        assert_eq!(
            c.drain(),
            vec![
                Outcome::Result("count is 2".into()),
                Outcome::Result("count is 5".into()),
                Outcome::Error("unknown command 'bogus' (try 'help')".into()),
            ]
        );
        assert!(c.drain().is_empty());
        c.run("", GameState::Walking, &mut w);
        assert!(c.drain().is_empty());
    }

    #[test]
    fn help_lists_and_describes_without_cheating() {
        let mut c = console();
        let mut w = World::default();
        c.run("help", GameState::Walking, &mut w);
        let all = c.drain();
        assert_eq!(all.len(), 5);
        assert!(all.contains(&Outcome::Result(
            "help [command] - list commands, or show one command's usage".into()
        )));
        c.run("help ch", GameState::Walking, &mut w);
        assert_eq!(
            c.drain(),
            vec![
                Outcome::Result("usage: cheat <n>".into()),
                Outcome::Result("add n to the count".into()),
                Outcome::Result("aliases: ch".into()),
                Outcome::Result("cheat: sets the cheated flag".into()),
            ]
        );
        c.run("help nope", GameState::Walking, &mut w);
        assert_eq!(errors(&mut c), vec!["help: unknown command 'nope'"]);
        assert!(!c.cheated());
    }

    #[test]
    fn cheated_is_set_on_first_cheat_only() {
        let mut c = console();
        let mut w = World::default();
        c.run("add 1", GameState::Walking, &mut w);
        assert!(!c.cheated());
        c.drain();
        c.run("failing 1", GameState::Walking, &mut w);
        assert!(!c.cheated(), "a refused cheat did not run");
        assert_eq!(errors(&mut c), vec!["failing: nope"]);
        c.run("ch 1", GameState::Walking, &mut w);
        assert!(c.cheated());
        c.run("add 1", GameState::Walking, &mut w);
        assert!(c.cheated(), "stays set");
        assert_eq!(w.count, 3);
    }

    #[test]
    fn disallowed_state_errors_and_changes_nothing() {
        let mut c = console();
        let mut w = World::default();
        c.run("walkonly 5", GameState::Flying, &mut w);
        assert_eq!(w.count, 0);
        assert_eq!(
            errors(&mut c),
            vec!["'walkonly' can't be used while flying"]
        );
        c.run("walkonly 5", GameState::Walking, &mut w);
        assert_eq!(w.count, 5);
    }

    #[test]
    fn disallowed_cheat_does_not_set_cheated() {
        let mut cheat = cmd("cheat", &[]);
        cheat.cheat = true;
        cheat.states = &[GameState::Walking];
        let mut c = Console::new(vec![cheat]).unwrap();
        let mut w = World::default();
        c.run("cheat 1", GameState::Flying, &mut w);
        assert!(!c.cheated());
        assert_eq!(w.count, 0);
    }
}
