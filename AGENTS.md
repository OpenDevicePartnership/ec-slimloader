# AGENTS.md

Operational guide for AI coding agents (GitHub Copilot, Claude, etc.) working in
the `openDevicePartnership/ec-slimloader` repository. This file is the
authoritative superset of `.github/copilot-instructions.md`; when guidance
appears in both, this file wins.

Human contributors are also welcome to read this — it doubles as a tour of the
repository's structure, build commands, and conventions.

---

## 1. Repository at a glance

`ec-slimloader` is a light-weight, fail-safe bootloader written in Rust with a
NOR-flash backed state journal. It is a **stage-two bootloader**: a small ROM
or ROM-loader runs first, hands control to `ec-slimloader`, which then verifies
and jumps into one of two application image slots ("A/B" boot).

The framework is intentionally platform-agnostic at its core. The only
opinionated piece is **how state is stored** (in a journal across at least two
NOR-flash pages). Platform-specific glue lives in separate support crates.

Today the only supported family is **NXP IMXRT685S / IMXRT633S** via the
`ec-slimloader-imxrt` crate. The repository also ships:

* a host-side tool (`bootloader-tool`) for key generation, image signing,
  flashing, fuse programming, and probe-rs/RTT attachment, and
* a complete `examples/rt685s/` workspace targeting the NXP MIMXRT685S-EVK.

### Top-level layout

```
.
├── libs/                       # core library crates (cargo workspace)
│   ├── ec-slimloader/          # platform-agnostic core (no_std)
│   ├── ec-slimloader-state/    # NOR-flash journal (no_std, fuzzed)
│   │   └── fuzz/               # cargo-fuzz targets
│   ├── ec-slimloader-imxrt/    # NXP IMXRT6xx platform support (no_std)
│   └── imxrt-rom/              # Rust bindings for NXP ROM API
├── examples/rt685s/            # binary example workspace
│   ├── bootloader/             # the bootloader binary (uses memory.x)
│   ├── application/            # the example application (uses memory.x)
│   └── bsp/                    # shared board support
├── bootloader-tool/            # host-side CLI (probe-rs, signing, fuses)
├── .github/
│   ├── workflows/check.yml     # the only CI workflow
│   └── copilot-instructions.md # commit-message + Assisted-by rules
├── rustfmt.toml                # nightly-only options; see §5
├── deny.toml                   # cargo-deny config (licenses, advisories)
├── CONTRIBUTING.md             # human-facing contributing rules
├── CODEOWNERS, SECURITY.md, CODE_OF_CONDUCT.md, LICENSE
└── AGENTS.md                   # this file
```

There is **no root `Cargo.toml`**. The repo contains **three independent
cargo workspaces** plus one nested fuzz workspace:

| Workspace path                      | Members                                                            |
| ----------------------------------- | ------------------------------------------------------------------ |
| `libs/`                             | `ec-slimloader`, `ec-slimloader-imxrt`, `ec-slimloader-state`, `imxrt-rom` |
| `examples/rt685s/`                  | `application`, `bootloader`, `bsp`                                 |
| `bootloader-tool/`                  | single binary crate                                                |
| `libs/ec-slimloader-state/fuzz/`    | fuzz harnesses (`random-flash`, `interrupted`)                     |

Every CI command (and likely every command you run) **must be executed from
the appropriate workspace root**. Running `cargo` at the repo root will fail
with "could not find `Cargo.toml`".

---

## 2. Required toolchains and targets

### Embedded target

All firmware crates build for **`thumbv8m.main-none-eabihf`** (ARMv8-M
Mainline, hard-float — Cortex-M33 on the IMXRT6xx). Install it once:

```
rustup target add thumbv8m.main-none-eabihf
```

### Channels

| Job        | Channel                | Why                                                                |
| ---------- | ---------------------- | ------------------------------------------------------------------ |
| `fmt`      | **nightly**            | `rustfmt.toml` uses `group_imports` + `imports_granularity` (unstable) |
| `clippy`   | stable **and** beta    | Catch new lints early                                              |
| `doc`      | **nightly**            | Uses `--cfg docsrs` for `doc_cfg`                                  |
| `hack`     | stable                 | Manual feature-matrix build via `cargo-batch`                      |
| `deny`     | stable                 | `cargo-deny` against the two workspace manifests                   |
| `msrv`     | **1.90**               | Pinned by `embassy-imxrt` requirements                             |
| `fuzz`     | stable                 | `cargo-fuzz` targets in `libs/ec-slimloader-state/fuzz/`           |

