# EasyGCode golden fixtures

G-code written by EasyGCode itself, for the golden tests (`crates/engine/src/golden_tests.rs`,
`scripts/wasm.test.mjs`, and the browser smoke test for `vase-circle`).

- `<case>.json`: EasyGCode's own settings for the case (its coat hanger, every extra off).
- `<case>.project.json`: the same settings as an EasyGCode 2 project (the vase kind).
- `<case>.gcode`: what EasyGCode's own generator (`gcode.js`) writes for them, minus the timestamp.
- `SOURCE`: the EasyGCode commit they came from.

| Case | What it covers |
|---|---|
| `vase-circle` | EasyGCode's coat hanger defaults (30 mm circle, G1), 10 revolutions, set feed |
| `vase-rect-filament` | The P1P (filament start/end, E in mm, multiplier, fan), flow mode, rounded rectangle, clockwise, seam at the front, open top after 8.5 revolutions |
| `vase-star-bare` | No start/end blocks, star, seam on +X, coarser outline, 3.75 revolutions |
| `vase-ellipse-fan` | Pellet temperatures, purge, pressure advance 0, the fan after the first turn, seam on −X (the case that needed V8's sine) |
| `vase-polygon` | Straight sides: the ramp's extra points |
| `vase-squircle` | `Math.pow`, clockwise |

Regenerate them from a clone of EasyGCode (which is only read, never changed):

```sh
node scripts/easygcode-fixtures.cjs ../easygcode
```

The tests compare commands, not comments (plus the start and end blocks verbatim), so EasyGCode 2's
own header comments may differ.
