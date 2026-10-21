# EasyGCode 2: architecture

*Written 21 Oct 2026 for the first build. EasyGCode is the original idea (a phone-first app that
writes custom G-code), EasyCAD the experiment that grew out of it (Rust, recipes, Grasshopper,
an agent). This is the merge: EasyGCode's projects and JSON files, on EasyCAD's engine and app
shell, and nothing else.*

---

## 1. The idea in one picture

```
  project file (JSON)                       Rust engine (WebAssembly, in a Web Worker)
  ┌──────────────────────────┐   run    ┌─────────────────────────────────────────────────┐
  │ kind: "vase"             │ ───────▶ │ schema: fill in defaults, check every setting   │
  │ name, notes              │          │ kind:   compose toolbox steps → toolpath(s)     │
  │ settings: { … }          │ ◀─────── │ stats, warnings, hints, preview arrays          │
  │ printer:  { … }          │ complete │                                                 │
  └──────────────────────────┘ settings │ gcode:  one writer for every kind and printer   │
        ▲            ▲                  └─────────────────────────────────────────────────┘
        │            │                                   │
  settings window   project library                3D preview (three.js) · G-code file
  (drawn from the   (on the device, saved
   kind's schema)    as you go; JSON files)
```

Three rules hold it together:

1. **The project file is the single source of truth.** A project is a small JSON document: its
   kind, a name, notes, the kind's settings and the printer's settings. Toolpaths and G-code
   are always regenerated from it. Every field change, slider move or file import is an edit to
   that JSON, and the engine fills in whatever is missing from the defaults.
2. **Kinds build toolpaths; one writer writes G-code.** A project kind (the vase today; the coat
   hanger, the bend stool, the vessel… next) composes reusable toolbox steps into a toolpath:
   moves with their bead, flow and feed, and events between them. It never writes G-code. The
   one G-code writer, golden-tested against EasyGCode, turns any toolpath into text for any
   printer, with EasyGCode's start and end blocks and its safety checks.
3. **Settings windows are data.** A kind describes its settings as groups of fields (number with
   unit and range, toggle, choice, shown only when…). The app draws the window from that
   description, checks nothing itself, and shows the engine's problems and hints next to the
   fields they're about. A new setting is one line in the kind, never new HTML.

## 2. What came from where

| | From EasyGCode | From EasyCAD | New here |
|---|---|---|---|
| **Projects** | Projects as settings + JSON save/load, "keep every value" | A project library on the device, autosave | No tabs: one window per project, its kind drawn from a schema; a Projects sheet to create, open, duplicate, export, delete |
| **Engine** | All the geometry and G-code (ported, golden-tested) | Rust engine → WebAssembly in a worker, "only the newest run counts", crash recovery | Kinds + toolbox instead of one generator per tab or a recipe language |
| **G-code** | Bead model, start/end blocks, E units, F rules | One shared writer with safety checks (bed, height, heaters, sane extrusion) | Events between moves (fan, dwell, comments) instead of text pushed inline |
| **Numbers** | — | `toFixed` rounding ported, golden tests | V8's own `Math.hypot`, `sin`, `cos`, `pow` ported bit-exact, so the engine matches EasyGCode on every shape |
| **Printers** | Pellet (Klipper) and filament (Marlin) modes, per-project printer card | The Ginger G1 and Bambu P1P as data, their limits and checks (low flow, fan, max flow) | The printer card is a schema too |
| **UI** | Phone-first, dark tokens, preview coloured by feed, Download / Share / Copy | App shell (the page never scrolls), three.js view, light theme, decimal comma | Live: every change runs (no "Regenerate" button) |
| **Quality** | Regression rule: other projects' output must stay byte-identical | CI: fmt, clippy (no panics), tests, wasm build + size budget, Playwright smoke test, Pages deploy | The browser's engine is golden-tested too (`scripts/wasm.test.mjs`) |

