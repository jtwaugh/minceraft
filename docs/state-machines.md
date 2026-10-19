# State machine conventions

Phase E of `docs/design/error-and-state-conventions.md`, based on the audit
in `docs/errors-and-states.md`. This is a convention for new and converted
code. It is not part of `CLAUDE.md`; the maintainer decides whether to
promote it there.

## What counts as a state machine

A thing with a small, named set of states and events that move it between
them. A value that is only read (a flag fixed at load time, a counter, a
cache cursor) is not one.

`bindings::Mode` (`Hold | Edge | Continuous`) is an attribute, not a state
machine: it is fixed per action at load time and describes how input is
read. It has no transitions, so none of this applies to it.

## Naming states

- One name per state, a lowercase noun or adjective for what the thing *is*
  (`walking`, `flying`, `unlocked`, `locked`), not the event that led there.
- In Rust, an enum whose variants are those names in `PascalCase`. In the
  client, a single field holding one of a closed set of names, or a boolean
  when there are exactly two states (as now), documented where it is
  declared.
- Name transitions after the triggering event (`KeyF`, `click`,
  `pointerlockchange`), and write each one down with its guard.

## Invalid transitions

An invalid transition is an event that the current state has no transition
for (or whose guard fails).

- **Client machines (current rule):** ignore it. Return early, change
  nothing, raise no error and log nothing. The event is routine (auto-repeat,
  clicks while locked), not a fault.
- **Pinned by a test:** every ignored event has a DSL script case asserting
  that state, mode and position are unchanged afterwards. Ignoring must be a
  tested decision, not an accident of an early return.
- **Future Rust machines:** the transition function takes the current state
  and an event and returns the next state, as an explicit enum, so an invalid
  pair is visible in a `match`. Decide per machine whether the invalid pair
  is a no-op (return the current state, as the client does) or a bug
  (a returned error, not a panic). Whichever is chosen, it is stated in the
  doc comment of the enum and pinned by a unit test. Reported errors follow
  the error conventions of Phases B to D. Per the decision in #382, code that
  is found to be a de facto state machine is converted to an explicit enum,
  one ticket each; per #383 the client machines below were the first target
  and are tested through the DSL.

## Testing

1. **Write the failing script first.** For client behaviour, add a case to a
   `client/test/scripts/*.input` file (DSL in `client/test/dsl.mjs`). Do not
   use screenshots.
2. A case fires the event in the state where it is invalid, then `expect`s
   the state fields (`mode`, `locked`, `lock_requests`, eye position).
3. If the DSL can't express the event or the state, extend the DSL.
4. For a Rust machine, unit-test every `(state, event)` pair that is not a
   valid transition, plus the valid ones, and check determinism: the same
   event sequence gives the same final state.

## Current machines

Both are in the client, in JavaScript. There are no Rust machines.

| Machine | States | Code | Scripts |
|---|---|---|---|
| Fly / walk | walking, flying | `client/controller.js` (`state.flying`) | `client/test/scripts/modes.input` (toggle, F while unlocked or repeating), `client/test/scripts/transitions.input` (click in fly mode, input while unlocked or after unlocking) |
| Pointer lock | unlocked, locked | `client/input.js` (`locked`) | `client/test/scripts/transitions.input` (click while already locked, input while unlocked or after unlocking) |

Worked example, an invalid transition: `click` while already locked has no
lock-to-lock transition, so it is ignored. `transitions.input` case "click
while already locked makes no second lock request" clicks twice after the
lock and expects `lock_requests = 1`, `locked = true`, `mode = walk`.

Worked example, a guarded transition: `KeyF` moves walking to flying only
when `locked` and not `e.repeat`. `modes.input` case "F does nothing while
unlocked, or on auto-repeat" pins both guards, and "F toggles fly and back"
pins the valid pair.
