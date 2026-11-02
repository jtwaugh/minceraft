---
title: Centre the belt and check HUD size on laptops
status: accepted
scope: game
source: "#2001"
superseded_by: ""
phases:
  A: todo
  B: todo
---

## Current state

Checked against the code:

- **Layout.** `hudLayout` (`client/hudlayout.js:55`) puts the belt panel and the hands panel in one row (`rowW = panelW + handsGap + handsPanelW`, line 79), takes `colW = max(rowW, heartsW)`, and centres that column (`left = floor((vw - colW) / 2)`, line 81). The belt panel starts at `left`, so the belt itself sits left of the viewport centre by about half of `handsGap + handsPanelW`. Hearts start at `left` (line 109), so they sit on the belt's left edge, as the doc wants to keep.
- **Scale.** `hudScale` is `interfaceScale` (`client/hud.js:24`, `client/uipx.js:26`). It is the least of: belt-width limit (`BELT_MAX_WIDTH_FRACTION = 0.5`), `floor(vh / MIN_VIEW_TEXELS_H)` (`MIN_VIEW_TEXELS_H = 160`) and the 100x65 panel fit. It gives scale 4 at 1366x768 and 3 at 390x844, so the scale-11 finding in #1520 is already fixed. The `hudsizes` scripts assert those two values (`client/test/scripts/hudsizes.input`).
- **Checks.** `client/hudcheck.js` has `overlapChecks`, `offscreenChecks`, `scaleChecks`, `clearZoneChecks`, `handsDistanceChecks` and `containmentChecks`, combined in `layoutChecks`. There is no centring check and no check on the height of the HUD.
- **Sweep.** `client/test/scripts/hudsizes.input` runs `layout <w> <h>` at 390x844, 1366x768, 1920x1080 and 3440x1440 and expects `layout.problems = 0`, belt fraction at most 0.5, and the timer, slot numbers and hands label inside the viewport. `layout.belt.w` and `layout.belt.fraction` are readable paths (`client/test/dsl.mjs`); no path exposes the belt's centre or the HUD height. Unit tests are in `client/test/hudlayout.test.mjs` and `client/test/hudcheck.test.mjs`.
- **Not yet satisfied.** Everything in section 2: the belt is not centred (by the current code, about 36 px left of centre at 1366x768 and 27 px at 390x844), and neither check exists.

## Reconciliation notes