Deliberately **left out** (they stay in EasyCAD): the recipe language and parser, the Graph tab,
the ~100 Grasshopper-style nodes, NURBS/STL, the Chat tab, the AI proxy and feature requests.

## 3. Layers

```
crates/engine   easygcode-engine   no UI, no I/O, no panics; compiles to wasm32
  lib.rs          the API: kinds, schema, printer_schema, new_project, run, gcode
  project.rs      the project file (JSON) and its checks
  schema.rs       settings windows as data; normalize = defaults + checks
  kinds/          what a project can be: vase.rs (more to come, one file each)
  toolbox/        reusable steps: shapes.rs (EasyGCode's outlines), vase.rs (vase spiral)
  toolpath.rs     moves + events + stats
  gcode.rs        the one G-code writer, start/end blocks, safety checks
  printer.rs      machines (G1, P1P) and per-project printer settings
  bead.rs, geom.rs, num.rs (JavaScript-exact numbers)
crates/wasm     easygcode-wasm     thin facade: JSON text in, JSON + typed arrays out
web/            the app: plain HTML/CSS/ES modules, no framework, no bundler
  js/app.js       wiring: run on every change, render, projects sheet
  js/engine.js    worker client (latest run wins); engine.worker.js hosts the WebAssembly
  js/form.js      draws a settings window from a schema
  js/library.js   the projects on this device (localStorage) and project files
  js/viewport.js  three.js preview: moves coloured by feed, travels faint, the bed
```

The UI talks only to the engine API. The same API is what a CLI, tests or an agent would use: every
call takes and returns plain data.

## 4. The project file

```json
{
  "app": "easygcode", "format": 2,
  "kind": "vase", "kindVersion": 1,
  "name": "Tall vase", "notes": "PETG, 5 mm nozzle",
  "settings": { "shape": { "kind": "star", "star": { "points": 7 } }, "height": 120 },
  "printer": { "model": "g1", "pellet": { "up": 195 } }
}
```

- **Partial is fine.** What a file leaves out takes its default (`schema::normalize`), so a project
  saved before a setting existed opens with it at the value that prints exactly as before: the
  EasyGCode rule "matching every print from before this setting existed", enforced by the
  engine instead of by hand.
- **Every value is kept.** All shapes' dimensions stay in the file when you switch shape, like
  EasyGCode's inputs did.
- **Kinds evolve.** `kindVersion` lets a kind carry older settings over (`Kind::migrate`) when
  it renames or restructures one.
- **Printer settings travel with the project**, so a file prints the same anywhere; a new project
  starts from the printer settings used last.
- The library adds `id`, `created` and `modified` on the device; exported files leave them out.
- EasyGCode 1's settings files (`{ "app": "easygcode", "version": 1, … }`) are recognised and
  answered with a message; importing them becomes possible kind by kind as the tabs are ported.

## 5. Kinds and the toolbox

A kind is one Rust file: a settings struct with EasyGCode's defaults, its schema, and `generate`:

```rust
fn generate(&self, settings: &Value, printer: &Printer) -> Result<Generated, String> {
    let s: Settings = typed(settings)?;              // already complete and checked
    let base = shapes::seam(&shapes::outline(&s.shape, s.tolerance), s.seam);
    let mut path = Toolpath::new();
    vase::spiral(&mut path, &vase::Spiral { base: &base, /* … */ })?;
    Ok(Generated { parts: vec![Part { path, /* header */ }], warnings, hints })
}
```

- **Toolbox steps are where EasyGCode's techniques go**, written once: the shape library, the vase
  spiral, and next the brim (with mouse ears), ring fills (staircase/zipper/spiral seams), weave
  and spike patterns, the wall hanger, bottom fillets, Catmull-Rom profiles, foaming events.
  Every tab that used one gets it for free. (In EasyGCode each tab re-implemented its own G-code
  output, and some duplicated whole editors, e.g. the container's copy of the vessel's profile.)
