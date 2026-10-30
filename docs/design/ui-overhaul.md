---
title: UI overhaul
status: accepted
scope: game
source: "#1138"
superseded_by: ""
phases:
  A: done
  B: done
  C: done
  D: todo
---

## Current state

Checked against the code:

- **Phase C done.** Crosshair art (#1355), nine-slice panel helper (#1356), hearts left and hands right with the label in the pixel font (#1362), timer on a panel with a sun/moon icon (#1375), belt on a panel strip with slot numbers (#1385), crosshair drawn from the texture (#1387) and the `hudsizes` scripts (#1389) are in (plan #1337); no `TEXT_CHAR_W` or `TEXT_H` remains in `client/`. Pop-up, wobble and flash were optional and are not built.
- **Phase B done.** The glyph art (#1248, #1251), the helper `client/font.js` with `client/test/font.test.mjs` (#1250) and the punctuation mapping (#1284, plan #1280) are in; `glyphName` maps A to Z, 0 to 9 and `. , : ! ? ' - / ( )`, and anything else draws `font_unknown`.
- **Phase A done.** One scale rule, `interfaceScale` in `client/uipx.js`, is used by the HUD, inventory, death screen and menu (#1161, #1166); the `layout` results are readable from `expect` (#1163); `client/test/scripts/hudsizes.input` checks the four sizes (#1169). It caps the belt at half the viewport width (`BELT_MAX_WIDTH_FRACTION = 0.5`); the one-third/30% target from #1151 is left to Phase B.
- **Scale (before Phase A).** `hudScale(w, h)` is `uiScale(w, h, 80, 45)` in `client/hud.js:22`. `uiScale` (`client/uipx.js:7`) returns the largest whole multiple that fits the base size. The inventory and death screen use their own `uiScale(w, h, 100, 60)` (`client/inventoryui.js:19`, `client/deathui.js:17`) and the menu uses `BASE_W` and `BASE_H` (`client/menu.js:85`). So there are three or four scale rules, not one.
- **Layout.** `client/hudlayout.js` computes exact rectangles from `textureSize`. Hearts and belt are in one column, left-aligned and centred as a block (`hudLayout`). The timer is a box centred at the top. Words with no texture use the fallback `TEXT_CHAR_W = 8` and `TEXT_H = 14` (12px monospace).
- **Layout checks.** The `layout <w> <h>` DSL statement (`client/test/dsl.mjs:240`) runs `hudLayout` and `layoutChecks` (`client/hudcheck.js`) and returns `problems`. The sizes in section 1 are not yet test cases in `client/test/scripts/`.
- **Text.** `12px ui-monospace` is set in `client/hud.js:51`, `client/inventoryui.js:33`, `client/deathui.js:28` and `client/index.html:8`. `play-critic-agent.md` note 7 (decided in #969) already says the timer's phase word and the hands' item name are to be drawn from textures.
- **Hint.** `#hint` in `client/index.html:11` is a `rgba(0,0,0,.55)` box with plain text, hidden by `client/scene.js:578` while the pointer is locked.
- **Crosshair.** `rects.crosshair` is placed by `mountHud` (`client/hud.js:98`); it is a CSS box, not a texture.
- **Art that exists** (`art/PROMPT.md`, `art/tools/draw_7_12.py`): `slot`, `slot_selected` (10x10), `panel_*` nine-slice (4x4), `digit_0..9` (4x6), `heart_*`, `sun`, `moon_*`. No letter or punctuation font, `phase_*`, `label_*`, crosshair, wordmark or pressed-panel texture exists.
- **Helper.** `client/uipx.js` has `findTexture`, `textureSize`, `pixelData`, `textureCanvas` and `textureUrl`. It imports nothing and the root page uses it, so any font helper placed beside it must also import nothing (no wasm).
- **Related docs.** `ui-art-and-inventory.md` (phases A to C done, built the current HUD), `root-menu.md`, `player-death.md`, `play-critic-agent.md`.

## Reconciliation notes

0. **Phase B was incomplete (plan #1173); fixed by #1284 (plan #1280).** Punctuation now maps to its glyphs.

1. **Item name per slot.** `itemLabel` in `client/inventoryui.js` derives labels from item names; `data/items.toml` `label` is not used. The pixel font should render whatever string `itemLabel` returns, so lower case or capitals-only (open question 2) decides what `label` values are allowed.
2. **Fixes #1003.** Section 1 says it fixes #1003 (hearts off both edges). The new scale rule and a `layout` test at the four sizes cover that; check #1003's own case when planning.
3. **`#1137` is separate.** The first-person held item and world item entities are out of scope; this doc only restyles existing HUD content.
4. **Menu page constraints.** The root page must not load wasm (`root-menu.md` note 6) and `.github/scripts/published_files.py` must find every file it loads. The font helper and its glyph data must satisfy both. Glyphs should come from `viewer/textures.json` like the other tiles (`art/tools/export_textures.py`), or the root page needs another way to fetch them.
5. **Art rules.** The font, `phase_*`, `label_*`, crosshair, wordmark and button textures are new art: add them to `art/PROMPT.md` and `art/tools/draw_7_12.py` (or a new generated batch script), never hand-edit the JSON, and keep the one 16-colour palette. `validate.py` and `test_art.py` must still pass. A glyph batch is likely over 200 reviewable lines, so split it (capitals and digits first, then punctuation).
6. **`hudLayout` text fallbacks.** Removing `TEXT_CHAR_W` and `TEXT_H` requires every string in the HUD to go through the font, including the hands label and the timer phase word. A string with a character the font lacks needs a defined fallback (blank cell or `?`) so layout stays exact.
7. **Test hooks.** All behaviour (scale rule, pop-up fade, heart wobble, flash) must be checkable from the controller or layout, not screenshots (CLAUDE.md "Testing the client"). Pop-up timing and damage flash need state in `client/controller.js` and `expect` lines in the DSL; the DSL may need extending.
8. **Debug console and overlay.** `client/consoleui.js` and `client/overlay.js` are debug tools. Giving them the panel and colours is cheap; using the pixel font there conflicts with their long, free-form output (open question 3).
9. **`[manual check]`** "Looks good", "consistent", "modest strip" and "never more than half the viewport". Measurable versions: the belt width in CSS pixels is at most 0.5 of the viewport width at the four sizes, and the target fraction from open question 1 holds within one scale step; no `problems` from `layoutChecks` at 390x844, 1366x768, 1920x1080 and 3440x1440.
10. **No world output change.** Everything is in `client/`, `art/` and tests; the wasm and terrain are untouched.

## Mince Raft: UI overhaul

Status: idea
Scope: game

The UI art exists now (slots, hearts, digits, the nine-slice wooden panel), but the HUD still looks rough: it's far too big, the pixel art sits next to plain monospace text, and every screen is styled a little differently. I want one consistent, good-looking pixel-art UI across the HUD, overlays and pages. Notes on the current code are there to help the reconciler and may be out of date.

---

## 1. Scale that fits

`hudScale` is `uiScale(w, h, 80, 45)`. That gives scale 17 at 1366x768 and 24 at 1920x1080, so a 10x10 slot is 170 to 240 CSS pixels wide, and the hearts row runs off both edges (#1003).

- One UI scale rule for the HUD, inventory, death screen and menu, chosen so the HUD takes up a modest strip of the screen: the belt about a third of the viewport width at most, never more than half. The rule is a fixed target: the belt is about 30% of the viewport width, and the scale is the largest integer at or below that, minimum 1. (Decided in #1151.)
- Nothing off-screen or overlapping at 390x844, 1366x768, 1920x1080 or 3440x1440. The `layout` DSL statement already checks this, so these sizes become test cases.
- Fixes #1003 as part of the overhaul, or before it.

> **Reconciler:** The arithmetic is right: `min(floor(1366/80), floor(768/45))` is 17 and 1920x1080 gives 24. Because the scale is an integer, "about a third" can only be met within one step at small viewports (at 390 wide a 5-slot belt of 10-texel slots plus gaps is about 60 texels, so scale 1 is already 15% and scale 2 is 31%).

## 2. One pixel font

Today the timer's phase word, the hands item name, the death screen, the inventory labels, the hint, the overlay and the console use `12px ui-monospace`. Next to scaled pixel art that's the main thing that looks wrong. `phase_*` and `label_*` textures were planned (play-critic-agent note 7) but none exist.

- A small bitmap font in `art/` (capitals A to Z, 0 to 9, basic punctuation, in a fixed 4x6 cell with a 3x5 glyph to match the digits; lower-case letters in a string are drawn with their capital glyph), drawn by a shared helper next to `uipx.js`, so any string renders as pixel art at the UI scale. (Decided in #1174.)
- The HUD, inventory, death screen and menu use it. That removes the text fallbacks in `hudLayout`, and every rectangle is exact.
- The debug console and overlay may stay monospace, since they're debug tools, but they get the same panel and colours.

> **Reconciler:** A font supersedes the per-word `phase_*` and `label_*` textures of play-critic-agent note 7: the same exact-rectangle goal is met by measuring a string from glyph widths. The existing `digit_*` (4x6) can be reused as the font's digits so the timer and slot numbers match.

## 3. A cohesive HUD

- **Belt** on a panel strip (the existing nine-slice wood) along the bottom centre, with slot numbers 1 to N in the pixel font.
- **Hearts** above the belt on the left, and the **hands** item (icon and label) above it on the right, instead of stacking everything in one left-aligned column.
- **Timer** on the wooden panel, not the `rgba(0,0,0,.55)` box, with a phase icon (`sun`, `moon` already exist) beside the digits.
- **Crosshair** as a small pixel-art texture at the UI scale, not a 16px CSS gradient.
- A short pop-up of the item name when the selected slot changes, then it fades.
- Optional: a heart wobble when health is low, and a brief flash when damage is taken.
- Every HUD part is always shown: hearts and the timer don't hide or fade when nothing changes, so a layout check can assume all of them are present. (Decided in #1346.)

> **Reconciler:** The crosshair is a CSS box today (see Current state), not a 16px gradient; the point stands. `sun` and `moon_*` exist, but `moon_*` is a set of eight 16x16 phases (`moon_0` to `moon_7`) in `art/PROMPT.md`, so the timer icon needs a small version or a chosen scale-down; that is new art. The pop-up fade and wobble are time-based, so they need a clock in the controller state to be testable (note 7).

## 4. Overlays and pages share one style

- The inventory, death screen and pause/hint all use the same panel, font, button style (a pressed and an unpressed panel state) and palette. The menu shares only the button style, as set out below.
- The first-load hint (`#hint` in `client/index.html`) becomes a panel with pixel text.
- The root menu gets a title graphic in `art/` (a "Mince Raft" wordmark) above its panel. The menu keeps its animated sea backdrop and gets only the wordmark and the shared button states, not the wooden panel for the rest of the screen. (Decided in #1433.)

> **Reconciler:** The menu already has a styled title screen with an animated sea (`ui-art-and-inventory.md` phase B, `client/menu.js`); the wordmark and shared panel go on top of it, and the "alpha" label must stay (`client/test/menu.test.mjs`). The pressed panel state is new art (`panel_*` has one state only).

## Out of scope

The first-person held item and item entities in the world are in #1137. Gameplay and HUD content don't change, only how things look and where they sit.

## Phases (reconciler)

**Phase A.** Section 1: one scale rule shared by the HUD, inventory, death screen and menu (replace the four `uiScale` calls with a single exported rule), with `layout` test scripts at 390x844, 1366x768, 1920x1080 and 3440x1440 that expect no `problems` and a belt width within the limits. Done when those scripts pass and #1003 is closed. Needs open question 1 decided first; if not, use the "belt at most one half of the viewport" bound, which the author already fixed, with the one-third target left to Phase B.

**Phase B.** Section 2: the bitmap font art (capitals, digits, punctuation, as the open question 2 decides) and the helper beside `uipx.js` that measures and draws a string at a scale. Done when the art passes `validate.py` and `test_art.py`, and a node test shows the helper's width and rects for several strings at several scales, with a defined fallback glyph.

**Phase C.** Sections 3 and 4 (HUD part): switch the HUD to the font and remove the text fallbacks in `hudLayout`; belt on a panel strip with slot numbers; hearts left and hands right above the belt; timer on a panel with a phase icon; crosshair texture. Done when `layout` scripts at the four sizes pass with no `problems` and no `TEXT_CHAR_W` or `TEXT_H` remains. Pop-up, wobble and flash are optional.

**Phase D.** Section 4: the shared panel, button states (pressed and unpressed) and palette on the inventory, death screen, hint and menu, plus the wordmark. Debug console and overlay get the panel and colours (open question 3 decides the font). Done when the inventory, death and menu tests still pass, the hint is a panel with font text, and the `[manual check]` look is accepted by the maintainer.

## Open questions

3. **Debug console and overlay font.** A: stay monospace with the shared panel and colours. B: move to the pixel font. A keeps long free-form output readable; B is more consistent.