If you only have stable installed, `cargo fmt --check` and `cargo doc` will
still run, but the rustfmt-unstable options will be silently ignored and you
will see warnings such as:

```
Warning: can't set `group_imports = StdExternalCrate`, unstable features are only available in nightly channel.
```

This is **expected on stable** and not a failure. For an authoritative fmt
check, use nightly. (The CI fmt job uses `dtolnay/rust-toolchain@nightly`.)

### Host tools used by CI

* `cargo-batch` (used by both `ci.sh` scripts):
  `cargo install --git https://github.com/embassy-rs/cargo-batch cargo --bin cargo-batch --locked`
* `cargo-fuzz` (for the fuzz job): `cargo install cargo-fuzz --locked`
* `cargo-deny` (run via the `EmbarkStudios/cargo-deny-action`)
* On Linux CI: `libudev-dev` is installed for the `bootloader-tool` build
  (needed transitively by `probe-rs`).

There is no `rust-toolchain.toml` pinning a channel in this repo; the active
toolchain is whatever rustup chooses or whatever an outer directory overrides
to. Be mindful when working from another worktree.

---

## 3. Build & verification commands

The `.github/workflows/check.yml` file is the single source of truth. The
exact commands below are the ones CI runs — reproduce them locally before
declaring a change "ready".

### Format (CI uses nightly)

```bash
# from libs/
cargo fmt --check

# from examples/rt685s/
cargo fmt --check

# from bootloader-tool/
cargo fmt --check
```

### Clippy (stable & beta, with strict lints)

```bash
# libs and examples use the same flag set:
#   -F clippy::suspicious -D clippy::correctness -F clippy::perf -F clippy::style
# bootloader-tool uses -D for clippy::suspicious instead of -F.

# from libs/
cargo clippy --target thumbv8m.main-none-eabihf -- \
  -F clippy::suspicious -D clippy::correctness -F clippy::perf -F clippy::style

# from examples/rt685s/
cargo clippy --target thumbv8m.main-none-eabihf -- \
  -F clippy::suspicious -D clippy::correctness -F clippy::perf -F clippy::style

# from bootloader-tool/  (host target)
cargo clippy -- \
  -D clippy::suspicious -D clippy::correctness -F clippy::perf -F clippy::style
```

Note `-F` (= `--forbid`) is stricter than `-D` (= `--deny`); forbid cannot be
overridden by `#[allow(...)]` in source.

### Docs (CI uses nightly)

```bash
# from libs/  (note required feature flag)
RUSTDOCFLAGS="--cfg docsrs" cargo doc --no-deps --features mimxrt685s

# from examples/rt685s/
RUSTDOCFLAGS="--cfg docsrs" cargo doc --no-deps
```

### Feature matrix (`cargo-batch`)

The `hack` job runs the in-tree `ci.sh` scripts. They drive `cargo batch` over
a fixed list of feature combinations and (for `libs`) also run host-target
tests.

`libs/ci.sh` builds these feature combos for `thumbv8m.main-none-eabihf`:

```
mimxrt633s
mimxrt633s,defmt
mimxrt633s,log
mimxrt633s,non-secure
mimxrt685s
mimxrt685s,defmt
mimxrt685s,log
mimxrt685s,non-secure
```

…and then runs host tests:

```bash
cargo test --locked --target x86_64-unknown-linux-gnu --features mimxrt633s
cargo test --locked --target x86_64-unknown-linux-gnu --features mimxrt685s
```

`examples/rt685s/ci.sh` builds the workspace once with no extra features and
then once per feature from `(defmt, non-secure)`.

Both scripts set `RUSTFLAGS=-Dwarnings` and `DEFMT_LOG=trace`.

### Cargo-deny

```bash
# Run against each workspace manifest separately:
cargo deny --manifest-path libs/Cargo.toml check
cargo deny --manifest-path examples/rt685s/Cargo.toml check
```

`deny.toml` lives at the repo root and is shared between both invocations.

### MSRV (1.90)

