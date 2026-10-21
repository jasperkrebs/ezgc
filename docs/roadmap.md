# EasyGCode 2: roadmap

**Rule** (as in EasyCAD): every step ends with something usable on the phone, and every ported
EasyGCode project prints exactly as it did, proven by golden tests against EasyGCode itself.

---

## Step 0: the base ✓ *21 Oct 2026*

The architecture with one reference kind to prove it end to end (see [devlog.md](devlog.md)):

- Rust engine → WebAssembly in a worker; project files as JSON; settings windows drawn from
  schemas; a project library on the device; one golden-tested G-code writer; the Ginger G1 and the
  Bambu P1P.
- **Vase**: EasyGCode's coat hanger without its extras (six shapes, vase spiral with a ramped
  first turn, flat or open top, seam side, direction, flow or feed). Its G-code matches
  EasyGCode's coat hanger command for command, natively and in the browser.
- CI: fmt, clippy, tests, wasm build and size budget, browser smoke test on phone and desktop,
  GitHub Pages deploy.

## Next: EasyGCode's projects, one at a time

Each one: fixtures from EasyGCode → toolbox steps → a kind → golden tests. A suggested order
(EasyGCode's tab order, and what each one teaches the toolbox):

| # | Project | New toolbox steps it brings | Size |
|---|---|---|---|
| 1 | **Coat hanger** (grows out of the vase) | brim (normal, mouse ears, inner), weave and spike patterns with their speeds and fan modes, wall hanger (single, double, tweens, bridge/overhang speeds), bottom fillet with true offsets, acceleration to the wall feed | L |
| 2 | **Spiral disc** | attractor spiral, `fade`, odd-bump interlock, stacked layers | S |
| 3 | **Vessel** | radius profile (Catmull-Rom), top and bottom curves (points editor), solid bottoms (ring fills), filleted bottom into the wall | M |
| 4 | **Lampshade** | lamp profiles (cone, bell, dome, spheres), socket threads (E14/E27), overhang compensation | M |
| 5 | **Thread** | ISO metric helix, flank clearance | S |
| 6 | **Spoon** | spoon spiral and stick | S |
| 7 | **Bend stool** | concentric rings with staircase/zipper seams, legs as hairpins, bend-zone spread, dome, foaming (machine events) | L |
| 8 | **Container** (salt & pepper) | two parts in one project (already supported), lid holes | M |

Along the way, as the projects need them:
- **New field types** for the settings window: a points/curve editor (the vessel's profile and
  curves; EasyGCode's drag-editable canvases), and 2D overlays in the preview (outline, seam).
- **Presets per project** ("small", "tall", "test print") and **templates**: save a project as a
  starting point for new ones.
- **Open EasyGCode 1 settings files**, per ported tab: its saved inputs become a project.
- **G1 material presets** (Ginger Slicer's profiles, already in EasyCAD's `print.js`).
- A layer slider in the preview, colour by height or by layer.

## Later / when needed
- Sending to the printer (Moonraker on the G1), which needs a small desktop wrapper.
- The agent (Claude with your key) editing project settings, or Claude Code writing new kinds from
  a description: everything is already plain data and one API.
- A command-line tool on the same engine (batch G-code, scripted tests).

---

## Open questions (21 Oct 2026)

The base makes a default choice for each; say if you want it otherwise.

1. **How many kinds?** One kind per EasyGCode tab (eight), or fewer, broader kinds where the
   projects share a structure? *Default:* the coat hanger, vessel and lampshade are all a shape
   spun into a vase spiral, so they could be one "Vase" kind with options (profile, patterns,
   hanger, lamp socket) while the disc-like ones (spiral disc, bend stool, spoon) and the thread
   stay their own. Fewer kinds = more reuse, but bigger settings windows.
2. **The reference "Vase" kind**: keep it, grow it into the coat hanger (patterns, hanger and brim
   as options that are off by default), or retire it once the coat hanger is ported?
   *Default:* grow it into the coat hanger.
3. **Printer settings: per project or per device?** *Default:* per project (the file prints the
   same anywhere, as EasyGCode's tabs each had their own), with new projects starting from the
   last used. The alternative: printers as device profiles that projects only name, so a bed
   temperature changes once for everything.
4. **Speed**: EasyCAD decided "always volumetric flow"; EasyGCode offers feed or flow per project.
   *Default:* both, flow selected (20 mm³/s, EasyGCode's flow default).
5. **Golden tests for every port**: byte-identical to EasyGCode is slow work for the big tabs (the
   coat hanger's generator is 3,000 lines). Keep it strict, or accept "same toolpath within
   0.001 mm" where exactness costs too much? And where EasyGCode has a quirk or a bug: copy it, or
   fix it and note it? *Default:* strict, and quirks copied unless you say otherwise.
6. **Old settings files**: worth importing EasyGCode 1's saved JSON files into new projects as
   each tab is ported?
7. **Hosting and name**: GitHub Pages from this repository (Pages needs it public, or a paid plan)?
   Is the app called "EasyGCode" (it is now; the G-code header says "EasyGCode 2"), and does the
   original EasyGCode keep running next to it until everything is ported?
8. **License**: all rights reserved, like EasyCAD (the files say so for now)?
