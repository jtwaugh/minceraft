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
        self.commands
            .iter()
            .find(|c| c.name == name || c.aliases.contains(&name))
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
}
