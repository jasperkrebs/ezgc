# Dev log

Newest first. Each entry: what landed, numbers, what's still open. Keep it short; it's how the
next session (human or agent) picks up.

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
