//! Recording and replaying a walking session, and a hash of the world state
//! to compare runs by. See docs/design/debug-console.md section 7.
//!
//! A recording is one record per tick: the `TickInput` and the command lines
//! submitted just before it. Its text form is one line per record,
//!
//! ```text
//! tick 1203: input {fwd=3f800000 str=0 yaw=0 pitch=0 flags=0 slot=-}; commands ["give hoe"]
//! ```
//!
//! after a leading `cheated true|false` line. Floats are written as the hex
//! of their bits so they round-trip exactly.
//!
//! The world hash (FNV-1a, 64 bit) covers `sim_tick`, the `cheated` flag, the
//! player's position, velocity, ground flag and health, the tool belt,
//! selection and hands, every item entity (id, item, position, velocity) and
//! every animal (id, species, health, position, velocity, facing). It does
//! not cover terrain, crops or the RNG streams.

use crate::input::TickInput;
use crate::walk::Walker;

/// What happened at one tick.
#[derive(Clone, Debug, PartialEq)]
pub struct TickRecord {
    /// The tick this record runs (`Walker::sim_tick() + 1` when it started).
    pub tick: u64,
    pub input: TickInput,
    /// Command lines submitted before the tick, in order.
    pub commands: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recording {
    pub ticks: Vec<TickRecord>,
    /// Whether a cheat had run by the end of the session.
    pub cheated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// A line of text that isn't a valid recording.
    Parse(String),
    /// The walker is not at the tick the next record expects.
    TickMismatch { expected: u64, found: u64 },
    /// The replay ended with a different `cheated` flag.
    CheatedMismatch { recorded: bool, replayed: bool },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Parse(m) => write!(f, "bad recording: {m}"),
            ReplayError::TickMismatch { expected, found } => {
                write!(f, "record is for tick {expected}, walker is at {found}")
            }
            ReplayError::CheatedMismatch { recorded, replayed } => {
                write!(f, "cheated: recorded {recorded}, replayed {replayed}")
            }
        }
    }
}

impl std::error::Error for ReplayError {}

fn bad<T>(msg: impl Into<String>) -> Result<T, ReplayError> {
    Err(ReplayError::Parse(msg.into()))
}

const FLAGS: usize = 7;

