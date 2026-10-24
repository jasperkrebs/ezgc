# ezgc

**Custom 3D-printing G-code on the go**: on your phone and computer, in the browser, installable,
offline. ezgc grew out of [EasyGCode](https://github.com/jasperkrebs/easygcode) (which stays its own
project) and runs on [EasyCAD](https://github.com/jasperkrebs/easycad)'s Rust engine, streamlined:

- **Everything grows from one spiral.** One continuous path, no travel: a shape printed like a
  vase, its extrusion ramping up off the bed over the first turn, a level rim at the top. EasyGCode's
  projects (container, spiral disc, coat hanger, lampshade, …) come back as features of that one
  spiral, not as tabs.
- **Projects** are small JSON files: a name, notes and the spiral's settings. Tap the project's
  name to make a new one, open, duplicate, export or delete one. Everything is saved on the device
  as you go, and travels as a file.
- **Printers** are profiles on the device (the Ginger Additive G1 on pellets, the Bambu P1P on
  filament, or your own based on one of them): every project prints on the printer you choose, and
  a project you send someone prints on theirs.
- **Speed as flow**: you set volumetric flow (mm³/s); the feed it gives is shown right next to it.
- **Live.** Every change re-runs the engine (Rust, compiled to WebAssembly, in the background):
  the 3D preview, coloured by speed, the print time and material, and the G-code.
- **Safe G-code.** One writer with EasyGCode's proven start and end blocks; it refuses toolpaths
  that leave the bed or ask the printer for temperatures it doesn't take.

> **Status (22 Oct 2026):** the spiral with six shapes, printer profiles, projects on the device,
> and the first EasyGCode project grown onto the spiral: the **salt & pepper shaker** (a container
> with a flat bottom, a body profile and closer layers on overhangs, and a lid with holes that
> screws on; New project → "Salt & pepper shaker"). Next: the lampshade
> ([roadmap](docs/roadmap.md)).

## Docs

| | |
|---|---|
| [docs/roadmap.md](docs/roadmap.md) | The decisions, and what's next |
| [docs/architecture.md](docs/architecture.md) | How it's built and why; how the spiral grows |
| [docs/devlog.md](docs/devlog.md) | What landed, what's open |
| [CLAUDE.md](CLAUDE.md) | Guidance for AI agents (and humans) working in this repo |

## Develop

Requirements: Rust (stable, with the `wasm32-unknown-unknown` target), `wasm-bindgen-cli` (the
version of the `wasm-bindgen` crate in `Cargo.lock`), Node 22 and Python 3.

```sh
cargo test --workspace     # engine tests, including the G-code snapshots
scripts/build-web.sh       # Rust → WebAssembly → web/pkg/
scripts/serve.sh           # http://localhost:8080
```

Changes reach `main` through pull requests; CI checks every push and deploys `main` to GitHub
Pages once Pages is switched on (Settings → Pages → Source: GitHub Actions).

## License

All rights reserved. Third-party components are permissively licensed; see
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
