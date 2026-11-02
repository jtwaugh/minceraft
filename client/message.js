// The HUD message line's model (docs/design/action-feedback.md section 2): the
// latest action result or refusal as text, held and then faded in sim ticks.
// Pure, no DOM; hud.js passes it through and the HUD draws it.
//
//   const m = createMessage(templates);   templates as game-web's message_templates()
//   m.feed(events, ticks)  this frame's `name key=value ...` events and the ticks it ran
//   m.view() -> { text, alpha }            alpha 1 for hold_ticks, then down to 0 over fade_ticks; '' and 0 after
//
// Only the events and `action_refused` reasons the templates name produce text
// (pickups, tilling, planting, harvesting, foraging, refusals); swings, misses
// and everything else leave the message as it is. Of several in one frame the
// last wins. `{field}` takes the event's field, `{drop_key}` the drop key's label.

/** The fields of one event string: `crop_planted position=1,2,3 crop=wheat` -> { name, position, crop }. */
export function parseEvent(line) {
  const [name, ...pairs] = line.split(' ');
  const fields = { name };
  for (const p of pairs) {
    const eq = p.indexOf('=');
    if (eq > 0) fields[p.slice(0, eq)] = p.slice(eq + 1);
  }
  return fields;
}

const zip = (names, texts) => new Map(Array.from(names, (n, i) => [n, texts[i]]));

export function createMessage(templates) {
  const events = zip(templates.event_names, templates.event_texts);
  const reasons = zip(templates.reason_names, templates.reason_texts);
  const hold = templates.hold_ticks, fade = templates.fade_ticks;
  const dropKey = templates.drop_key;
  let text = '', age = 0;

  const textOf = (line) => {
    const f = parseEvent(line);
    const template = f.name === 'action_refused' ? reasons.get(f.reason) : events.get(f.name);
    if (template == null) return null;
    return template.replace(/\{(\w+)\}/g, (all, k) => (k === 'drop_key' ? dropKey : f[k] ?? all));
  };

  return {
    feed(lines, ticks) {
      let latest = null;
      for (const line of lines) latest = textOf(line) ?? latest;
      if (latest != null) {
        text = latest;
        age = 0;
      } else if (text) {
        age += ticks;
        if (age >= hold + fade) text = '';
      }
    },
    view() {
      if (!text) return { text: '', alpha: 0 };
      return { text, alpha: age <= hold ? 1 : 1 - (age - hold) / fade };
    },
  };
}