fn flags_of(i: &TickInput) -> u32 {
    [
        i.jump,
        i.sneak,
        i.primary,
        i.secondary,
        i.secondary_held,
        i.interact,
        i.drop,
    ]
    .iter()
    .enumerate()
    .fold(0, |acc, (n, &b)| acc | (u32::from(b) << n))
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Parse `["a", "b"]` as written by `quote`.
fn parse_commands(s: &str) -> Result<Vec<String>, ReplayError> {
    let inner = s
        .trim()
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .map_or_else(|| bad("commands must be a [..] list"), Ok)?;
    let mut out = Vec::new();
    let mut chars = inner.chars().peekable();
    loop {
        while matches!(chars.peek(), Some(' ') | Some(',')) {
            chars.next();
        }
        match chars.next() {
            None => return Ok(out),
            Some('"') => {}
            Some(c) => return bad(format!("unexpected {c:?} in commands")),
        }
        let mut cur = String::new();
        loop {
            match chars.next() {
                None => return bad("unterminated command string"),
                Some('"') => break,
                Some('\\') => match chars.next() {
                    Some('n') => cur.push('\n'),
                    Some('r') => cur.push('\r'),
                    Some(c @ ('"' | '\\')) => cur.push(c),
                    _ => return bad("bad escape in command"),
                },
                Some(c) => cur.push(c),
            }
        }
        out.push(cur);
    }
}

fn hex32(s: &str) -> Result<f32, ReplayError> {
    u32::from_str_radix(s, 16)
        .map(f32::from_bits)
        .or_else(|_| bad(format!("bad float bits {s:?}")))
}

fn parse_input(body: &str) -> Result<TickInput, ReplayError> {
    let mut input = TickInput::default();
    let mut seen = 0;
    for field in body.split_whitespace() {
        let Some((k, v)) = field.split_once('=') else {
            return bad(format!("bad input field {field:?}"));
        };
        match k {
            "fwd" => input.move_forward = hex32(v)?,
            "str" => input.move_strafe = hex32(v)?,
            "yaw" => input.yaw = hex32(v)?,
            "pitch" => input.pitch = hex32(v)?,
            "flags" => {
                let Ok(f) = v.parse::<u32>() else {
                    return bad(format!("bad flags {v:?}"));
                };
                if f >> FLAGS != 0 {
                    return bad(format!("unknown flag bits in {v:?}"));
                }
                let bit = |n: usize| f >> n & 1 == 1;
                input.jump = bit(0);
                input.sneak = bit(1);
                input.primary = bit(2);
                input.secondary = bit(3);
                input.secondary_held = bit(4);
                input.interact = bit(5);
                input.drop = bit(6);
            }
            "slot" => {
                input.tool_slot = if v == "-" {
                    None
                } else {
                    match v.parse::<u8>() {
                        Ok(n) => Some(n),
                        Err(_) => return bad(format!("bad slot {v:?}")),
                    }
                }
            }
            _ => return bad(format!("unknown input field {k:?}")),
        }
        seen += 1;
    }
    if seen != 6 {
        return bad("input needs fwd, str, yaw, pitch, flags and slot");
    }
    Ok(input)
}

impl Recording {
    pub fn to_text(&self) -> String {
        let mut out = format!("cheated {}\n", self.cheated);
        for r in &self.ticks {
            let i = &r.input;
            let slot = i.tool_slot.map_or("-".to_string(), |n| n.to_string());
            let cmds: Vec<String> = r.commands.iter().map(|c| quote(c)).collect();
            out.push_str(&format!(
                "tick {}: input {{fwd={:x} str={:x} yaw={:x} pitch={:x} flags={} slot={}}}; commands [{}]\n",
                r.tick,
                i.move_forward.to_bits(),
                i.move_strafe.to_bits(),
                i.yaw.to_bits(),
                i.pitch.to_bits(),
                flags_of(i),
                slot,
                cmds.join(", ")
            ));
        }
        out
    }

    pub fn parse(text: &str) -> Result<Recording, ReplayError> {
        let mut lines = text.lines();
        let cheated = match lines.next() {
            Some("cheated true") => true,
            Some("cheated false") => false,
            _ => return bad("first line must be `cheated true|false`"),
        };
        let mut ticks = Vec::new();
        for line in lines.filter(|l| !l.trim().is_empty()) {
            let Some(rest) = line.strip_prefix("tick ") else {
                return bad(format!("expected `tick N:`, got {line:?}"));
            };
            let Some((n, rest)) = rest.split_once(": input {") else {
                return bad(format!("expected `: input {{`, got {line:?}"));
            };
            let Some((body, rest)) = rest.split_once("}; commands ") else {
                return bad(format!("expected `}}; commands`, got {line:?}"));
            };
            let Ok(tick) = n.parse::<u64>() else {
                return bad(format!("bad tick number {n:?}"));
            };
            ticks.push(TickRecord {
                tick,
                input: parse_input(body)?,
                commands: parse_commands(rest)?,
            });
        }
        Ok(Recording { ticks, cheated })
    }
}

/// Drives a walker and records what it was given.
#[derive(Debug, Default)]
pub struct Recorder {
    ticks: Vec<TickRecord>,
}

impl Recorder {
    pub fn new() -> Recorder {
        Recorder::default()
    }

    /// Submit `commands`, then tick `walker` with `input`, recording both.
    pub fn step(&mut self, walker: &mut Walker, input: &TickInput, commands: &[&str]) {
        let tick = walker.sim_tick() + 1;
        for c in commands {
            walker.submit_command(c);
        }
        walker.tick(input);
        self.ticks.push(TickRecord {
            tick,
            input: *input,
            commands: commands.iter().map(|c| c.to_string()).collect(),
        });
    }

    /// The recording so far, with `walker`'s final `cheated` flag.
    pub fn finish(self, walker: &Walker) -> Recording {
        Recording {
            ticks: self.ticks,
            cheated: walker.cheated(),
        }
    }
}

/// Feed `walker` the recorded inputs and commands at the recorded ticks, then
/// check its `cheated` flag matches. `walker` must be a fresh one set up as
/// the recorded session's was (same regions and command registry).
pub fn replay(recording: &Recording, walker: &mut Walker) -> Result<(), ReplayError> {
    for r in &recording.ticks {
        let expected = walker.sim_tick() + 1;
        if r.tick != expected {
            return Err(ReplayError::TickMismatch {
                expected,
                found: r.tick,
            });
        }
        for c in &r.commands {
            walker.submit_command(c);
        }
        walker.tick(&r.input);
    }
    if walker.cheated() != recording.cheated {
        return Err(ReplayError::CheatedMismatch {
            recorded: recording.cheated,
            replayed: walker.cheated(),
        });
    }
    Ok(())
}

struct Fnv(u64);

impl Fnv {
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ u64::from(x)).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    fn f64s(&mut self, v: &[f64]) {
        for x in v {
            self.u64(x.to_bits());
        }
    }
    fn str(&mut self, s: &str) {
        self.u64(s.len() as u64);
        self.bytes(s.as_bytes());
    }
}

