# ezgc: roadmap

**Rule** (as in EasyCAD): every step ends with something usable on the phone. ezgc is a new app;
EasyGCode (the original, still its own project) is where ideas and tested prints come from, not a
spec to match.

---

## Decisions (answered 22 Oct 2026)

| # | Question | Decision |
|---|---|---|
| 1 | How many kinds? | **One base, everything grows from it: the spiral.** Almost every project is one continuous spiral, no travel, a single wall and a single bottom thickness: a cylinder in vase mode whose extrusion ramps up over the first turn. The container is the same spiral with another body, layers that follow the overhang, and a flat bottom turning into the wall through a fillet; the spiral disc is a bottom that never turns into a wall, with its spacing and bumps; the coat hanger is the spiral with patterns; the lampshade a cylinder with another shape on top. Rules and features are applied to the spiral. (Agreed, with two caveats below.) |
| 2 | The reference "Vase" kind | Answered by 1: it *is* the base, renamed **Spiral**. |
| 3 | Printer settings | **Printers are profiles on the device, apart from projects.** One project can be made for different printers, and a project sent to someone prints on their own printer, one they make or one ezgc ships. |
| 4 | Speed | **Always set as volumetric flow (mm³/s)**, with the feed it gives shown right next to it to check. |
| 5 | Testing against EasyGCode | **No.** EasyGCode was a work in progress; ezgc is something new. No tests against EasyGCode unless the owner asks for one. (ezgc's own G-code is still snapshot-tested, so every change to it is one somebody meant.) |
| 6 | Importing EasyGCode's settings files | **No.** New projects are made new; a translator may come later, on request. |
| 7 | Hosting and name | The new app is **ezgc**; **EasyGCode stays its own project**. ezgc runs on **GitHub Pages from `main`**; work happens on a branch and reaches `main` through pull requests. |
| 8 | License | **All rights reserved.** |

### The two caveats on "everything is one spiral"

1. **Not everything is a spiral.** The bend stool is concentric rings with seams (staircase or
   zipper), legs as hairpins, stacked layers with a lift and travel between them, and foaming with
   pauses and prime lines. Forcing that into the spiral would bend the spiral out of shape, so it
   becomes a kind of its own when its turn comes (the engine already has room for kinds). The
   spoon (a flat spiral with a stick) and the thread (a spiral whose layer is the thread pitch) do
   fit the spiral.
2. **One kind means one big settings window.** Every feature therefore joins as a group that only
   opens when it's switched on (Bottom: ramp / flat disc / flat disc into a fillet; Pattern: none /
   weave / spikes; …), and **templates** give quick starting points ("Coat hanger", "Container",
   "Spiral disc": saved spiral projects, not separate code).

---

## Step 0: the base ✓ *21 Oct 2026*

Rust engine → WebAssembly in a worker; project files as JSON; settings windows drawn from schemas;
a library on the device; one G-code writer with EasyGCode's start and end blocks; the Ginger G1 and
the Bambu P1P. CI with a browser smoke test, GitHub Pages deploy. (See [devlog.md](devlog.md).)

## Step 1: the decisions ✓ *22 Oct 2026*

ezgc as the name; the **Spiral** as the one base; **printer profiles** on the device (made from the
G1 or the P1P, duplicated, renamed, exported, opened from a file); **speed as flow** with the feed
beside it; ezgc's own **snapshot tests** instead of EasyGCode's; first pull request into `main`.

## Step 2: the salt & pepper shaker ✓ *22 Oct 2026*

EasyGCode's container with the spiral-disc lid, grown onto the spiral as settings that are off by
default (every earlier project prints exactly as before; the snapshots prove it), and a template:

- **Body profile**: the shape's size along the height, a smooth curve through points (height %,
  scale %) that leaves and meets them level; edited as a list of points.
- **Flat bottom**: flat spiral layers (rings one line apart linked into a spiral from the centre
  out), turning into the wall through a quarter-circle fillet; the bead eases from the flat spiral's
  into the wall's up the fillet.
- **Flat spirals** (the bottom's and the lid's): their own layer height, line width and extrusion,
  and several layers stacked, out, back in, out… (an odd count, so the last ends where the wall
  starts), one line with no travel; the fillet and wall start on top of them.
- **Extrusion** (wall and flat spirals): more plastic per move *at the same flow*, the move slows to
  match.
- **Closer layers on overhangs**, 0–250 %: 100 % climbs `layer × cos(lean)` per turn, above that
  even less (`× cos^(pace − 1)`, so 250 % is `layer × cos^2.5`); straight walls keep full layers.
- **Lid** (round shapes): a second part, printed upside down: a flat spiral whose turns spread
  apart around a ring and interlock (the holes), a fillet, a skirt; the container gets a neck as
  tall; both turn the same way so the lid screws on.
- **Templates** in the New project sheet: "Salt & pepper shaker".

The three changes from the owner's EasyCAD chat are in: extrusion that keeps the flow, stacked flat
spiral layers (default 3, odd), and closer layers on overhangs above 100 %.

## EasyGCode's projects in ezgc (the owner's list, 22 Oct 2026)

**Every EasyGCode project comes to ezgc eventually, except the spoon and the thread**, one or two at
a time, as features of the spiral (or, for the bend stool, a kind of its own) plus a template.

| Order | Project | Status | What it needs |
|---|---|---|---|
| 1 | **Container** as the salt & pepper shaker (container + spiral-disc lid with holes) | ✓ 22 Oct | Step 2 above |
| 2 | **Lampshade**, with all its shades (straight cone, bell, dome, sphere, flat sphere/UFO), throat down or wide edge down | **next** | Shade profiles; the throat with its socket thread (E27, E14, custom); overhang compensation (wider bead or slower). **Note:** the shades also need *closer layers on overhangs above 100 %* (even tighter than `cos(lean)`); that setting already goes up to 250 % (built with the shaker), so the lampshade uses it |
| 3 | **Coat hanger** | | Wall patterns (weave, spikes) with their speeds and the fan during bumps; the wall-hanger cut-out and pocket |
| 4 | **Bend stool** | | Its own kind (not a spiral): concentric rings with seams, legs, stacked layers with travel, foaming |
| 5 | **Vessel** | eventually | Mostly there (flat bottom, profile, overhang layers); bottom shapes, the star top (morph), randomize |
| 6 | **Spiral disc** | eventually | The lid's flat spiral with holes on its own (no wall), its attractor and bumps settings |
| ✗ | Spoon, thread | not wanted | |

Still open from the earlier list: a shaped top (morph) for the lampshade and vessel, wall patterns
and the hanger for the coat hanger; a layer slider in the preview, a curve you drag for the profile,
G1 material presets for printer profiles (Ginger Slicer's), a brim.

## Later / when needed
- The bend stool as its own kind.
- Custom start and end G-code for printers beyond the G1 and the P1P (each needs a hand-checked
  fixture and the owner's OK: G-code drives real machines).
- Sending to the printer (Moonraker on the G1), which needs a small desktop wrapper.
- A translator from EasyGCode's settings files, on request.
- The agent (Claude with your key) editing settings, or Claude Code adding features from a
  description: everything is already plain data and one API.