- **Numbers in the idea are from earlier code.** The idea reports the belt at x 495, width 232 at 1366x768. The current code gives scale 4 there, so the panel is 248 px wide (`BELT_TEXELS_W = 62`), not 232. The off-centre finding still stands, with different figures. The tests should use the code's numbers, not the idea's.
- **The scale check the idea mentions (#1520) is already covered** by the `layout.scale = 4` expectation. The new height check is a different guard (HUD height as a fraction of the viewport) and would catch a scale regression of the 37% kind. Suggest keeping the `layout.scale` expectations as they are.
- **Fallback needs a precise rule.** "If the hands strip would then run off a narrow screen, fall back to centring the whole column." Suggest: centre the belt panel; if `beltPanelX + panelW + handsGap + handsPanelW > vw`, use the current column placement. Because `interfaceScale` limits the belt to half the width, the hands strip (about 56 texels wide with its gap) fits whenever the viewport is wider than about 118 texels at that scale, so the fallback should rarely fire. A unit test with a tiny viewport (or a model that forces it) is needed to cover it.
- **Hearts and the hands label follow the belt.** Hearts are placed at `left` and the label is right-aligned to the hands strip (lines 109 and 132), and the `beside` test (line 134) compares them. Moving the belt changes both, so the label-versus-hearts and crosshair-clear-zone logic needs a re-run at every sweep size. A wider hearts row than the belt (`heartsW > panelW`) would make the hearts extend left past the belt's left edge only if they are left-aligned to it; the doc says hearts stay on the belt's left edge, so keep `hearts.x = beltPanel.x` and let the offscreen check catch the narrow case.
- **Tolerance and fraction are constants to choose.** "A few pixels" and "for example 20%" are not numbers. Suggest `MAX_BELT_OFFCENTRE = 2` CSS pixels (belt panel centre is integer-rounded, so an odd width differs by up to 1) and `MAX_HUD_HEIGHT_FRACTION = 0.2`, both exported from `client/hudcheck.js` beside `MIN_SCALE`. Rough check of the fraction on today's layout: the belt panel, the gap and one row of hearts come to well under 20% at 1366x768 and 390x844. The tests should confirm this before the constant is committed; if it does not hold at some sweep size, report the size and the measured fraction rather than raising the constant. The belt-centre tolerance is decided: `MAX_BELT_OFFCENTRE = 2` CSS pixels. (Decided in #2021.)
- **The belt-centre check cannot run in the fallback case.** When the layout falls back to centring the column, the belt is off-centre on purpose. The check should skip, or the fallback should be reported as an advisory (as `scaleChecks` does for portrait), so a narrow viewport does not become a `finding`. See open question 1.
- **"Bottom block" definition.** Take it as the vertical extent from `rects.hearts.y` to the bottom of `rects.beltPanel`. The bottom margin (`BOTTOM_MARGIN = 12`) is not part of it. The hands label is excluded: it is a different element and already has its own distance check.
- **Existing doc overlap.** `ui-overhaul.md` (all phases done) set up the scale rule and the `hudsizes` scripts; `play-critic-agent.md` section 3 defines the mechanical checks and their constants. This doc extends both and does not conflict with either. Do not edit them; the docs updater records it.
- **Invariants.** Client JS only: no change to `crates/terrain`, SPEC.md or world output, and no wasm size change. `hudlayout.js` stays DOM-free. No `[external]` items.
- **`[manual check]`** "A few pixels" and "no taller than a set fraction" are measurable once the constants above are fixed. Whether the centred belt looks right under the crosshair is a visual call for the maintainer; the measurable version is the belt-centre check at the sweep sizes.

# Mince Raft: Centre the belt and check HUD size on laptops

Status: idea
Scope: client (HUD layout and layout checks)

Two layout sweeps flagged the HUD. At 1366x768 the belt sat at x 495, width 232, centred at 611 against a viewport centre of 683; on a 390x844 portrait phone it was 54 px left of centre (#1574). An earlier run measured the HUD at scale 11, covering 37% of the screen height at that size (#1520). Neither was caught by the layout check. Notes on the current code may be out of date; they are there to help the reconciler.

---

## 1. What exists

- `client/hudlayout.js` `hudLayout`: the belt strip and the hands strip sit side by side in one centred column (`rowW = panelW + handsGap + handsPanelW`), with the hearts above on its left edge. The column is centred, so the belt alone sits left of the crosshair by about half the hands strip.
- `client/uipx.js` `interfaceScale`: already capped by both a belt width fraction and `MIN_VIEW_TEXELS_H`. The later run measured scale 4 at 1366x768, so the scale-11 finding looks fixed; this idea only asks for a check that keeps it fixed.
- `client/hudcheck.js` and the `layout <w> <h>` DSL command: checks overlap, off-screen, scale below 2, crosshair clear zone and belt-to-hands distance (play-critic-agent section 3, #970, #971). No centring check, no check on total HUD height.

> **Reconciler:** All three bullets match the code. The belt-width figures in the intro are from before the current scale rule; see the first reconciliation note.

## 2. Want

- Centre the belt strip itself under the crosshair, with the hands strip hanging off its right side, and the hearts on the belt's left edge as now. If the hands strip would then run off a narrow screen, fall back to centring the whole column.
- Add two checks to `hudcheck.js`, each a constant with a test: belt centre within a few pixels of the viewport centre, and the HUD's bottom block (hearts to belt) no taller than a set fraction of the viewport height (for example 20%).
- Run both over the existing viewport sweep, including 1366x768 and 390x844 portrait.

> **Reconciler:** The sweep is `client/test/scripts/hudsizes.input`; it already covers 390x844, 1366x768, 1920x1080 and 3440x1440. The checks join `layoutChecks`, so `expect layout.problems = 0` in those cases covers them with no new script lines, though a case that asserts the centre and height values directly is worth adding (needs two new `layout.*` paths in `dsl.mjs`).

## Source findings

#1520 item 3, #1574 item 3.

## Phases (reconciler)

**Phase A.** Centre the belt in `hudLayout` (section 2, first bullet) and add the belt-centre check to `hudcheck.js` with its constant and tests, in the same change: the check would fail on the old layout, so the two land together. Includes the narrow-screen fallback, its test, and a hudlayout test at 1366x768 and 390x844 that the belt panel's centre is within the tolerance of the viewport centre. Done when `hudsizes.input` still passes with `layout.problems = 0` at all four sizes, and `node --test client/test/*.test.mjs` passes. The check itself is added after the layout change in the same PR order, so no intermediate commit fails.

**Phase B.** The HUD-height check (section 2, second bullet): `MAX_HUD_HEIGHT_FRACTION` and a check in `hudcheck.js` joined to `layoutChecks`, a unit test with a layout that is too tall (for example a forced scale) and one that passes, and `layout.*` paths in `dsl.mjs` for the belt centre offset and the HUD height fraction, asserted in `hudsizes.input` at 390x844 and 1366x768. Done when those expectations pass at all four sweep sizes.

## Open questions

1. **Narrow-screen fallback and the centre check.** When the hands strip would run off the screen and the layout falls back to centring the whole column, should the belt-centre check:
   - A. skip (the fallback is by design); or
   - B. report an `advisory` severity, like `scaleChecks` on portrait; or
   - C. stay a `finding`, making the fallback visible as a failure?
