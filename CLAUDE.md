# CLAUDE.md: working on ezgc

Guidance for Claude Code (and humans) working in this repo. Read this first.

## What this is

**ezgc** is a phone-first web app that writes custom G-code for 3D printing (the Ginger G1 pellet
printer on Klipper, the Bambu P1P), with a Rust engine compiled to WebAssembly. It grew out of
EasyGCode (`jasperkrebs/easygcode`, the original, which stays its own project) and EasyCAD
(`jasperkrebs/easycad`, the experiment it borrows its engine and app shell from). The repository
is still called `easygcode2`.

- **Everything grows from one spiral.** Almost every project is one continuous spiral, no travel:
  a shape printed like a vase, ramping up off the bed over the first turn. Features (a flat bottom
  turning into the wall, a body profile, overhang layers, patterns, a shaped top) join the
  **Spiral** kind as settings that are off by default. Only a project that genuinely isn't a
  spiral (the bend stool) becomes a kind of its own.
- A **project** is a small JSON file: its kind, a name, notes and the kind's settings. No printer.
- A **printer** is a profile on the device (based on the G1 or the P1P): one project prints on
  any printer, and a project sent to someone prints on theirs.
- **No tabs per project**: settings windows are drawn from schemas; projects and printers live in a
  library on the device and travel as files.

## Read in this order

1. `README.md`: the idea
2. `docs/roadmap.md`: the owner's decisions, and what's next (growing the spiral)
3. `docs/architecture.md`: how it's built and why, and how to add a feature (§8)
4. `docs/devlog.md`: what landed recently, what's open

## Build and run

```sh
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
scripts/build-web.sh               # wasm32 build + wasm-bindgen → web/pkg/ (fails over the size budget)
scripts/serve.sh                   # serves web/ at http://localhost:8080 (WebAssembly can't load from file://)
node --test scripts/wasm.test.mjs scripts/library.test.mjs scripts/form.test.mjs
node scripts/smoke-test.cjs [dir]  # headless Chromium, phone + desktop; screenshots into [dir]
UPDATE_SNAPSHOTS=1 cargo test -p ezgc-engine snapshots   # after a G-code change you meant; review the diff
python3 scripts/third-party-notices.py             # after adding/updating a dependency (CI runs --check)
```

CI (`.github/workflows/ci.yml`) runs all of the above on every push, and deploys `web/` to GitHub
Pages from `main`. CI uses the newest stable Rust: its clippy can be stricter than an older local
one, so update the toolchain (`rustup update stable`) when CI flags a lint you don't see. Look at
the smoke-test screenshots for any UI change.

- `web/pkg/` is generated; never edit it or commit it.
- `wasm-bindgen-cli` must match the `wasm-bindgen` version in `Cargo.lock` exactly.

## Workflow

- Work on the session's `claude/…` branch and open **pull requests into `main`**; the owner merges,
  and `main` deploys to GitHub Pages. Never push to `main` directly.
- Sync `main` before starting and before opening a PR; keep PRs small; resolve conflicts by
  keeping the non-overlapping changes from both sides.

## Non-negotiables

- **The project file is the single source of truth.** Toolpaths and G-code are always regenerated
  from it and the printer profile. Settings a file leaves out take their defaults, and a new
  setting's default must print exactly as before it existed.
- **Grow the spiral; don't fork it.** A new technique is a toolbox step and a settings group on the
  Spiral, off by default. A new kind only for a project that isn't one continuous spiral, and only
  after asking the owner.
- **Kinds never write G-code.** They build `Toolpath`s from toolbox steps; `gcode.rs` writes them.
- **Settings windows are data.** A setting is a field in a schema, never hand-written HTML. Tests
  keep each schema and its settings struct in step.
- **Speed is volumetric flow** (mm³/s), always; show the feed it gives next to it.
- **G-code safety.** G-code drives real machines.
  - The snapshot tests (`fixtures/snapshots/`) fail on any change to the G-code: rewrite them only
    for a change you meant, and say what changed in the PR.
  - The start and end blocks are EasyGCode's proven ones; changing them, or adding G-code for a new
    machine, needs the owner's OK and a hand-checked fixture.
  - The writer refuses toolpaths off the bed, past the build height, with nonsense extrusion or
    feeds, or with temperatures outside the machine's limits. Keep it that way.
- **EasyGCode is a reference, not a spec.** Don't test against it or copy its quirks unless the
  owner asks; take its ideas and its tested numbers where they help.
- **Never crash.** No `unwrap`/`expect`/`panic!`/`todo!` in non-test Rust, no `unsafe`. Bad input
  of any kind becomes a message. Every crash fix comes with a regression test.
- **Never break wasm.** Everything in `crates/` builds for `wasm32-unknown-unknown` and stays
  inside the size budget; the browser's build must write the same G-code as the native one (the
  snapshots check both).
- **Layering.** The engine knows nothing about the browser, files or the network; the UI talks
  only to the engine's API (`crates/engine/src/lib.rs`, through `crates/wasm`).
- **Phone-first UI in the EasyGCode style.** Plain HTML/CSS/ES modules, no framework, no bundler.
  Third-party UI code is vendored and pinned in `web/vendor/`. Every shipped file is listed in
  `web/sw.js` (the smoke test checks).
- **Licenses.** ezgc is all rights reserved. Dependencies must be permissive (MIT/Apache/BSD);
  list every one in `THIRD-PARTY-NOTICES.md` in the same change.

## Conventions

- **Rust:** edition 2024, `cargo fmt`, `cargo clippy -- -D warnings`, tests with every change.
  Trigonometry and distances go through `num.rs` (V8's own `sin`/`cos`/`pow`/`hypot`), so native
  and browser builds compute the same bits.
- **JavaScript:** ES modules, 2-space indent, small pure helpers, JSDoc types. Text from a project
  or printer file only reaches the page through `textContent`.
- **Comments explain geometry and intent**, as in EasyGCode.
- **Docs move with the code.** Update the relevant doc in the same change and add a short entry to
  `docs/devlog.md` (what landed, what's still open).