```bash
cargo +1.90 check --features mimxrt685s            # in libs/
cargo +1.90 check                                  # in examples/rt685s/
cargo +1.90 check                                  # in bootloader-tool/
```

### Fuzzing

```bash
cd libs/ec-slimloader-state/fuzz
cargo fuzz run --sanitizer none -j$(nproc) random-flash  -- -max_total_time=10
cargo fuzz run --sanitizer none -j$(nproc) interrupted   -- -max_total_time=10
```

The `_test` feature on `ec-slimloader-state` (gated `arbitrary` derive
support) exists specifically for fuzz harnesses; do not enable it elsewhere.

### Minimum local pre-flight

If you only have time for a quick check before pushing, run at minimum:

```bash
( cd libs              && cargo fmt --check && cargo check --features mimxrt685s --target thumbv8m.main-none-eabihf )
( cd examples/rt685s   && cargo fmt --check && cargo check --target thumbv8m.main-none-eabihf )
( cd bootloader-tool   && cargo fmt --check && cargo check )
```

---

## 4. Crate-by-crate map

### `libs/ec-slimloader` (no_std)

The platform-agnostic core. Exposes two main traits:

* `BootStatePolicy` — application-supplied policy (default state, custom
  state validation).
* `Board` — the per-platform integration point. Implementations provide
  `init`, `journal()` (mutable access to the `FlashJournal`),
  `check_and_boot(&Slot)`, and `abort() -> !`.

Features: `defmt`, `log` (mutually compatible via `defmt-or-log`), default
empty.

### `libs/ec-slimloader-state` (no_std, fuzzed)

The fail-safe state journal that records which slot to boot and its lifecycle
(`Initial → Attempting → Confirmed / Failed`). Designed to tolerate
interrupted writes; correctness is exercised by the `cargo-fuzz` harnesses in
`fuzz/`. CRC32 (via the `crc` crate) protects each record.

Both the bootloader and the application read and mutate this journal — the
application must mark a freshly booted slot `confirmed` or the bootloader will
roll back to the backup slot on the next boot.

### `libs/ec-slimloader-imxrt` (no_std)

Platform support for NXP IMXRT685S / IMXRT633S. Submodules:

* `bootload.rs` — sets up the application's vector table and performs the
  Cortex-M jump.
* `fcb.rs` — Flash Configuration Block emission for booting from QSPI.
* `mbi.rs` — Master Boot Image header parsing/copying.
* `verification.rs` — leverages NXP ROM authentication routines (skipped when
  the `non-secure` feature is enabled).

Key features:

* `mimxrt685s` / `mimxrt633s` — chip selection (mutually exclusive — pick one).
* `mimxrt685s-evk` — convenience: enables FCB for the RT685-EVK board.
* `non-secure` — **disables image verification**. Use only for development.
* `fcb`, `imxrt-fcb-1spi-nor`, `imxrt-fcb-rt685evk`, `imxrt-fcb-1spi-a1-nor`,
  `imxrt-fcb-1spi-b1-nor` — FCB layout selection.
* `empty-otfad` — emit an all-zero OTFAD region (XIP off, no encryption).
* `defmt` / `log` — logging backends.

This crate pulls `embassy-imxrt` and `partition-manager` from git tags
(`v0.1.0` each). Bumping those tags is a coordinated change.

### `libs/imxrt-rom`

Safe-ish Rust wrappers around the NXP ROM API tables (fuse access and the
image authentication routine). Has a `build.rs`; the `rt` feature is required
when used as a runtime dependency.

### `bootloader-tool` (host crate, edition 2024)

A `clap`-based CLI with subcommands:

```
generate   keys & certificates
sign       sign binaries / extract FCB ("prelude")
download   flash binaries via probe-rs
run        flash + attach to the bootloader chain (RTT/defmt output)
fuse       burn fuse registers / set OTP shadow registers
```

Config lives in `./config.toml` (defaults to `./config.toml`, overridable with
`-c`). Artifacts are placed under `./artifacts/`. See `README.md` §"Quick
guide" for the end-to-end flow.

This crate is **only relevant for IMXRT** and is not built for any embedded
target.

### `examples/rt685s/{bootloader,application,bsp}`

