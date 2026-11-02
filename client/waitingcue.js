// The "Click to play" cue (docs/design/click-to-play-cue.md Phase A). The condition is a
// pure function so the scene, Node tests and the input-script DSL share one rule.
//
//   waitingForClick({ locked, console, inventory, dead }) -> boolean

export const CUE_TEXT = 'Click to play';

/** True when the game is waiting for a click: pointer unlocked, console and inventory closed, player alive. */
export function waitingForClick({ locked, console: consoleOpen, inventory, dead }) {
  return !locked && !consoleOpen && !inventory && !dead;
}
