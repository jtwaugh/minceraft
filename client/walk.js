// Walking mode: the wasm `Walk` player (crates/game, Player::tick) stepped at
// 20 Hz, with the camera at its interpolated eye. See
// docs/design/basic-gameplay.md sections 1 rule 1, 3 and 4.
//
// The sim's yaw convention is not fly.js's: yaw 0 faces -z (as in
// crates/game/src/camera.rs), turning right increases yaw.

/** Look vectors in the sim's convention. */
export function simLookVectors(yaw, pitch) {
  const [sy, cy, sp, cp] = [Math.sin(yaw), Math.cos(yaw), Math.sin(pitch), Math.cos(pitch)];
  return {
    forward: [sy * cp, sp, -cy * cp],
    right: [cy, 0, sy],
  };
}

/**
 * Steps `walk` for the ticks the clock reports. Nothing is stepped until the
 * walk has terrain, so the player is not left falling through unloaded space.
 * Returns the eye to render at `alpha`.
 */
export function stepWalk(walk, ticks, alpha, nextInput) {
  if (walk.has_world()) for (let i = 0; i < ticks; i++) walk.tick(nextInput());
  return Array.from(walk.eye(alpha));
}
