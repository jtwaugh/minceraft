//! Recording a walking session. See docs/design/debug-console.md section 7.
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

use crate::input::TickInput;

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

/// Why a recording failed to parse.
pub type ParseError = String;

fn bad<T>(msg: impl Into<String>) -> Result<T, ParseError> {
    Err(msg.into())
}

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
fn parse_commands(s: &str) -> Result<Vec<String>, ParseError> {
    let inner = s
        .trim()
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .ok_or("commands must be a [..] list")?;
    let mut out = Vec::new();
    let mut chars = inner.chars().peekable();
    loop {
        match chars.find(|c| !matches!(c, ' ' | ',')) {
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

fn hex32(s: &str) -> Result<f32, ParseError> {
    u32::from_str_radix(s, 16)
        .map(f32::from_bits)
        .or_else(|_| bad(format!("bad float bits {s:?}")))
}

fn parse_input(body: &str) -> Result<TickInput, ParseError> {
    let mut input = TickInput::default();
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

    pub fn parse(text: &str) -> Result<Recording, ParseError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Recording {
        let input = TickInput {
            move_forward: 0.5,
            yaw: -0.1,
            jump: true,
            drop: true,
            tool_slot: Some(1),
            ..TickInput::default()
        };
        Recording {
            ticks: vec![
                TickRecord {
                    tick: 1,
                    input,
                    commands: vec![],
                },
                TickRecord {
                    tick: 2,
                    input: TickInput::default(),
                    commands: vec!["give hoe".into(), "say \"hi\" \\ x".into()],
                },
            ],
            cheated: true,
        }
    }

    #[test]
    fn recording_round_trips() {
        let rec = sample();
        assert_eq!(Recording::parse(&rec.to_text()).unwrap(), rec);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(Recording::parse("").is_err());
        assert!(Recording::parse("cheated maybe\n").is_err());
        assert!(Recording::parse("cheated false\ntick x: input {}; commands []\n").is_err());
        assert!(Recording::parse("cheated false\ntick 1: input {fwd=0}; commands []\n").is_err());
    }
}
