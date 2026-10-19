// DOM-free logic for the debug console's text box: history (Up/Down) and Tab
// completion (docs/design/debug-console.md section 8). Name lists come from
// game-web (`command_names`, `tool_names`, `item_names`, `species_names`).

const HISTORY_KEY = 'minceraft.console.history';
const HISTORY_MAX = 100;

/** The lists Tab completes from, read from the wasm module. */
export function nameLists(wasm) {
  return {
    commands: wasm.command_names(),
    tools: wasm.tool_names(),
    items: wasm.item_names(),
    species: wasm.species_names(),
  };
}

/**
 * Submitted lines, oldest first. `storage` is an optional localStorage-like
 * object; any failure to read or write it is swallowed.
 */
export class History {
  constructor(storage = null, { key = HISTORY_KEY, max = HISTORY_MAX } = {}) {
    this.storage = storage;
    this.key = key;
    this.max = max;
    this.lines = [];
    try {
      const saved = JSON.parse(storage?.getItem(key) ?? '[]');
      if (Array.isArray(saved)) {
        this.lines = saved.filter((l) => typeof l === 'string' && l !== '').slice(-max);
      }
    } catch {
      this.lines = [];
    }
    this.cursor = this.lines.length;
    this.draft = '';
  }

  /** Record a submitted line; blank lines and repeats of the last one are skipped. */
  add(line) {
    this.cursor = this.lines.length + 1;
    this.draft = '';
    if (line.trim() === '' || this.lines[this.lines.length - 1] === line) {
      this.cursor = this.lines.length;
      return;
    }
    this.lines.push(line);
    if (this.lines.length > this.max) this.lines.splice(0, this.lines.length - this.max);
    this.cursor = this.lines.length;
    try {
      this.storage?.setItem(this.key, JSON.stringify(this.lines));
    } catch {
      // Storage may be full, blocked or missing; history stays in memory.
    }
  }

  /** Older entry. `current` is the box's text, kept as the draft on the first step back. */
  up(current) {
    if (this.lines.length === 0) return current;
    if (this.cursor === this.lines.length) this.draft = current;
    if (this.cursor > 0) this.cursor--;
    return this.lines[this.cursor];
  }

  /** Newer entry; past the newest, the unfinished line. */
  down(current) {
    if (this.cursor >= this.lines.length) return current;
    this.cursor++;
    return this.cursor === this.lines.length ? this.draft : this.lines[this.cursor];
  }
}

function commonPrefix(words) {
  let p = words[0];
  for (const w of words) while (!w.startsWith(p)) p = p.slice(0, -1);
  return p;
}

/** Names that complete word `index` of a line whose earlier words are `words`. */
function candidates(words, index, names) {
  if (index === 0) return names.commands;
  const cmd = words[0];
  if (cmd === 'give' && index === 2) {
    if (words[1] === 'tool') return names.tools;
    if (words[1] === 'item') return names.items;
  }
  if (cmd === 'give' && index === 1) return ['tool', 'item'];
  if (cmd === 'spawn' && index === 1) return [...names.items, ...names.species];
  if (cmd === 'help' && index === 1) return names.commands;
  return [];
}

/**
 * Tab on `line` (the caret is at its end). Returns the new text and the
 * matching names; a unique match gets a trailing space, an ambiguous prefix
 * extends to the common prefix, no match leaves the line alone.
 */
export function complete(line, names) {
  const m = /(\S*)$/.exec(line);
  const word = m[1];
  const before = line.slice(0, m.index);
  const words = before.split(/\s+/).filter(Boolean);
  const matches = [...new Set(candidates(words, words.length, names))]
    .filter((n) => n.startsWith(word))
    .sort();
  if (matches.length === 0) return { line, matches };
  if (matches.length === 1) return { line: `${before}${matches[0]} `, matches };
  return { line: before + commonPrefix(matches), matches };
}
