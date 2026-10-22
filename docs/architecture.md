# ezgc: architecture

*Written 21 Oct 2026 for the first build, updated 22 Oct with the owner's decisions
([roadmap.md](roadmap.md#decisions-answered-22-oct-2026)). EasyGCode is the original idea (a
phone-first app that writes custom G-code), EasyCAD the experiment that grew out of it (Rust,
recipes, Grasshopper, an agent). ezgc is the streamlined merge: EasyGCode's way of working, on
EasyCAD's engine and app shell, and nothing else.*

---

## 1. The idea in one picture

```
  project file (JSON)          printer profile (JSON)
  ┌────────────────────┐       ┌────────────────────┐      Rust engine (WebAssembly, in a Web Worker)
  │ kind: "spiral"     │       │ based on: g1 / p1p │ run ┌───────────────────────────────────────────┐
  │ name, notes        │──┐ ┌──│ bed, temperatures, │────▶│ schema: fill in defaults, check settings  │
  │ settings: { … }    │  │ │  │ placement, material│     │ kind:   toolbox steps → toolpath(s)       │
  └────────────────────┘  └─┴──┴────────────────────┘◀────│ stats, warnings, hints, preview arrays    │
        ▲                          ▲              complete │ gcode:  one writer, any kind, any printer │
        │                          │              settings └───────────────────────────────────────────┘
  settings window             Printer tab                              │
  (drawn from the schema)     (profiles on the device)     3D preview (three.js) · G-code file
```

Four rules hold it together:

1. **The project file is the single source of truth.** A project is a small JSON document: its
   kind, a name, notes and the kind's settings. Toolpaths and G-code are always regenerated from
   it, for the printer chosen on the device. The engine fills in whatever a file leaves out.
2. **Everything grows from one spiral.** Almost every project is one continuous spiral, no
   travel: a shape printed like a vase whose extrusion ramps up over the first turn. Features join
   the **Spiral** kind as settings that are off by default (§5). Only a project that isn't a spiral
   at all (the bend stool) becomes a kind of its own.
3. **Kinds build toolpaths; one writer writes G-code.** A kind composes reusable toolbox steps
   into a toolpath: moves with their bead, flow and feed, and events between them. It never writes
   G-code. The one writer turns any toolpath into text for any printer, with EasyGCode's proven
   start and end blocks and its safety checks.
4. **Settings windows are data.** A kind (and the printer) describes its settings as groups of
   fields (number with unit and range, toggle, choice, shown only when…). The app draws the window
   from that, checks nothing itself, and shows the engine's problems and hints next to the fields
   they're about. A new setting is one line in the kind, never new HTML.

## 2. What came from where

| | From EasyGCode | From EasyCAD | New here |
|---|---|---|---|
| **Projects** | Projects as settings + JSON files, "keep every value" | A library on the device, autosave | No tabs: one window per project, drawn from a schema; one spiral instead of a tab per project |
| **Printers** | Pellet (Klipper) and filament (Marlin) modes, start/end blocks | The Ginger G1 and Bambu P1P as data, their limits and checks (low flow, fan, max flow) | Printer profiles on the device, apart from projects, with their own bed |
| **Engine** | The geometry (shape library, vase spiral), ported | Rust engine → WebAssembly in a worker, "only the newest run counts", crash recovery | Kinds + toolbox instead of one generator per tab or a recipe language |
| **G-code** | Bead model, start/end blocks, E units, F rules | One shared writer with safety checks (bed, height, heaters, sane extrusion) | Events between moves (fan, dwell, comments) instead of text pushed inline |
| **Speed** | Feed or flow per project | "Always volumetric flow" | Flow always, the feed it gives shown next to it |
| **UI** | Phone-first, dark tokens, preview coloured by feed, Download / Share / Copy | App shell (the page never scrolls), three.js view, light theme, decimal comma | Live: every change runs (no "Regenerate" button) |
| **Quality** | — | CI: fmt, clippy (no panics), tests, wasm build + size budget, Playwright smoke test, Pages deploy | Snapshots of ezgc's own G-code, checked natively and in the browser's build |

Deliberately **left out** (they stay in EasyCAD): the recipe language and parser, the Graph tab,
the ~100 Grasshopper-style nodes, NURBS/STL, the Chat tab, the AI proxy and feature requests.

## 3. Layers

```
crates/engine   ezgc-engine   no UI, no I/O, no panics; compiles to wasm32
  lib.rs          the API: kinds, schema, printer_schema, new_project, new_printer, run, gcode
  project.rs      the project file (JSON) and its checks
  schema.rs       settings windows as data; normalize = defaults + checks
  kinds/          what a project can be: spiral.rs (the base, and its features)
  toolbox/        reusable steps: shapes.rs (outlines, seam, sampler), spiral.rs (the wall, plain or
                  shaped), disc.rs (flat spirals: ring fill, hole spiral, stacked layers),
                  layers.rs (closer layers on overhangs), profile.rs (smooth curve, fades)
  toolpath.rs     moves + events + stats
  gcode.rs        the one G-code writer, start/end blocks, safety checks
  printer.rs      machines (G1, P1P) and printer profiles
  bead.rs, geom.rs, num.rs (V8-exact numbers)
crates/wasm     ezgc-wasm     thin facade: JSON text in, JSON + typed arrays out
web/            the app: plain HTML/CSS/ES modules, no framework, no bundler
  js/app.js       wiring: run on every change, render, projects sheet, printer tab
  js/engine.js    worker client (latest run wins); engine.worker.js hosts the WebAssembly
  js/form.js      draws a settings window from a schema
  js/library.js   projects and printers on this device (localStorage), and their files
  js/viewport.js  three.js preview: moves coloured by feed, travels faint, the bed
```

The UI talks only to the engine API. The same API is what a CLI, tests or an agent would use: every
call takes and returns plain data.

## 4. Files

A project:

```json
{
  "app": "ezgc", "format": 1,
  "kind": "spiral", "kindVersion": 1,
  "name": "Tall vase", "notes": "PETG, 5 mm nozzle",
  "settings": { "shape": { "kind": "star", "star": { "points": 7 } }, "height": 120, "flow": 30 }
}
```

A printer profile:

```json
{
  "app": "ezgc", "format": 1, "type": "printer",
  "name": "Ginger G1, 5 mm nozzle",
  "settings": { "model": "g1", "pellet": { "up": 195, "mid": 210, "down": 210 } }
}
```

- **Partial is fine.** What a file leaves out takes its default (`schema::normalize`; for a
  printer, its machine's defaults, bed included), so a project saved before a setting existed
  opens with it at the value that prints exactly as before.
- **Every value is kept.** All shapes' dimensions stay in the file when you switch shape.
- **Kinds evolve.** `kindVersion` lets a kind carry older settings over (`Kind::migrate`) when it
  renames or restructures one.
- **Projects and printers are apart.** A project prints on whichever printer is chosen on the
  device; sent to someone, it prints on theirs. A printer profile is based on a machine (the G1 or
  the P1P: its start and end G-code and heater limits) and has its own bed, temperatures,
  placement, travel and material estimate.
- The library adds `id`, `created` and `modified` on the device; exported files leave them out.
  "Export all" writes `{ "app": "ezgc", "projects": […], "printers": […] }`.

## 5. The spiral, and how it grows

Today the Spiral is: an outline (circle, rounded rectangle, ellipse, polygon, star, squircle;
simplified to a chord tolerance, starting on a chosen side, either direction), walked by arc
length with Z rising one layer per turn; the first turn ramps the extrusion from 0 to a full bead;
the top is a level rim (one more turn tapering to nothing) or open; speed is a volumetric flow.
Switched on, it also has a body profile, a flat bottom of stacked flat spiral layers turning into
the wall through a fillet, closer layers on overhangs, an extrusion factor that keeps the flow, and
a lid with holes as a second part: the salt & pepper shaker, one tap away as a template.

Each feature from EasyGCode's projects joins it as a **group of settings that is off by
default**, built from **toolbox steps** (written once, reusable):

| Feature | Makes | Toolbox step | |
|---|---|---|---|
| Flat bottom: flat spiral layers, turning into the wall through a fillet | Container, vessel | `disc::ring_spiral`, `disc::stack`, `spiral::Shaped` | ✓ |
| Body profile along the height | Vessel, container, lampshade | `profile::Graph` | ✓ |
| Layers that follow the overhang (0–250 %) | Container, vessel, lampshade | `layers::paced` | ✓ |
| Disc spacing near a ring, interlocking bumps | Shaker lid; the spiral disc | `profile::fade`, `disc::hole_spiral`, `disc::interlock` | ✓ |
| Several spirals in one project | Container + lid | parts (`Generated`) | ✓ |
| A flat spiral alone (no wall) | Spiral disc | | |
| Wall patterns (weave, spikes), the hanger cut-out | Coat hanger | patterns, hanger | |
| Shaped top (morph, lamp throat and thread) | Lampshade, vessel's star top | morph | |

The settings window stays manageable because a group only shows its settings when it's switched
on, and **templates** (a kind's `templates()`: settings that differ from its defaults, a name and
notes) give one-tap starting points in the New project sheet ("Salt & pepper shaker", …).

A wall that isn't a straight prism is a `spiral::Shaped`: at each height the outline scaled (the
profile, a fillet), the turns starting at given heights (`layers::paced`) and the bead set by height
(easing from a flat bottom's into the wall's). Each turn blends one height's outline into the
next's, so it's still one continuous line. A lid or anything printed as its own piece is another
`Part` of the same project, with its own G-code file.

**Not a spiral:** the bend stool (concentric rings with seams, legs, stacked layers with travel,
foaming). It becomes its own kind when its turn comes; the kind registry is there for it.

## 6. G-code, numbers and safety

- **One writer** (`gcode.rs`): absolute positions (G90), relative extrusion (M83), E = bead area ×
  length × flow (mm³ on the G1, mm of filament on the P1P) × multiplier; F only when the feed
  changes; EasyGCode's start and end blocks verbatim; the end lift above the tallest point, never
  past the build height.
- **Checks before writing** (also shown live in the G-code tab): every move on the bed and inside
  the build height, extrusion and feeds sane, temperatures inside the machine's limits. The printer
  also warns about flow over what it melts, the G1's low-flow inlet plug, and its fan.
- **Snapshots** (`fixtures/snapshots/`): projects + printers and the G-code ezgc writes for them.
  The tests fail on any change, natively and in the browser's build; after a change that's meant,
  `UPDATE_SNAPSHOTS=1 cargo test -p ezgc-engine snapshots` rewrites them, and the diff gets reviewed.
- **Same bits everywhere** (`num.rs`): distances and trigonometry use V8's own `Math.hypot`, `sin`,
  `cos` and `pow` (fdlibm, ported and checked bit for bit against Node on 45,000 inputs), and
  `toFixed` rounding. The native tests and the browser compute exactly the same numbers, and the
  geometry matches EasyGCode's should the owner ever want a comparison.

## 7. The app

- **Phone first, app shell**: the preview on top (42 % of the height) and the panel below; on a
  desktop the preview left and the panel right. The page never scrolls; the panel does.
- **Three panels**: *Settings* (the kind's window, then the project's name and notes), *Printer*
  (choose, make, duplicate, rename, export or delete a printer, and its settings), *G-code* (time,
  size, layers, material, flow and feed, weight and cost; Download, Share, Copy, Peek; why a part
  can't be written yet).
- **Projects sheet** (tap the project's name): new project, the projects on this device (open,
  duplicate, export, delete), open a file (projects, printers or a backup), export everything.
- **Live**: every change runs the engine in the worker; a newer change replaces a waiting run, so
  typing never queues up work. Problems mark their field in red and nothing prints until they're
  fixed; warnings show above the settings.
- **Offline**: a PWA with EasyGCode's network-first service worker (nothing to bump per release).

## 8. Adding a feature to the spiral

1. Write the technique as a toolbox step (or extend one), with unit tests of its geometry
   (continuity, no travel, layer heights, bounds).
2. Add its settings to the Spiral as a group that is off by default, so every existing project
   prints exactly as before (the snapshots prove it), and add a snapshot case that turns it on.
3. The settings window, saving, the preview, G-code, sharing: nothing to do.

## 9. Budgets and rules

- No `unwrap`/`expect`/`panic!` in non-test Rust (clippy), no `unsafe`; bad input is a message.
- Everything in `crates/` builds for `wasm32-unknown-unknown`; the engine stays under 1 MB
  gzipped (it's 132 KB).
- A change runs in milliseconds (the default spiral: 6,600 moves in about 15 ms in the browser).
- Dependencies: permissive licenses only, listed in `THIRD-PARTY-NOTICES.md`.
