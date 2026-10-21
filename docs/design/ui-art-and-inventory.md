---
title: UI art and inventory view
status: accepted
scope: game
source: "#502"
superseded_by: ""
phases:
  A: done
  B: todo
  C: todo
---

## Current state

What exists today that this doc relies on or overlaps (checked against the code):

- **HUD.** `client/hud.js` has `hudModel` (plain data, node-testable) and `mountHud`, which draws the crosshair, belt slots, hands text ("carrying X") and hearts with CSS boxes and text. Slots are `N name` text, hearts are coloured divs, and `data/items.toml` labels are not used. `client/scene.js` mounts it and updates it every frame from `controller.state.hud` (`client/controller.js`).
- **No day phase timer.** The phase clock of basic-gameplay section 12 is not built (see `basic-gameplay.md`, Reconciliation note 0), so the timer this idea lists has no data to show yet.
- **Menu.** The root `index.html` has inline CSS (monospace links, a yellow `.alpha` tag) and `client/menu.js`, whose `menuModel` and `renderMenu` build the entries from `client/scenes.js` (`root-menu.md`, done). The "alpha" label is `entry.alpha`, tested in `client/test/menu.test.mjs`.
- **Items.** `data/items.toml` defines tools `sword`, `hoe` and items `seeds`, `raw_beef`, `leather`, `raw_pork`, `raw_chicken`, `feather`, `wheat`, each with a `label`.
- **Art that exists** (`art/PROMPT.md`, `art/batches_*.json`): `hoe`, `seeds`, `wheat`, `heart_full`, `heart_half`, plus Batch 10 "INVENTORY" in `art/tools/draw_7_12.py`: `slot` and `slot_selected` (10×10), the nine-slice `panel_*` (4×4) and `digit_0..9` (4×6). Batches 4 and 6 also list `hunger_*`, `sun`, `moon_*` and `cloud`. Nothing in `client/` uses the Batch 10 tiles yet.
- **Art that is missing:** no `sword`, `raw_beef`, `leather`, `raw_pork`, `raw_chicken` or `feather` texture is listed in `art/PROMPT.md`. Batch 4 names `raw_salmon`, `raw_tuna`, `raw_mackerel` and `raw_squid`, not land meats.
- **Tileset in the client.** `client/tiles.js` builds a 16×16 texture array from `viewer/textures.json` for blocks only (`NAMES`); the HUD and menu are DOM and never read `textures.json`. Whether `textures.json` includes non-8×8 and item textures was not verified here; the export is `art/tools/export_textures.py`.
- **Pixel scaling.** `viewer/index.html` uses `image-rendering: pixelated` for its map only; the client has no pixel-art DOM rendering yet.
- **Pointer release and modes.** `client/input.js` already has a console mode on `Backquote` (debug only) that releases pointer lock and closes with Enter or Escape (`debug-console.md` section 2), and `client/test/dsl.mjs` has the input-script DSL with `mode = ...` expectations.
- **Phase A done** (#513): PRs #549 (sword, leather, feather icons), #568 (raw_beef, raw_pork, raw_chicken icons) and #569 (every `data/items.toml` name has a texture, export carries UI tiles) added the six icons to `art/PROMPT.md` and `art/tools/draw_7_12.py` and the coverage check in `art/tools/test_art.py`. This supersedes the "Art that is missing" item above.
- **Belt and hands data.** `PlayerState` in `crates/game/src/player.rs` holds the belt (5 slots) and one hands item; the client reads them via `belt` and `walk.health()` in `client/controller.js`.

## Reconciliation notes

1. **Rule 4 conflict (open question 1).** `basic-gameplay.md` section 1 rule 4 says "There is no inventory grid", and section 15 asks whether the belt is fixed. A readout of belt, hands and later crates is consistent with it; a real multi-slot inventory reverses it and would also change pickup (section 6: hands carry one item), `give`/`clear` in `debug-console.md`, and the Overcooked design. Recommended: readout only (open question 1, option A).
2. **Day phase timer has no source.** The idea lists the phase timer as art to restyle. It is blocked on basic-gameplay Phase 8 (clock and HUD timer, #470). Style it when it exists; do not build the clock here.
3. **Missing item art.** Icons are wanted for every entry in `data/items.toml`, but `sword`, `raw_beef`, `leather`, `raw_pork`, `raw_chicken` and `feather` are not in `art/PROMPT.md`. They must be added to the brief and drawn in `art/tools/draw_7_12.py` (`batches_7_12.json` is generated; never hand-edit it, CLAUDE.md "Art"). The icon set should be checked by a test that every `name` in `data/items.toml` has a texture, so a new item cannot ship without an icon. Each of the six gets its own distinct 8×8 icon now, with no shared placeholder, and the coverage test has no exceptions. (Decided in #515.)
4. **Sizes (open question 2).** Existing Batch 10 art (`slot` 10×10, `panel_*` 4×4, `digit_*` 4×6) is already a UI sheet at non-8×8 sizes, and the format supports it through `sizes` (`art/README.md`). Item icons at 8×8 scaled by an integer factor fit a 10×10 slot opening of 8×8. Hearts exist at 8×8. So option A (8×8 tiles scaled up, UI pieces at the sizes already drawn) needs no new format. Decision: option A; there is no separate larger UI sheet. (Decided in #514.)
5. **Client reads the same build.** The idea wants art read "from the same tileset build as the blocks". `client/tiles.js` only builds block layers for WebGL. DOM UI would need either the same `textures.json` (check it carries item and UI textures and non-8×8 sizes) or per-texture data URLs generated in JS. Either keeps "no framework or bundler". The root menu page (`index.html`) does not load `viewer/textures.json` today and must not load wasm (`root-menu.md` note 6); fetching the JSON is fine, but `.github/scripts/published_files.py` must still find every file it loads.
6. **Crisp scaling.** Use integer scale factors and `image-rendering: pixelated` on canvas or `<img>`. `[manual check]` "crisp at any screen size". Measurable version: a unit test that the scale function returns an integer `>= 1` for any viewport size.
7. **`[manual check]` criteria.** "Nice-looking, consistent" art and a "styled" title screen are judgments. Measurable parts: all art passes `validate.py` with the existing palette; every item has an icon; the menu still shows "alpha" on Test play while `ALPHA` is set (existing menu test).
8. **Inventory view and input modes.** It is a new client mode like console mode, opened with a key (suggest `Tab` or `I`, as a code in `data/input.toml`, not the character) and closed with the same key or Escape. Unlike the console it is not debug-only. While open, the sim keeps ticking with neutral input, as for the console (decided in #402 for the console; this doc should confirm it for the inventory, see open question 4). Logic belongs in `client/controller.js` and `client/input.js` with `client/test/scripts/*.input` scripts (CLAUDE.md "Testing the client"); no screenshots.
9. **Invariants.** No terrain, `no_std` or world-output change. Wasm size is unaffected if the view is client-only and reads state already exported (belt, hands). Texture JSON growth adds to what ships to the browser; report sizes in PRs.
10. **No SPEC.md involvement.** The doc states no Beta behaviour. Do not copy Minecraft's inventory layout or textures; art is this project's own.
11. **Overlap with other docs.** `root-menu.md` (done) owns the menu structure, which this doc only restyles; `debug-console.md` owns pointer release patterns; `basic-gameplay.md` owns the belt, hands and HUD content (sections 6 and 12). No conflicts found beyond note 1.
12. **Phases.** The idea has none; the three below keep each PR near the 200-line limit and put the inventory view last because it depends on the icons.

## Mince Raft: UI art and inventory view

Status: idea
Scope: game

The HUD and menu work but look like debug output. I want real UI graphics, plus a way to see what I'm carrying. Notes on the current code are there to help the reconciler and may be out of date.

---

## 1. UI graphics

Nice-looking, consistent art for:

- **The action bar** (tool belt): slot frames, the selected-slot highlight, the hands indicator, hearts and the day phase timer. Today `client/hud.js` draws these with plain CSS boxes and text.
- **Items**: an icon for every tool and item in `data/items.toml`, shown in the belt, the hands indicator and the inventory view. Some exist in `art/` already (`hoe`, `seeds`, `heart_full`, `heart_half`); others, such as the sword, don't (see `art/PROMPT.md`).
- **The root menu** (`index.html`, `client/menu.js`): a styled title screen instead of a monospace link list. The "alpha" label on Test play stays.

All art is this project's own, in the existing `art/` format and palette (CLAUDE.md "Art"), and the client reads it from the same tileset build as the blocks. Pixel art scaled up crisply (no blurring) at any screen size.

> **Reconciler:** Slot frames, the selected highlight, nine-slice panels and
> digits already exist as Batch 10 art but are unused (Current state). The
> day phase timer has no data until basic-gameplay Phase 8 (note 2). Six item
> icons are missing (note 3). Crisp scaling is `[manual check]` (note 6).

## 2. Inventory view

A view, opened with a key, that shows the items I have: the belt's tools, what's in my hands, and anything else I'm holding.

- Today there is nothing to show beyond the belt and hands: basic-gameplay rule 4 says "There is no inventory grid", and the hands carry one item. Decide whether this view is only a readout of the belt and hands (and later stations and crates), or whether it means adding a real inventory, which reverses rule 4.
- Opening it should release the pointer the way the debug console does, and close with the same key or Escape.

> **Reconciler:** See notes 1 and 8. As a readout the view shows five belt
> slots and one hands item, so it is mostly a larger, labelled rendition of
> the HUD; "anything else I'm holding" has no source today.

## Phases

> **Reconciler:** The idea has no phases.

### Phase A: Item art and UI sheet

- Add the missing item icons (`sword`, `raw_beef`, `leather`, `raw_pork`, `raw_chicken`, `feather`) to `art/PROMPT.md` and `art/tools/draw_7_12.py`, regenerate the JSON, and confirm the export carries items, hearts and the Batch 10 UI tiles.
- A test that every tool and item name in `data/items.toml` has a texture.

Done when: `validate.py` prints `ok` with a nonzero count, the item-coverage test passes, and `CI` finds `batches_7_12.json` matching the script.

### Phase B: Styled HUD and menu

- `client/hud.js` draws slot frames, the selection highlight, item icons, the hands indicator, hearts (full, half, empty) and digits from the tileset, with integer pixel scaling. The model (`hudModel`) stays DOM-free and tested.
- Root `index.html` and `client/menu.js` get the title-screen styling, keeping the "alpha" label (`client/test/menu.test.mjs` still passes). The menu background is a CSS-animated raft or sea built from tiles, with no wasm loaded on the root page. (Decided in #585.)
- The day phase timer is styled when the clock exists; otherwise this phase leaves it out.

Done when: the HUD and menu model tests pass, the scale-factor test passes, `published_files.py` finds every file the pages load, and the look is accepted `[manual check]`.

### Phase C: Inventory view

- Inventory mode in `client/input.js` and `client/controller.js`: a key code in `data/input.toml`, pointer released on open, closed by the same key or Escape. Input scripts cover open, close, Escape and that held movement keys are released.
- The view shows the belt, the hands item and a label per item, using the Phase B pieces.
- If open question 1 is answered "real inventory", this phase is replaced by a design update to basic-gameplay and a new plan; it is not built here.

Done when: the input scripts pass and the view's model test lists the belt and hands for a given state.

## Open questions (author's)

1. Inventory: a readout of what exists (belt, hands, later crates), or a real multi-slot inventory that changes rule 4?
2. Do the menu and HUD use the 8×8 tiles scaled up, or a separate UI sheet at a larger size (`sizes` in the batch)?
3. Should the menu show anything live behind it (a slow flyover, the raft bobbing), or stay static?

## Open questions

1. **Inventory kind** (author's question 1). Options: A) readout of belt, hands and later crates, rule 4 unchanged (recommended, note 1); B) a real multi-slot inventory, which reverses rule 4 and needs its own design doc first.
4. **Sim while open.** Options: A) keep ticking with neutral input, as the console does (recommended, matches #402); B) pause the sim, which puts a pause flag in the Rust command layer's territory (`debug-console.md` section 9).
5. **Open key.** Options: A) `Tab`; B) `KeyI`; C) `KeyE`, which is already Interact (not advised). Stored in `data/input.toml` by code.
