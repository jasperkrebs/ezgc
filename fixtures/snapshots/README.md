# Snapshots

ezgc's own G-code, for the snapshot tests (`crates/engine/src/snapshot_tests.rs`, and
`scripts/wasm.test.mjs` for the browser's build of the engine).

- `<case>.project.json`: a project (partial is fine: the rest takes its defaults).
- `<case>.printer.json`: a printer profile's settings (likewise).
- `<case>.gcode`: what ezgc writes for them (the first part; a project of several parts writes each
  other one to `<case>.<part>.gcode`, `salt-shaker.lid.gcode`).

G-code drives real machines, so the tests fail when any of it changes. After a change you meant,
rewrite the snapshots, then read the diff before committing:

```sh
UPDATE_SNAPSHOTS=1 cargo test -p ezgc-engine snapshots
git diff fixtures/snapshots
```

| Case | What it covers |
|---|---|
| `circle-g1` | The spiral's defaults, 6 mm tall, on the Ginger G1 (pellets, Klipper start/end) |
| `rect-p1p` | The Bambu P1P (filament start/end, E in mm, multiplier, fan), a rounded rectangle, clockwise from the front, an open top after 8.5 revolutions |
| `star-bare` | No start/end blocks, a star, starting on +X, a coarser outline, 3.75 revolutions |
| `ellipse-fan` | Pellet temperatures, purge, pressure advance 0, the fan coming on after the first turn, starting on −X |
| `salt-shaker` | A small salt & pepper shaker on the P1P: body profile, 3 flat spiral layers of their own height at 125 % extrusion, the fillet, the wall at 110 % extrusion with closer layers on overhangs at 150 %, a flat rim; the lid (`salt-shaker.lid.gcode`) with its holes, 7 bumps, a fit of 0.1 mm |
