// Inventory view model (docs/design/ui-art-and-inventory.md Phase C): the belt
// and the hands item with a label each. Plain data for Node tests; drawing is a
// later ticket.
//
//   inventoryModel(set, belt, labelOf) -> { slots: [{ name, label, selected, frame, icon }], hands: { name, label } }
//
// `belt` is readBelt's result; `labelOf(name)` gives an item's display label.
// Empty slots and empty hands have a null name and label. An item without a
// texture still has its label and a null icon.

import { slotView } from './hud.js';

export { hudScale } from './hud.js';

export function inventoryModel(set, belt, labelOf) {
  const label = (name) => (name ? labelOf(name) : null);
  return {
    slots: belt.slots.map((name, i) => {
      const selected = i === belt.selected;
      return { name, label: label(name), selected, ...slotView(set, name, selected) };
    }),
    hands: { name: belt.hands, label: label(belt.hands) },
  };
}
