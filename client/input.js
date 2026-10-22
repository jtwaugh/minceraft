// Browser input for the client: pointer lock, DOM listeners, and the
// per-frame feed into the wasm InputSampler (crates/game-web). Key names are
// mapped through data/input.toml inside the sampler; this file only forwards
// raw events. See docs/design/basic-gameplay.md sections 2.1 and 2.2.
//
//   const input = attachInput({ sampler: new InputSampler(), canvas, document, window });
//   input.look()   every frame: { yaw, pitch } for the camera
//   input.tick()   at each sim tick boundary: the TickInput, as a plain object
//   input.drain()  the same, as the wasm Tick the sim bindings take (caller frees it)
//   input.console  true while the debug console is open (docs/design/debug-console.md 2)
//   input.closeConsole({ submitted })  leave console mode; submitted re-requests pointer lock
//   input.inventory  true while the inventory view is open (docs/design/ui-art-and-inventory.md note 8)
//   input.closeInventory({ relock })  leave inventory mode; relock re-requests pointer lock
//   input.detach() remove every listener
//
// With `debug` on, Backquote (the code, not the character) opens the console
// from a locked pointer: it drops held inputs, releases the pointer, and stops
// forwarding key, mouse and wheel events to the sampler, so the sim keeps
// ticking with neutral input. Enter or Escape closing it is the text box's job.
//
// The inventory key (is_inventory_key, from data/input.toml) does the same from
// a locked pointer, with or without `debug`. It does nothing while the console is
// open, and Backquote does nothing while the inventory is. The same key closes
// it and asks for the pointer again; Escape closes it and leaves the pointer free.

const MOUSE_KEYS = ['MouseLeft', null, 'MouseRight']; // MouseEvent.button 0, 1, 2

const TICK_FIELDS = [
  'move_forward', 'move_strafe', 'yaw', 'pitch', 'jump', 'sneak', 'sprint', 'primary',
  'secondary', 'secondary_held', 'interact', 'drop', 'tool_slot',
];

export function attachInput({ sampler, canvas, document, window, debug = false }) {
  let locked = false;
  let consoleOpen = false;
  let inventoryOpen = false;
  const cleanups = [];
  const on = (target, type, fn, opts) => {
    target.addEventListener(type, fn, opts);
    cleanups.push(() => target.removeEventListener(type, fn));
  };

  // Held inputs are dropped whenever we stop receiving events for them.
  const release = () => sampler.clear_held();

  on(canvas, 'click', () => {
    if (!locked && !inventoryOpen) canvas.requestPointerLock();
  });

  // Escape (and any other way out) arrives here, as the browser owns that key.
  on(document, 'pointerlockchange', () => {
    locked = document.pointerLockElement === canvas;
    if (!locked) release();
  });
  on(window, 'blur', () => {
    release();
  });

  const closeInventory = ({ relock = false } = {}) => {
    if (!inventoryOpen) return;
    inventoryOpen = false;
    release();
    if (relock) canvas.requestPointerLock();
  };

  on(document, 'keydown', (e) => {
    if (inventoryOpen) {
      // the pointer is already free, so Escape reaches the page
      if (e.code === 'Escape' || sampler.is_inventory_key(e.code)) {
        e.preventDefault();
        closeInventory({ relock: e.code !== 'Escape' });
      }
      return;
    }
    if (locked && !consoleOpen && sampler.is_inventory_key(e.code)) {
      e.preventDefault();
      inventoryOpen = true;
      release();
      document.exitPointerLock?.();
      return;
    }
    if (debug && locked && !consoleOpen && e.code === 'Backquote') {
      e.preventDefault();
      consoleOpen = true;
      release();
      document.exitPointerLock?.();
      return;
    }
    if (!locked || consoleOpen || inventoryOpen) return;
    e.preventDefault();
    if (!e.repeat) sampler.key_down(e.code);
  });
  on(document, 'keyup', (e) => {
    if (!locked || consoleOpen || inventoryOpen) return;
    e.preventDefault();
    sampler.key_up(e.code);
  });

  on(document, 'mousedown', (e) => {
    const key = MOUSE_KEYS[e.button];
    if (locked && !consoleOpen && !inventoryOpen && key) sampler.key_down(key);
  });
  on(document, 'mouseup', (e) => {
    const key = MOUSE_KEYS[e.button];
    if (locked && !consoleOpen && !inventoryOpen && key) sampler.key_up(key);
  });
  on(document, 'mousemove', (e) => {
    if (locked && !consoleOpen && !inventoryOpen) sampler.mouse_move(e.movementX, e.movementY);
  });
  on(document, 'wheel', (e) => {
    if (!locked || consoleOpen || inventoryOpen) return;
    e.preventDefault();
    sampler.wheel(e.deltaY);
  }, { passive: false }); // document wheel listeners default to passive
  on(canvas, 'contextmenu', (e) => e.preventDefault());

  return {
    get console() {
      return consoleOpen;
    },
    closeConsole({ submitted = false } = {}) {
      if (!consoleOpen) return;
      consoleOpen = false;
      release();
      if (submitted) canvas.requestPointerLock();
    },
    get inventory() {
      return inventoryOpen;
    },
    closeInventory,
    get locked() {
      return locked;
    },
    look() {
      return { yaw: sampler.yaw, pitch: sampler.pitch };
    },
    drain() {
      return sampler.drain();
    },
    tick() {
      const t = sampler.drain();
      const out = {};
      for (const f of TICK_FIELDS) out[f] = t[f];
      out.tool_slot = out.tool_slot < 0 ? null : out.tool_slot;
      t.free();
      return out;
    },
    detach() {
      for (const c of cleanups) c();
      cleanups.length = 0;
    },
  };
}
