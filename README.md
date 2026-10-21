# EasyGCode 2

**EasyGCode on EasyCAD's architecture.** For creating custom 3D-printing G-code on the go: on your
phone and computer, in the browser, installable, offline. Same idea as
[EasyGCode](https://github.com/jasperkrebs/easygcode), streamlined:

- **No tab per project.** A project is a small JSON file: its *kind*, a name, notes, its settings
  and its printer's. Tap the project's name to make a new one, open, duplicate, export or delete
  one. Everything is saved on the device as you go, and travels as a file.
- **The settings window is drawn from the kind**, so a new project kind is a small Rust file, not
  a new tab of HTML. Fields that don't apply hide themselves; a bad value is marked where it is.
- **Live.** Every change re-runs the engine (Rust, compiled to WebAssembly, in the background):
  the 3D preview, coloured by speed as in EasyGCode, the print time and material, and the G-code.
- **The same G-code as EasyGCode.** One shared writer with EasyGCode's start and end blocks and
  bead model, golden-tested against G-code written by EasyGCode itself, command for command. It
  refuses toolpaths that leave the bed or ask the printer for temperatures it doesn't take.
- **Printers:** the Ginger Additive G1 (pellets, Klipper; E in mm³) and the Bambu P1P (filament).

> **Status (21 Oct 2026):** the base. One project kind, **Vase** (EasyGCode's coat hanger without
> its extras: six shapes as one vase-mode spiral), proves the whole path from settings to G-code.
> EasyGCode's projects come next, one at a time ([roadmap](docs/roadmap.md)).

## Docs

| | |
|---|---|
| [docs/architecture.md](docs/architecture.md) | How it's built and why: what came from EasyGCode, what from EasyCAD; how to port a tab |
| [docs/roadmap.md](docs/roadmap.md) | What's next, and open questions |
| [docs/devlog.md](docs/devlog.md) | What landed, what's open |
| [CLAUDE.md](CLAUDE.md) | Guidance for AI agents (and humans) working in this repo |

## Develop

Requirements: Rust (stable, with the `wasm32-unknown-unknown` target), `wasm-bindgen-cli` (the
version of the `wasm-bindgen` crate in `Cargo.lock`), Node 22 and Python 3.

```sh
cargo test --workspace     # engine tests, including the golden tests against EasyGCode
scripts/build-web.sh       # Rust → WebAssembly → web/pkg/
scripts/serve.sh           # http://localhost:8080
```

## License

All rights reserved. Third-party components are permissively licensed; see
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
