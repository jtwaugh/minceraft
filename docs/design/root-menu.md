---
title: Root menu
status: done
scope: game
source: "#447"
superseded_by: ""
phases:
  A: done
  B: done
---

## Current state

- Phase B is done (plan #464, PRs #468 and #490): `.vercelignore` publishes the root `index.html`, `viewer/` and `client/` (with `vercel.json`), and `.github/scripts/published_files.py` (run in CI, tested by `test_published_files.py`) fails if any file the three pages load is not published. Applying the host's project settings stays `[external]`. The bullets below predate Phase A and B.
- Phase A is done (plan #453, PRs #457 and #460): `client/scenes.js` holds the scene list, alpha flag and `menuEntries`, `client/scene.js` resolves `?scene=` through it, and the root `index.html` with `client/menu.js` links to `viewer/` and `client/`, tested by `client/test/scenes.test.mjs` and `client/test/menu.test.mjs`. The bullets below predate it. Phase B (deploy) is still `todo`.
- No menu exists. `vercel.json` sets `outputDirectory` to `viewer`, and
  `.vercelignore` uploads only `/viewer` and `/vercel.json`, so the deployed
  root page is `viewer/index.html` (the world generator viewer and flyover).
- `client/index.html` is the game client page. It loads `./scene.js`, which
  imports `../viewer/pkg/terrain_web.js`, `../viewer/mesh.js` and
  `./pkg/game_web.js`, and `client/tiles.js` imports `../viewer/mesh.js` and
  reads `viewer/textures.json`. So the client needs the `viewer/` modules and
  `viewer/pkg/` next to it; the relative `../viewer/` paths only work if the
  deployed tree keeps the repo layout.
- `client/pkg/` is built by `./build.sh` (`wasm-pack build crates/game-web`)
  and is gitignored, like `viewer/pkg/`. `./build.sh --serve-client` serves
  the repo root for local use; `--serve` serves `viewer/` only.
- `client/scene.js` reads `?seed=`, `?radius=` (clamped 0 to 3), `?scene=`
  (only `raft` is recognised, as `const raftScene = qs.get('scene') === 'raft'`),
  `?mode=`, `?plants=`, `?debug=` and `?log=`. With no `?scene=` it generates
  terrain around the seed (default 12345, radius 1): this is the "single
  chunk" scene.
- `client/debug.js` (`debugEnabled`) is on for `?debug=1` or any non-empty
  `?scene=`, as `docs/design/debug-console.md` section 6 describes.
- There is no scene list anywhere; the one scene name is hard-coded in
  `client/scene.js`.
- The viewer has its own seed box (`#seed` in `viewer/index.html`) and does
  not read `?seed=` from the URL.

## Reconciliation notes

1. **Deploy layout (`[external]` for hosting, repo side in scope).** The doc
   says the deploy must include `client/`, `client/pkg/` and what both pages
   load from `viewer/`. Hosting config is `[external]`. The repo-side part is
   `.vercelignore` and `vercel.json`, which say `viewer` only. Because
   `client/` imports `../viewer/...`, the simplest layout is to deploy the
   repo root (menu `index.html`, `viewer/`, `client/`) rather than copy files.
   Note `.vercelignore` would then need `client/` and `client/pkg/` allowed
   and `art/`, `crates/`, `work/` and the rest left out.
2. **Viewer moving under a subpath.** Open question 1 asks about moving
   `viewer/`. The viewer uses only relative paths (`./pkg/`, `./mesh.js`,
   `./textures.json`, `./flyover/`), and `worker.js` imports `./pkg/`, so it
   works from any subpath. But `client/` imports `../viewer/`, `build.sh`
   writes `viewer/pkg`, CI and `CLAUDE.md` name `viewer/`, and the
   `--serve` flag serves it. Moving the directory is a large rename for no
   gain; a root `index.html` that links to `viewer/` and `client/` avoids it.
3. **Scene list in one place.** `raft` is a string literal in
   `client/scene.js`. For "adding a scene means adding an entry", the menu's
   list and the scene selection in `scene.js` should share one module, for
   example `client/scenes.js` exporting `[{ id, name, description }]`, with
   `scene.js` looking the id up instead of comparing to `'raft'`. The
   single-chunk scene has no `?scene=` value, so its entry needs `id: null`
   (or a reserved id) that maps to no query parameter.
4. **Alpha flag and debug mode.** `debugEnabled` treats any `?scene=` as debug
   mode. Once Test play moves into debug mode, a test scene would be
   reachable only with debug on, so either `scene` stops implying debug or
   the scene picker is shown only under `?debug=1` (the doc already says the
   rule "probably needs revisiting"). Smallest change now: leave
   `debugEnabled` alone and have the menu own the flag. Any change to
   `debug.js` needs a client input script (`client/test/scripts/*.input`) per
   CLAUDE.md.
5. **Alpha flag location.** `data/*.toml` is read at compile time by
   `crates/terrain/build.rs` and is for terrain constants, and the menu is
   plain JS with no bundler, so a toml value would not reach it. A constant
   exported from the scenes module (or a menu module) is the smallest fit.
   `[manual check]` that the on-screen "alpha" label is visible and legible.
6. **Menu is plain HTML + ES modules.** Matches the "no frameworks or
   bundler" invariant. It must not load wasm; it only links, so it adds
   nothing to wasm size. Menu logic (which entries show for a given flag and
   `?debug`) should live in a small DOM-free module so it can be tested with
   `node --test client/test/*.test.mjs`.
7. **No overlap with other design docs.** `debug-console.md` owns debug mode
   (section 6); this doc only depends on it. `basic-gameplay.md` and
   `world-layout.md` do not describe a menu.
8. No SPEC.md sections are involved; the doc states no Beta behaviour.

## Mince Raft: Root menu

Status: idea
Scope: game

A menu that renders as the root page of the site. It sends the user to one of two places: **World gen** or **Test play**. Notes on the current code are there to help the reconciler and may be out of date.

---

## 1. What the menu offers

- **World gen.** The existing world generator viewer and flyover (`viewer/`).
- **Test play.** The game client (`client/`), with a scene picker:
  - **Single chunk**: the default generated-terrain scene (`client/` with no `?scene=`).
  - **Each test scene**: today that's the raft scene (`client/?scene=raft`). Every scene added later shows up here too.

The scene list should come from one place (a small data list or module) so adding a scene means adding an entry, not editing the menu by hand.

> **Reconciler:** No scene list exists yet; see note 3. Phase A builds it.

## 2. Test play is alpha only

Test play is on the main menu **only while the game is in alpha**. Once the game has a real play mode, Test play moves out of the main menu and into debug mode (`?debug=1`, see `client/debug.js` and the debug console doc), next to the console and the other debug tools. The menu should be built so that move is a switch, not a rewrite: for example a single flag that says whether we're in alpha, which decides whether Test play appears on the main menu or only with debug mode on.

The menu should say this on screen too, for example a small "alpha" label on Test play.

> **Reconciler:** The on-screen label is `[manual check]`; a measurable
> version is a test that the rendered entry for Test play contains the text
> "alpha" while the flag is on and is absent from the main list when it is off.

## 3. Current state worth knowing

- `vercel.json` sets `outputDirectory` to `viewer`, and `.vercelignore` uploads only `viewer/`. So the deployed root page is the viewer today, and `client/` isn't deployed at all. The menu needs the deploy to include `client/`, `client/pkg/` and whatever both pages load from `viewer/` (hosting config itself is `[external]`; the repo side is what the build produces).
- `client/debug.js` turns debug mode on for `?debug=1` or any `?scene=`. With Test play moving into debug mode later, this rule probably needs revisiting.
- CLAUDE.md: no frameworks or bundler. The menu is a plain HTML page with ES modules.

> **Reconciler:** All three bullets check out against the code; see notes 1, 4 and 6.

## Phases

> **Reconciler:** The idea has no phases; these two keep each PR within the
> 200-line limit and put the deploy change last, where it can't break the
> current root page.

### Phase A: Scene list and menu page

- `client/scenes.js`: the scene list (`id`, `name`, optional `description`)
  and the alpha flag, with a pure function returning the menu entries for a
  flag and debug state. `client/scene.js` resolves `?scene=` through it.
- Root `index.html`: plain HTML + ES module menu with World gen
  (`viewer/`) and Test play (scene picker, "alpha" label), linking with
  relative paths.
- Node tests for the entry function: alpha on, alpha off with and without
  debug, an unknown scene id.

Done when: the entry tests pass, `?scene=raft` still selects the raft scene
(existing input scripts pass), and opening the root `index.html` from the
repo root (`./build.sh --serve-client`) shows both entries and links work.

### Phase B: Deploy the menu

- `vercel.json` and `.vercelignore` publish the root `index.html`, `viewer/`,
  `client/` and `client/pkg/`, with the viewer still reachable. Applying the
  host's settings is `[external]`.
- If CI builds the site, it checks that every file the two pages load is in
  the published set.

Done when: a script lists the published files and finds every module and
asset that `viewer/index.html` and `client/index.html` import.

## Open questions (author's)

1. Where the menu page lives: a new root `index.html`, or `viewer/` moves under a subpath and the menu takes its place.
2. What decides "alpha": a build-time flag, a value in `data/`, or a constant in the menu module.
3. Should scene entries carry a short description and a thumbnail, or just a name?
4. Should the menu pass through options like `?seed=` and `?radius=` (a seed box, a radius picker), or leave that to the query string?

## Open questions

1. **Menu location.** Options: A) new root `index.html`, deploy the repo
   root subset, `viewer/` stays (recommended, note 2); B) move `viewer/` to a
   subpath and put the menu in `viewer/`'s place (renames `client/`'s
   imports, `build.sh`, CI, docs).
2. **Alpha flag.** Options: A) constant exported from `client/scenes.js`
   (recommended, note 5); B) build-time flag written by `build.sh`; C) a
   `data/*.toml` value, which would need new plumbing outside `build.rs`.
3. **Scene entry content.** Options: A) name only; B) name plus a one-line
   description (recommended, cheap); C) also a thumbnail, which needs
   committed images that are this project's own art and a `[manual check]`.
4. **Options on the menu.** Options: A) none, query string only; B) a seed
   box and radius picker passed as `?seed=` and `?radius=` to Test play
   (`radius` is clamped 0 to 3 in `scene.js`); the viewer does not read
   `?seed=`, so a seed box would need a change there too (recommended: A for
   now).
5. **Debug mode rule.** When Test play moves into debug mode, should
   `?scene=` still imply debug (A, unchanged), or should it stop and the
   menu add `?debug=1` itself (B)?
