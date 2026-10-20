// The minimum HUD (docs/design/basic-gameplay.md section 12): crosshair, tool
// belt with the selected slot, the carried item, and health hearts. The model
// is plain data that Node tests check; the DOM renderer is a thin mirror.
//
//   hudModel({ belt, health, maxHealth }) ->
//     { crosshair, slots: [{ name, selected }], selected, hands, hearts: { full, half, empty } }
//   mountHud(document, parent) -> { update(model) }
//
// Health is in half-hearts, so 19 of 20 is nine full hearts and one half.

export function hudModel({ belt, health, maxHealth }) {
  const hp = Math.max(0, Math.min(health, maxHealth));
  const full = Math.floor(hp / 2), half = hp % 2;
  return {
    crosshair: true,
    slots: belt.slots.map((name, i) => ({ name, selected: i === belt.selected })),
    selected: belt.selected,
    hands: belt.hands,
    hearts: { full, half, empty: Math.ceil(maxHealth / 2) - full - half },
  };
}

const CSS = `
#hud { position: fixed; inset: 0; pointer-events: none; font: 12px/1.2 ui-monospace, monospace; color: #fff; }
#hud-cross { position: absolute; left: 50%; top: 50%; width: 16px; height: 16px; transform: translate(-50%, -50%);
  background: linear-gradient(#fff, #fff) center / 2px 100% no-repeat, linear-gradient(#fff, #fff) center / 100% 2px no-repeat; mix-blend-mode: difference; }
#hud-bottom { position: absolute; left: 50%; bottom: 12px; transform: translateX(-50%); display: flex; flex-direction: column; align-items: flex-start; gap: 4px; }
#hud-hearts { display: flex; gap: 2px; }
.heart { width: 14px; height: 14px; background: #3a0a0a; }
.heart.full { background: #d22; }
.heart.half { background: linear-gradient(90deg, #d22 50%, #3a0a0a 50%); }
#hud-belt { display: flex; gap: 2px; }
.slot { min-width: 48px; padding: 6px 4px; text-align: center; background: rgba(0, 0, 0, .55); border: 2px solid #555; }
.slot.selected { border-color: #fff; }
#hud-hands { min-height: 1.2em; text-shadow: 0 0 3px #000; }
`;

export function mountHud(document, parent = document.body) {
  const style = document.createElement('style');
  style.textContent = CSS;
  const root = document.createElement('div');
  root.id = 'hud';
  root.innerHTML = '<div id="hud-cross"></div><div id="hud-bottom"><div id="hud-hands"></div><div id="hud-hearts"></div><div id="hud-belt"></div></div>';
  parent.append(style, root);
  const cross = root.querySelector('#hud-cross'), hands = root.querySelector('#hud-hands');
  const hearts = root.querySelector('#hud-hearts'), belt = root.querySelector('#hud-belt');
  let last = '';
  return {
    update(m) {
      const key = JSON.stringify(m);
      if (key === last) return;
      last = key;
      cross.hidden = !m.crosshair;
      hands.textContent = m.hands ? `carrying ${m.hands}` : '';
      const kinds = [...Array(m.hearts.full).fill('full'), ...Array(m.hearts.half).fill('half'), ...Array(m.hearts.empty).fill('empty')];
      hearts.replaceChildren(...kinds.map((k) => Object.assign(document.createElement('div'), { className: `heart ${k}` })));
      belt.replaceChildren(...m.slots.map((s, i) => Object.assign(document.createElement('div'), {
        className: s.selected ? 'slot selected' : 'slot', textContent: `${i + 1} ${s.name ?? '-'}`,
      })));
    },
  };
}
