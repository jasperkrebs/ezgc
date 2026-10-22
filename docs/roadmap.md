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

## Next: growing the spiral

Each a group of settings on the spiral, off by default, so earlier projects print as before. A
suggested order (most reuse first):

| # | Feature | What it makes possible |
|---|---|---|
| 1 | **Flat bottom**: a spiral disc from the centre out at one layer height, turning into the wall through a fillet | Container, vessel; with "no wall", the spiral disc |
| 2 | **Body profile**: the outline scaled along the height (points through a smooth curve, edited by dragging) | Vessel, container, lampshade bodies |
| 3 | **Layers that follow the overhang**: closer layers where the wall leans out | Container, vessel, lampshade |
| 4 | **Disc spacing and bumps**: spacing that spreads near a ring (attractor), odd bumps that interlock between turns | Spiral disc, the shaker's lid |
| 5 | **Wall patterns**: weave and spikes, with their speeds and the fan during bumps | Coat hanger |
| 6 | **Wall hanger**: the coat hanger's cut-out and pocket | Coat hanger |
| 7 | **Shaped top**: another shape at the top (morph), lamp throat and socket thread | Lampshade, vessel's star top |
| 8 | **Parts**: more than one spiral in a project, each its own G-code | Container + lid |
| 9 | **Templates**: starting points in the New project sheet | All of the above, one tap away |

And along the way: a points/curve editor field for the settings window (for the profile), a layer
slider in the preview, G1 material presets for printer profiles (Ginger Slicer's), a brim.

## Later / when needed
- The bend stool as its own kind.
- Custom start and end G-code for printers beyond the G1 and the P1P (each needs a hand-checked
  fixture and the owner's OK: G-code drives real machines).
- Sending to the printer (Moonraker on the G1), which needs a small desktop wrapper.
- A translator from EasyGCode's settings files, on request.
- The agent (Claude with your key) editing settings, or Claude Code adding features from a
  description: everything is already plain data and one API.