- **Several parts per project** are built in (`Generated.parts`): the container and its lid are one
  project with two G-code files.
- **Hints** are the live notes under a field ("= 1069 mm/min with this bead"), computed by the kind
  on every run, like EasyGCode's hints, without extra UI code.
- A kind's schema and its settings struct can't drift apart: a test checks that every field has a
  setting and every setting a field.

## 6. G-code, numbers and safety

- **One writer** (`gcode.rs`): absolute positions (G90), relative extrusion (M83), E = bead area ×
  length × flow (mm³ on the G1, mm of filament on the P1P) × multiplier; F only when the feed
  changes; EasyGCode's start and end blocks verbatim; the end lift above the tallest point, never
  past the build height.
- **Checks before writing** (also shown live in the G-code tab): every move on the bed and inside
  the build height, extrusion and feeds sane, temperatures inside the machine's limits. The
  printer also warns about flow over what it melts, the G1's low-flow inlet plug, and its fan.
- **JavaScript-exact numbers** (`num.rs`): EasyGCode runs in V8, so the engine reproduces V8's
  `toFixed`, `Math.round`, `Math.hypot`, and `Math.sin`, `Math.cos` and `Math.pow` (V8's own fdlibm
  port, checked bit for bit against Node on 45,000 inputs). Without this an ellipse already printed
  differently: a one-bit difference in a sine flipped a tie when EasyGCode simplifies the outline.
  The WebAssembly build uses the same code, so the browser writes exactly what the tests check.
- **Golden tests**: `scripts/easygcode-fixtures.cjs` runs EasyGCode's own `gcode.js` headlessly
  and stores its G-code; the engine must reproduce every command (and the start/end blocks with
  their comments). Six vase cases cover every shape, both printers, both speed modes, both tops,
  every seam side, both directions, fractional heights and no start/end blocks.

## 7. The app

- **Phone first, app shell**: the preview on top (42 % of the height) and the panel below; on a
  desktop the preview left and the panel right. The page never scrolls; the panel does.
- **Three panels**: *Settings* (the kind's window, then the project's name and notes), *Printer*
  (the printer's window), *G-code* (time, size, layers, material, flow and feed ranges, weight and
  cost; Download, Share, Copy, Peek; why a part can't be written yet).
- **Projects sheet** (tap the project's name): new project by kind, the projects on this device
  (open, duplicate, export, delete), open a file, export everything as one backup file.
- **Live**: every change runs the engine in the worker; a newer change replaces a waiting run, so
  typing never queues up work. Problems mark their field in red and nothing prints until they're
  fixed; warnings show above the settings.
- **Offline**: a PWA with EasyGCode's network-first service worker (nothing to bump per release).

## 8. Adding a kind (porting an EasyGCode tab)

1. Run EasyGCode's generator for the tab headlessly over a handful of settings that cover its
   features (extend `scripts/easygcode-fixtures.cjs`), and store the fixtures.
2. Move the tab's techniques into toolbox steps (or reuse existing ones); write the kind as a
   settings struct + schema + `generate`.
3. Add the cases to the golden tests (native and `scripts/wasm.test.mjs`) until every command
   matches. Where EasyGCode has a bug worth fixing, fix it deliberately: keep the fixture of the
   old behaviour out and say so in the dev log.
4. The settings window, saving, the preview, G-code, sharing: nothing to do.

## 9. Budgets and rules

- No `unwrap`/`expect`/`panic!` in non-test Rust (clippy), no `unsafe`; bad input is a message.
- Everything in `crates/` builds for `wasm32-unknown-unknown`; the engine stays under 1 MB
  gzipped (it's 132 KB).
- A change runs in milliseconds (the default vase: 6,600 moves in about 15 ms in the browser).
- Dependencies: permissive licenses only, listed in `THIRD-PARTY-NOTICES.md`.