Real binaries that link the libs together. Each binary crate ships its own
`build.rs` and `memory.x` (see §6 for layout). The workspace has its own
release profile (`lto = true`, `debug = 2`, `opt-level = "s"`) — match this
profile if you add a new example binary.

---

## 5. Code style

* `rustfmt.toml`:
  * `group_imports = "StdExternalCrate"` (nightly)
  * `imports_granularity = "Module"` (nightly)
  * `max_width = 120`
* Workspace lint: `clippy::manual_let_else = "deny"` is set in
  `libs/Cargo.toml`. Use `let … else` instead of manual `match`/`if let`
  patterns.
* `examples/rt685s` sets `warnings = "deny"` at the workspace level (every
  warning fails the build).
* `bootloader-tool` is edition 2024; the `libs/*` and `examples/rt685s/*`
  crates are edition 2021. Don't "upgrade" them in passing.
* Comments are minimal in this codebase — prefer expressive types and names
  over commentary. Comment only when something genuinely needs clarification.

---

## 6. Bootloader specifics: targets, linkers, memory layout

Both binary crates use `cortex-m-rt`-style linker integration with a
per-binary `memory.x` describing the chip's memory map.

### Bootloader memory map (`examples/rt685s/bootloader/memory.x`)

```
MEMORY {
  PRELUDE_OTFAD : ORIGIN = 0x08000000, LENGTH = 256
  PRELUDE_FCB   : ORIGIN = 0x08000400, LENGTH = 512
  PRELUDE_BIV   : ORIGIN = 0x08000600, LENGTH = 4

  RAM           : ORIGIN = 0x30176000, LENGTH = 20K
  FLASH         : ORIGIN = 0x10170000, LENGTH = 24K  /* XIP mapped into RAM */
  ROM_TABLE (r) : ORIGIN = 0x1303F000, LENGTH = 64
}
```

The `.otfad`, `.fcb`, `.biv`, and `.rom_table` sections are inserted after
`.uninit`. The whole range `0x08000000…0x08001000` is collectively called the
**prelude** and contains everything the NXP ROM needs to find the FCB and
start executing the bootloader.

`bootloader-tool sign bootloader …` extracts the prelude into a separate ELF
(`ec-slimloader.prelude.elf`) for independent flashing.

### Application memory map (`examples/rt685s/application/memory.x`)

```
MEMORY {
  FLASH         : ORIGIN = 0x10020000, LENGTH = 1M
  RAM           : ORIGIN = 0x30120000, LENGTH = 32K
  ROM_TABLE (r) : ORIGIN = 0x1303F000, LENGTH = 64
}
```

There are two image slots on the device (slot 0 and slot 1). The application
is built once and downloaded to both slots; the bootloader picks which one to
run based on the journal.

### What you must rewire if porting

If you implement a new platform-support crate (the equivalent of
`ec-slimloader-imxrt` for another MCU), you must decide and configure:

* From which memory the bootloader itself runs and where its working RAM lives.
* The NOR-flash region used for the state journal (≥ 2 erasable pages).
* The memory regions for image slot 0, slot 1, … (≥ 2 slots).
* How images are loaded (in-place XIP, copy-to-RAM, swap external↔internal).
* How images are verified (or whether to skip verification — `non-secure`
  feature in the IMXRT crate is the precedent for "skip everything").
* How the jump to the application is performed (Cortex-M vector-table setup,
  RISC-V `mret`, etc.).

The application side must also be wired up to mutate the journal — write
"attempting" before reboot for a new image, "confirmed" after a successful
boot.

---

## 7. Git workflow and commit policy

### Per-commit author identity

When committing as an AI agent on someone's behalf, **always set the author
inline on the commit** rather than touching `git config --global user.*`:

```bash
git -c user.name="Felipe Balbi" \
    -c user.email="felipe.balbi@microsoft.com" \
    commit -m "..."
```

Replace name/email with the actual human author when you are running on
someone else's behalf.

### Commit message format (from `.github/copilot-instructions.md`)

* Subject line: capitalized, **50 characters or less**, imperative mood
  ("Fix bug" not "Fixed bug").
* Blank line separates subject from body.
* Wrap body text at **72 characters**.
* Body explains **what** and **why**, not **how**.

`CONTRIBUTING.md` additionally requires:

