// Walking mode: the wasm `Walk` player (crates/game, Player::tick) stepped at
// 20 Hz, with the camera at its interpolated eye. See
// docs/design/basic-gameplay.md sections 1 rule 1, 3 and 4. The view matrix
// comes from the sim too (game_web view_matrix), so look conventions live in
// crates/game/src/camera.rs only.

/**
 * Steps `walk` for the ticks the clock reports. Nothing is stepped until the
 * walk has terrain, so the player is not left falling through unloaded space.
 * Returns the eye to render at `alpha`.
 */
export function stepWalk(walk, ticks, alpha, nextInput) {
  if (walk.has_world()) {
    const tick = () => {
      const t = nextInput();
      walk.tick(t);
      t.free?.(); // a wasm Tick; test fakes are plain objects
    };
    // `pause` (a console command, in the sim) stops the ticking; the sim still
    // runs queued commands, and `step n` asks for n ticks (debug-console.md 9).
    for (let i = 0; i < ticks && !walk.paused?.(); i++) tick();
    if (walk.paused?.()) {
      walk.run_commands();
      for (let n = walk.take_steps(); n > 0; n = walk.take_steps()) {
        for (let i = 0; i < n; i++) tick();
      }
    }
  }
  return Array.from(walk.eye(alpha));
}
