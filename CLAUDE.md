# CLAUDE.md: working on EasyGCode 2

Guidance for Claude Code (and humans) working in this repo. Read this first.

## What this is

EasyGCode 2 is **EasyGCode on EasyCAD's architecture**, streamlined: a phone-first web app that
writes custom G-code for 3D printing (the Ginger G1 pellet printer on Klipper, the Bambu P1P),
with a Rust engine compiled to WebAssembly.

- A **project** is a small JSON file: its **kind** (vase today; EasyGCode's coat hanger, bend
  stool, vessel… next), a name, notes, the kind's settings and the printer's settings.
- There are **no tabs per project**: the settings window is drawn from the kind's schema, and
  projects live in a library on the device (create, open, duplicate, export, import).
- **Kinds compose toolbox steps into toolpaths**; one G-code writer turns any toolpath into G-code.

EasyGCode (`jasperkrebs/easygcode`) is the original and stays as it is; EasyCAD
(`jasperkrebs/easycad`) is the experiment this borrows its engine and app shell from.

## Read in this order

1. `README.md`: the idea
2. `docs/architecture.md`: how it's built and why, and how to port an EasyGCode tab (§8)
3. `docs/roadmap.md`: what's next, and the open questions for the owner
4. `docs/devlog.md`: what landed recently, what's open

## Build and run

```sh
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
scripts/build-web.sh               # wasm32 build + wasm-bindgen → web/pkg/ (fails over the size budget)
scripts/serve.sh                   # serves web/ at http://localhost:8080 (WebAssembly can't load from file://)
node --test scripts/wasm.test.mjs scripts/library.test.mjs scripts/form.test.mjs
node scripts/smoke-test.cjs [dir]  # headless Chromium, phone + desktop; screenshots into [dir]
node scripts/easygcode-fixtures.cjs ../easygcode   # regenerate the golden fixtures from EasyGCode
python3 scripts/third-party-notices.py             # after adding/updating a dependency (CI runs --check)
```

CI (`.github/workflows/ci.yml`) runs all of the above except regenerating fixtures. Look at the
smoke-test screenshots for any UI change.

- `web/pkg/` is generated; never edit it or commit it.
- `wasm-bindgen-cli` must match the `wasm-bindgen` version in `Cargo.lock` exactly.

## Non-negotiables

- **The project file is the single source of truth.** Toolpaths and G-code are always regenerated
  from it. Settings that a file leaves out take their defaults, and a new setting's default must
  print exactly as before it existed.
- **Kinds never write G-code.** They build `Toolpath`s from toolbox steps; `gcode.rs` writes them.
  A technique goes into `toolbox/` once, so every kind can use it.
- **Settings windows are data.** A setting is a field in the kind's schema; never hand-written
  HTML per project. A test keeps each kind's schema and settings struct in step.
- **G-code safety.** G-code drives real machines. A ported project must match EasyGCode's G-code
  command for command (golden tests from `scripts/easygcode-fixtures.cjs`, natively and in
  `scripts/wasm.test.mjs`). Keep the start/end blocks identical to EasyGCode's. EasyGCode itself is
  never changed. New G-code with no EasyGCode counterpart needs the owner's OK and a hand-checked
  fixture.
- **JavaScript-exact numbers.** Geometry that's compared with EasyGCode uses `num.rs` (`hypot2/3`,
  `sin`, `cos`, `pow`, `fixed`, `round`), never `f64::sin`/`hypot` directly.
- **Never crash.** No `unwrap`/`expect`/`panic!`/`todo!` in non-test Rust, no `unsafe`. Bad input
  of any kind becomes a message. Every crash fix comes with a regression test.
- **Never break wasm.** Everything in `crates/` builds for `wasm32-unknown-unknown` and stays
  inside the size budget.
- **Layering.** The engine knows nothing about the browser, files or the network; the UI talks
  only to the engine's API (`crates/engine/src/lib.rs`, through `crates/wasm`).
- **Phone-first UI in the EasyGCode style.** Plain HTML/CSS/ES modules, no framework, no bundler.
  Third-party UI code is vendored and pinned in `web/vendor/`. Every shipped file is listed in
  `web/sw.js` (the smoke test checks).
- **Licenses.** All rights reserved (to be confirmed by the owner). Dependencies must be
  permissive (MIT/Apache/BSD); list every one in `THIRD-PARTY-NOTICES.md` in the same change.

## Conventions

- **Rust:** edition 2024, `cargo fmt`, `cargo clippy -- -D warnings`, tests with every change.
- **JavaScript:** ES modules, 2-space indent, small pure helpers, JSDoc types. Text from a project
  file only reaches the page through `textContent`.
- **Comments explain geometry and intent**, as in EasyGCode.
- **Docs move with the code.** Update the relevant doc in the same change and add a short entry to
  `docs/devlog.md` (what landed, what's still open).

## Collaboration workflow

As in EasyGCode: sync `main` before starting and before opening a PR; keep branches short-lived and
PRs small; resolve conflicts by keeping the non-overlapping changes from both sides.