impl Walker {
    /// A hash of the state a replay must reproduce; see the module docs for
    /// what it covers.
    pub fn world_hash(&self) -> u64 {
        let mut h = Fnv(0xcbf2_9ce4_8422_2325);
        h.u64(self.sim_tick());
        h.u64(u64::from(self.cheated()));
        let p = self.player();
        h.f64s(&p.pos);
        h.f64s(&p.vel);
        h.u64(u64::from(p.on_ground));
        h.u64(u64::from(p.health));
        let s = self.player_state();
        for slot in &s.tool_belt {
            match slot {
                Some(t) => {
                    h.u64(1);
                    h.str(&t.tool);
                }
                None => h.u64(0),
            }
        }
        h.u64(u64::from(s.selected));
        match &s.hands {
            Some(i) => {
                h.u64(1);
                h.str(&i.item);
            }
            None => h.u64(0),
        }
        h.u64(self.items().len() as u64);
        for e in self.items().iter() {
            h.u64(e.id as u64);
            h.str(&e.item.item);
            h.f64s(&e.pos);
            h.f64s(&e.velocity);
        }
        h.u64(self.animals().len() as u64);
        for a in self.animals().iter() {
            h.u64(a.id as u64);
            h.str(&a.species);
            h.u64(u64::from(a.health));
            h.f64s(&a.pos);
            h.f64s(&a.velocity);
            h.f64s(&[a.facing]);
        }
        h.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::default_registry;
    use crate::items::ItemInstance;

    fn walker() -> Walker {
        let mut w = Walker::new([0.5, 5.0, 0.5]).unwrap();
        w.set_commands(default_registry().unwrap());
        w
    }

    fn input(fwd: f32, yaw: f32) -> TickInput {
        TickInput {
            move_forward: fwd,
            yaw,
            jump: true,
            tool_slot: Some(1),
            ..TickInput::default()
        }
    }

    fn session() -> (Recording, u64) {
        let mut w = walker();
        let mut rec = Recorder::new();
        rec.step(&mut w, &input(1.0, 0.3), &[]);
        rec.step(&mut w, &input(0.5, -0.1), &["give hoe", "say \"hi\" \\ x"]);
        rec.step(&mut w, &TickInput::default(), &["spawn seeds 3"]);
        for _ in 0..5 {
            rec.step(&mut w, &TickInput::default(), &[]);
        }
        let hash = w.world_hash();
        (rec.finish(&w), hash)
    }

    #[test]
    fn recording_round_trips() {
        let (rec, _) = session();
        assert!(rec.cheated);
        let text = rec.to_text();
        assert_eq!(Recording::parse(&text).unwrap(), rec);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(Recording::parse("").is_err());
        assert!(Recording::parse("cheated maybe\n").is_err());
        assert!(Recording::parse("cheated false\ntick x: input {}; commands []\n").is_err());
        assert!(Recording::parse("cheated false\ntick 1: input {fwd=0}; commands []\n").is_err());
    }

    #[test]
    fn replay_runs_commands_at_the_same_ticks_and_matches_the_hash() {
        let (rec, hash) = session();
        let mut w = walker();
        replay(&rec, &mut w).unwrap();
        assert_eq!(w.world_hash(), hash);
        assert_eq!(w.sim_tick(), 8);
        assert!(w.cheated());

        // Through text too, and dropping the commands changes the outcome.
        let mut w2 = walker();
        replay(&Recording::parse(&rec.to_text()).unwrap(), &mut w2).unwrap();
        assert_eq!(w2.world_hash(), hash);
        let mut stripped = rec.clone();
        for t in &mut stripped.ticks {
            t.commands.clear();
        }
        stripped.cheated = false;
        let mut w3 = walker();
        replay(&stripped, &mut w3).unwrap();
        assert_ne!(w3.world_hash(), hash);
    }

    #[test]
    fn replay_checks_cheated_and_tick_order() {
        let (mut rec, _) = session();
        rec.cheated = false;
        let mut w = walker();
        assert_eq!(
            replay(&rec, &mut w),
            Err(ReplayError::CheatedMismatch {
                recorded: false,
                replayed: true
            })
        );
        let (mut rec, _) = session();
        rec.ticks[0].tick = 4;
        assert!(matches!(
            replay(&rec, &mut walker()),
            Err(ReplayError::TickMismatch { .. })
        ));
    }

    #[test]
    fn hash_is_equal_for_equal_states_and_differs_otherwise() {
        assert_eq!(walker().world_hash(), walker().world_hash());
        let base = walker().world_hash();

        let mut w = walker();
        w.teleport([3.5, 5.0, 0.5]);
        assert_ne!(w.world_hash(), base);

        let mut w = walker();
        w.player_state_mut().selected = 2;
        assert_ne!(w.world_hash(), base);
        let mut w = walker();
        w.player_state_mut().tool_belt[1] = None;
        assert_ne!(w.world_hash(), base);

        let mut w = walker();
        w.player_state_mut().hands = Some(ItemInstance {
            item: "seeds".into(),
        });
        assert_ne!(w.world_hash(), base);

        let mut w = walker();
        w.spawn_item(
            ItemInstance {
                item: "seeds".into(),
            },
            [1.0, 5.0, 1.0],
        );
        let with_item = w.world_hash();
        assert_ne!(with_item, base);
        let mut w = walker();
        w.spawn_item(
            ItemInstance {
                item: "seeds".into(),
            },
            [2.0, 5.0, 1.0],
        );
        assert_ne!(w.world_hash(), with_item);

        let mut w = walker();
        if w.spawn_animal("pig", [1.0, 5.0, 1.0], 0.0).is_some() {
            assert_ne!(w.world_hash(), base);
        }
    }
}
