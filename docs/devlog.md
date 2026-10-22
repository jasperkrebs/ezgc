# Dev log

Newest first. Each entry: what landed, numbers, what's still open. Keep it short; it's how the
next session (human or agent) picks up.

## 2026-10-22: the salt & pepper shaker

**Why:** the owner wants every EasyGCode project in ezgc except the spoon and the thread, one or
two at a time, starting with the container as the salt & pepper shaker (with the lid from their
EasyCAD project and the three changes from its chat). The list and order are in
[roadmap.md](roadmap.md#easygcodes-projects-in-ezgc-the-owners-list-22-oct-2026).

**Landed** (all on the Spiral, off by default: the four earlier snapshots are byte-identical)
- **Body profile** (`toolbox::profile`): a monotone cubic through (height %, scale %) points, level
  at both ends (EasyCAD's smooth `graph`). The settings window has a new field type for it, a list
  of points (`Input::Points`; rows of two numbers, add and remove).
- **Shaped walls** (`toolbox::spiral::Shaped`): the wall as the outline scaled along the height,
  turns starting at given heights, the bead set by height; the plain spiral is untouched.
- **Paced layers** (`toolbox::layers::paced`): each turn `layer · cos^(pace−1) · (1 − min(pace,1) ·
  (1 − cos))`, cos of the lean where the wall leans most (no less than 0.05), stepped by its middle
  and fitted to end on the top: 0–250 %.
- **Flat spirals** (`toolbox::disc`): the ring fill linked as a spiral (EasyCAD's `rings` +
  `link_rings`), the lid's hole spiral (RK4 at 720 steps a turn, the fade around a ring, the
  interlock solve, the stretch after the wiggles), and `stack`: odd layers out, in, out as one line.
- **Flat bottom, fillet, lid**: the fillet a true quarter circle; bead easing (smoothstep) from the
  flat spiral's into the wall's; the lid a second part (container + lid → two G-code files), the
  container's neck as tall as the skirt, the lid one line width (+ fit) outside it.
- **Extrusion that keeps the flow**: wall and flat spirals, more plastic per move and the feed
  divided by the same factor (the EasyCAD chat's first fix).
- **Templates**: `templates()` in the engine API; "Salt & pepper shaker" in the New project sheet.
- **Preview**: parts stand side by side (each still prints on the bed's centre).
- **Snapshots** can have several parts (`<case>.gcode`, `<case>.lid.gcode`); a small shaker on the
  P1P covers every new setting (extrusion 110 %/125 %, 150 % pace, profile, fillet, lid, holes).

**Numbers:** 57 engine tests (5 snapshot cases, 6 files), 9 wasm tests, 8 Node tests; engine 160 KB
gzipped. The template (two parts, about 33,000 moves) runs in about 10 ms in Chromium.

**Open**
- At 250 % the fillet's foot, nearly flat, takes about 140 turns (EasyCAD's lofted fillet about
  110; 22 at 100 %): fine where walls lean moderately (the lampshade), heavy for a fillet.
- The lid needs the circle shape (it screws on); other shapes get a warning and no lid.
- Next: the lampshade with all its shades ([roadmap.md](roadmap.md)).

## 2026-10-22: the owner's decisions: ezgc, one spiral, printer profiles

**Why:** the owner answered the open questions ([roadmap.md](roadmap.md#decisions-answered-22-oct-2026)):
everything grows from one spiral; printers are profiles on the device, apart from projects; speed is
always volumetric flow; no testing against EasyGCode; the new app is called ezgc; work reaches
`main` (and GitHub Pages) through pull requests.

**Landed**
- **ezgc**: the app, its files (`"app": "ezgc"`, format 1), storage keys and crates
  (`ezgc-engine`, `ezgc-wasm`) are renamed. EasyGCode's files are now "another app's" (no import).
- **The Spiral** is the one kind (the vase, renamed): settings regrouped as Shape, Size, Bead and
  flow, Top, Path, ready for features to join as groups that are off by default. The toolbox step
  is `spiral::wall`.
- **Flow only**: the speed is `flow` (mm³/s); the feed it gives shows under the field.
- **Printer profiles**: a project no longer holds a printer. Profiles live on the device, based on
  the G1 or the P1P (start/end G-code and heater limits), each with its own bed (new), temperatures,
  placement, travel and material estimate. The Printer tab chooses, makes, duplicates, renames,
  exports and deletes them; files and backups carry them; the first start makes one of each.
  Engine API: `run(project, printer)`, `gcode(project, printer, part)`, `new_printer(model)`.
- **Snapshots instead of EasyGCode's goldens**: `fixtures/snapshots/` holds four projects with
  printers and the G-code ezgc writes for them; the native and the browser's build must match them
  (`UPDATE_SNAPSHOTS=1` rewrites them after a meant change). EasyGCode's fixtures and the script that
  made them are gone.
- Fixed: a flow set to exactly the printer's limit (20 mm³/s on the P1P) warned, from float noise.
- CI runs the newest stable Rust; its clippy (1.99) flagged a late initialisation in the ported
  `rem_pio2`, fixed.

**Numbers:** 42 engine tests (4 snapshots), 8 wasm tests, 8 Node tests; engine 132 KB gzipped.

**Open**
- Growing the spiral: the flat bottom turning into the wall is next ([roadmap.md](roadmap.md)).
- GitHub Pages needs switching on (and, while the repository is private, GitHub Pro).

## 2026-10-21: the base

**Why:** EasyCAD (an experiment merging CAD, AI and Grasshopper) went in many directions; the owner
wants EasyGCode itself, streamlined, on EasyCAD's architecture: no tabs per project, JSON project
files, the Rust engine. Design in [architecture.md](architecture.md), plan and questions in
[roadmap.md](roadmap.md).

**Landed**
- **Engine** (`crates/engine`): project files (JSON, partial is fine, defaults fill in), settings
  schemas with checks, kinds + toolbox, toolpaths as data (moves, events, stats), one G-code
  writer (EasyGCode's start/end blocks, bead model, safety checks), the Ginger G1 and Bambu P1P.
- **Vase**, the reference kind: EasyGCode's coat hanger with its extras off. EasyGCode's shape
  library (`geometry.js`: six shapes, chord-tolerance simplification, seam, arc-length sampler)
  and its vase spiral (ramped first turn, flat or open top) ported line for line.
- **JavaScript-exact numbers**: `toFixed`, `Math.round`, V8's `Math.hypot` (Kahan sum), and V8's
  fdlibm `sin`, `cos` and `pow`. The `libm` crate's sine differs from V8's in the last bit in about
  2 % of calls; on the ellipse that flipped a tie in EasyGCode's outline simplification and the
  toolpath came out different. With the port: 0 differences in 23,220 angles and 22,000 powers.
- **Golden tests**: six cases from EasyGCode's own `gcode.js` (`scripts/easygcode-fixtures.cjs`),
  every command identical, natively (`cargo test`) and in the browser build
  (`scripts/wasm.test.mjs`).
- **App** (`web/`): app shell, schema-drawn settings and printer windows, live runs in a worker,
  three.js preview coloured by feed, G-code tab (stats, Download/Share/Copy/Peek, why a part
  can't be written), projects sheet (new, open, duplicate, export, delete, open a file, export
  all), autosave with a backup of each project's previous version, offline PWA.
- **CI**: fmt, clippy (no panics), tests, wasm check, notices check, wasm build with a 1 MB
  budget, Node tests, Playwright smoke test on phone and desktop (the downloaded G-code must equal
  EasyGCode's), GitHub Pages deploy.

**Numbers:** 40 engine tests (6 golden cases), 9 wasm tests, 7 Node tests; engine 132 KB
gzipped; the default vase (100 revolutions, about 6,600 moves) runs in about 15 ms in Chromium.

**Open**
- The questions in [roadmap.md](roadmap.md#open-questions-21-oct-2026).
- No EasyGCode tab is ported yet (by design): the coat hanger is next.
- The preview draws every move as a line; a layer slider and bead-width lines come later.