* Each commit must build successfully without warnings (CI enforces
  `-Dwarnings` in `ci.sh`).
* Misc commits to fix typos / formatting are squashed before review.
* Squash-merging is disabled at the org level; keep the history clean as you
  go.

### AI attribution trailer (REQUIRED)

Every commit that includes AI-generated or AI-assisted work **must** carry an
`Assisted-by` trailer:

```
Assisted-by: AGENT_NAME:MODEL_VERSION [TOOL1] [TOOL2]
```

* `AGENT_NAME`: e.g. `GitHub Copilot`.
* `MODEL_VERSION`: the **actual** model you are running. **Verify your own
  identity before composing the trailer — never hard-code a version from a
  previous session.**
* Optional bracketed tools: specialized analysis tools (e.g. `coccinelle`,
  `clang-tidy`). Basic dev tools (git, cargo, editors) are **not** listed.

AI agents **MUST NOT** add `Signed-off-by` — only humans can certify the DCO.

Example trailer line (using a model you should actually verify):

```
Assisted-by: GitHub Copilot:claude-opus-4.7
```

### PR etiquette

* Open as a **draft PR first** (per `CONTRIBUTING.md`).
* Ensure CI is green on the draft before requesting review.
* Do not force-push to shared branches without coordination. When this guide
  is used by an automation that explicitly says "no PR" or "push to fork
  only", honor that and stop.

### Line endings

The repo uses **LF** throughout (verified for `README.md` and
`.github/copilot-instructions.md`). The repo does **not** ship a
`.gitattributes` file. On Windows, set:

```
git config core.autocrlf false
```

per-clone, and configure your editor to write LF. Never commit a `^M` —
`git diff --check` is your friend.

### Submodules

CI checks out with `submodules: true`. Today there are no submodules in tree,
but if you add any, run `git submodule update --init --recursive` after
cloning.

---

## 8. Workflow for AI agents

1. **Orient yourself.** Read this file, then `README.md`, then
   `.github/workflows/check.yml`. Skim each `Cargo.toml` you'll touch.
2. **Pick the right workspace.** `cd` into `libs/`, `examples/rt685s/`, or
   `bootloader-tool/` before running `cargo`. Never run `cargo` at the repo
   root.
3. **Make focused changes.** Don't refactor unrelated code, don't bump
   dependency versions opportunistically, don't reformat files you didn't
   touch.
4. **Run the relevant CI commands locally** (see §3). At minimum: `cargo fmt
   --check` and `cargo check` for every workspace you touched.
5. **For new features that gate behavior, mirror the existing matrix.** If
   you add a feature to `ec-slimloader-imxrt`, add a corresponding entry to
   `libs/ci.sh`'s `FEATURE_COMBINATIONS`.
6. **For new fuzzing-relevant data structures**, add a harness under
   `libs/ec-slimloader-state/fuzz/` and a target name to the `bin:` matrix
   in `.github/workflows/check.yml`.
7. **Commit using the rules in §7.** Verify your model version. Include the
   `Assisted-by` trailer. Set the author per-commit, not globally.
8. **Don't open PRs unless explicitly instructed.** When a tasking message
   says "push to fork only" or "stop at pushed to fork", obey it literally —
   do not run `gh pr create`.

### Common pitfalls

* Running `cargo fmt` on **stable** silently skips the nightly-only options;
  the file may still pass locally but fail nightly CI. Use nightly when in
  doubt.
* Forgetting `--features mimxrt685s` (or `mimxrt633s`) when building `libs/`
  will leave the IMXRT crate effectively unconfigured and produce confusing
  errors.
* `bootloader-tool` builds for the **host**, not for `thumbv8m`. Don't pass
  `--target thumbv8m.main-none-eabihf` there.
* `non-secure` skips image verification. Do not enable it in CI defaults,
  do not enable it in any artifact intended for a real device.
* `embassy-imxrt` and `partition-manager` are pinned to git tag `v0.1.0`.
  Updating either is a coordinated, opinionated change — not a drive-by.
* `ec-slimloader-state` `_test` feature is for fuzzing only.
* When porting to a new platform, both `Board::abort` and
  `Board::check_and_boot` are infallible in the "successful boot" sense:
  `check_and_boot` does **not return** on success.

---

## 9. Source of truth and precedence

