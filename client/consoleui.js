// The debug console's text box and output lines (docs/design/debug-console.md
// section 8): plain DOM over the canvas. The line buffer and fade timing are
// DOM-free (`OutputBuffer`) so Node tests cover them; `createConsoleUI` is the
// thin DOM layer, wired up in scene.js.

/** How long the output stays on screen after the console closes (or output arrives). */
export const FADE_DELAY_MS = 4000;
/** Output lines shown above the text box; the buffer keeps every line. */
export const VISIBLE_LINES = 10;

/**
 * The game sends output as plain strings, so errors are recognised by the
 * shapes of the messages in crates/game/src/commands.rs.
 */
const ERROR_SHAPES = [
  /^unknown /,
  /^wrong number of arguments/,
  /^bad value /,
  /^'.*' can't be used/,
  /^missing\b/,
  /^count must be /,
  /^bad command name/,
];

/** `'error'` for a line shaped like a command failure, else `'result'`. */
export function classify(text) {
  return ERROR_SHAPES.some((re) => re.test(text)) ? 'error' : 'result';
}

/** Every line printed this page session, with what is visible at a given time. */
export class OutputBuffer {
  constructor({ fadeMs = FADE_DELAY_MS, visible = VISIBLE_LINES } = {}) {
    this.fadeMs = fadeMs;
    this.visibleMax = visible;
    this.lines = []; // { kind: 'echo' | 'result' | 'error', text }
    this.activeAt = -Infinity; // last time a line arrived or the console closed
    this.open = false;
  }

  push(kind, text, now) {
    this.lines.push({ kind, text });
    this.activeAt = now;
  }

  /** Echo a submitted line. */
  echo(line, now) {
    this.push('echo', `> ${line}`, now);
  }

  /** Add a line the game printed, coloured by `classify`. */
  output(text, now) {
    this.push(classify(text), text, now);
  }

  setOpen(open, now) {
    if (open === this.open) return;
    this.open = open;
    if (!open) this.activeAt = now;
  }

  /** The lines to draw at `now`: the newest few while open or not yet faded, else none. */
  visible(now) {
    if (!this.open && now - this.activeAt >= this.fadeMs) return [];
    return this.lines.slice(-this.visibleMax);
  }
}

const COLOURS = { echo: '#9ad', result: '#fff', error: '#f88' };

/**
 * DOM layer. `onSubmit(line)` and `onCancel()` are called from the text box;
 * `history` and `complete` come from consolelogic.js.
 */
export function createConsoleUI({ document, buffer, history, complete, names, onSubmit, onCancel }) {
  const root = document.createElement('div');
  root.id = 'console';
  const out = document.createElement('div');
  out.id = 'console-out';
  const box = document.createElement('input');
  box.id = 'console-input';
  box.type = 'text';
  box.autocomplete = 'off';
  box.spellcheck = false;
  box.hidden = true;
  root.append(out, box);
  document.body.append(root);

  box.addEventListener('keydown', (e) => {
    if (e.code === 'Enter') {
      e.preventDefault();
      const line = box.value;
      box.value = '';
      history.add(line);
      onSubmit(line);
    } else if (e.code === 'Escape') {
      e.preventDefault();
      box.value = '';
      onCancel();
    } else if (e.code === 'ArrowUp') {
      e.preventDefault();
      box.value = history.up(box.value);
    } else if (e.code === 'ArrowDown') {
      e.preventDefault();
      box.value = history.down(box.value);
    } else if (e.code === 'Tab') {
      e.preventDefault();
      box.value = complete(box.value, names).line;
    }
  });

  return {
    /** Sync the box with console mode and redraw the output for time `now` (ms). */
    update(open, now) {
      if (open !== !box.hidden) {
        box.hidden = !open;
        if (open) box.focus();
        else box.blur();
      }
      buffer.setOpen(open, now);
      const lines = buffer.visible(now);
      out.replaceChildren(
        ...lines.map((l) => {
          const div = document.createElement('div');
          div.textContent = l.text;
          div.style.color = COLOURS[l.kind];
          return div;
        }),
      );
    },
  };
}