If two documents disagree:

1. `.github/workflows/check.yml` defines what CI actually runs — it is the
   ground truth for build/test commands.
2. This `AGENTS.md` is the next-most-authoritative summary, including for AI
   workflow and commit-message rules. It is the **superset** of
   `.github/copilot-instructions.md`.
3. `.github/copilot-instructions.md` is preserved for tools that look for it
   by name; it points back here.
4. `README.md` documents end-user workflows (build, flash, sign, attach).
5. `CONTRIBUTING.md` is the human contributor policy.

If you discover a real contradiction, prefer fixing the source of truth (the
workflow file or this document) rather than papering over it in a comment.

## Model selection & cost discipline

Premium models (Opus, GPT-5 family, "high"/"xhigh" reasoning variants)
cost an order of magnitude more than standard models (Sonnet, Haiku,
mini). Most steps in a typical task do not need premium reasoning,
and over-using premium models wastes credits without improving
outcomes. The rules below apply to *all* model selection: your own
session, sub-agents launched via the `task` tool, and parallel work
launched via `/fleet`.

### Default posture

- **Default to the cheapest model that can do the job.** Reach for a
  premium model only when one of the escalation triggers below is hit.
- **Plan with premium, execute with cheap.** Spend at most one or two
  premium turns on design / planning, then downshift to a cheaper
  model for mechanical execution of the plan.
- **Never bump the model "just in case."** If you cannot articulate
  *why* a cheaper model would fail, use the cheaper model.

### Escalation triggers (use a premium model)

Reach for a premium model when *any* of these are true:

- Cross-module refactor, architectural design, or API design from
  scratch.
- Subtle correctness reasoning: concurrency, lifetimes, `unsafe`,
  FFI ABI, cryptography, safety-critical control paths.
- Debugging a failure that survived one prior cheap-model attempt.
- Reviewing code on a safety-, security-, or money-critical path.
- The diff cannot be predicted in advance — i.e. there is genuine
  creative or design work to do, not just typing.

### De-escalation triggers (use a cheap model)

Use the cheapest available model when *any* of these are true:

- Searching, reading, summarising files or docs.
- Single-file mechanical edits: rename, format, lint fix, dependency
  bump, boilerplate, scaffolding from a known template.
- Generating tests for code that already works.
- Running builds, tests, linters, or other commands where the model
  only needs to report success/failure.
- Routine commits, PR descriptions, changelog entries.
- The diff is essentially predictable before generation.

### Sub-agent routing (the `task` tool)

When delegating with the `task` tool, set `model:` explicitly. Do not
let sub-agents inherit a premium default for cheap work.

| Sub-agent type    | Default model             | Override to                                     |
|-------------------|---------------------------|-------------------------------------------------|
| `explore`         | cheap                     | keep cheap (`claude-haiku-4.5` or `gpt-5-mini`) |
| `task` (run cmd)  | cheap                     | keep cheap                                      |
| `research`        | cheap for breadth         | premium only for the final synthesis            |
| `general-purpose` | match task                | cheap for mechanical work; premium for design   |
| `rubber-duck`     | premium                   | keep premium — this is where reasoning pays off |
| `code-review`     | premium on critical paths | cheap on cosmetic / mechanical diffs            |

### `/fleet` (parallel sub-agents) rules

- Fleet mode multiplies cost by the fleet width. Apply the rules
  above *per worker*, not in aggregate.
- Split a fleet job along complexity lines: route the cheap,
  parallelisable workers (file edits, test runs, doc updates) to a
  cheap model; reserve premium models for the small number of
  workers that need real reasoning.
- If every worker in a fleet would need a premium model, the work is
  probably not a good fit for fleet mode — reconsider the
  decomposition before paying N× premium.

### Session hygiene

- Keep sessions short and focused. Long premium sessions are the
  single largest source of waste because every turn re-processes the
  full history.
- Use `/compact` when the conversation grows long, and `/new` for
  unrelated work.
- Prefer `/ask` for one-off side questions so they don't extend the
  main session.

### When in doubt

Ask: *"If a cheaper model produced the wrong answer here, would I
catch it in seconds (compiler, tests, my own review) or in
weeks (production incident)?"* If the former, use the cheap model
and let the feedback loop do its job.
