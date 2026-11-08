# Work recall validation — 2026-11-07

## Implemented and installed

Local branch: `feat/work-recall`. Installed version: `0.1.0-work-recall-20261107` under `~/.local/opt/omarchy-menu-rust/versions/`. The previous `0.1.0` version remains available. At the implementation-validation checkpoint, no commit, push, or GitHub issue publication had been performed. Later documentation checkpoints are recorded in [the change history](../CHANGELOG.md); they do not imply that the implementation was committed or published.

The local configuration enables eight explicitly selected project documents and local Ollama embeddings. Configuration and source contents are outside the repository. Queries for Music, Automation, Gate, and Pub/Sub returned real project documents. These are retrieval checks, not proof that every semantically related result is useful.

## Checks

- `cargo test --offline`: 64 binary tests and 2 integration tests passed. Tests requiring local HTTP fixture sockets were run outside the restricted sandbox.
- `cargo clippy --offline --all-targets -- -D warnings`: passed.
- Formatting and `git diff --check`: passed.
- Native candidate flow: root → Your work → Music → original source preview → scroll → return. Inspected screenshots for wrapping and readability.
- Full-sized panel geometry on the tested 2560×1440 output: 680×440 at x=940, y=572, putting its center at 55% of output height.
- Installed toggle launched the expected version, showed Music results, and closed with Escape. The installed process executable was checked through `/proc`.
- Removed a duplicate Alt+Space dictation binding before the existing menu binding. Hyprland reload succeeded and config errors were empty. The active binding list contains one Alt+Space menu binding; other dictation shortcuts remain.

## Remaining validation limit

Synthetic Alt+Space through `wtype` did not open the menu, even after removing the duplicate binding. Calling the installed toggle directly did work. The physical shortcut has not been verified; do not report end-to-end Alt+Space acceptance as passed. The cause of the synthetic shortcut failure remains unresolved.

## Local evidence and recovery

Evidence is in `~/Work/shade-menu-recall-notes/`: `final-tests.log`, `native.log`, `recall-*.png`, `installed-music.png`, and the native check scripts. Screenshots remain local because desktop content can be visible behind the panel.

The installed binary SHA-256 is `054e7cd9da635b53a4ea4969a12941baaf0aa29bf8cc87e672a856747739ace5`.

For rollback, close the menu and restore the prefix's `current` link and its `bin` entry links to the retained `versions/0.1.0` installation. Inspect the current links first; the launcher can resolve its own version independently of `current`. Keep the new version until rollback is confirmed. The pre-change desktop binding is backed up at `~/.config/hypr/bindings.lua.bak.menu-1794102887659420830`; restoring it would also restore the duplicate dictation shortcut, so prefer retaining the explicit unbind.

This release previews sources in process. External opens, music playback, agent dispatch, filesystem listeners, and floating web research remain future work. Embeddings use bounded document text rather than chunk retrieval.

## Publication checkpoint — 2026-11-07

Implementation commit: `4395ee1`. Jordan explicitly authorized pushing this work to `origin/feat/work-recall`. A fresh detached checkout of this commit built successfully with `cargo build --offline --locked --release -j 4`, using a new build directory and the local Cargo registry cache. Toolchain: `rustc 1.99.0 (b940084d7 2026-09-28)`. This is a fresh source/build-directory check on Thor, not a fresh OS or an Amp orb check.

Before the commit, the 66 tests, Clippy with warnings denied, and formatting checks were rerun successfully. No source changes followed those checks. The fresh-checkout build does not close native acceptance.

Code review follow-ups assigned to the first orb: preview scrolling bounds count original lines while painting wraps them (so long wrapped content is not fully reachable); late background results while previewing may reset return selection through GUI clamping; a config error during initial worker loading may require restart even after repair. The first issue follows directly from the code paths; the latter two need targeted reproduction and regression tests. See the [orb brief](orb-plan.md).

The implementation/shared-brain branch was pushed successfully to `origin/feat/work-recall` at `4395ee1` on 2026-11-07. The continuation/orb-plan documentation follows in the same branch history. `master` was not changed. No pull request or GitHub issue was posted.

## Installed planner update — 2026-11-07

Jordan requested updating the local menu to current work. Built the release binary and installed `0.1.0-fetch-planner-20261107`, retaining `0.1.0-work-recall-20261107` for rollback. Enabled the opt-in planner using the previously evaluated resident `qwen3-coder-next:48k`; backed up the private recall config first. Config and corpus remain outside git.

Installed binary SHA-256: `79af6dd4a1fe0c8cf779b5e236b3648ac0585e6413ae825c8109741fba0c71e7`. Installed CLI Music check timed out at the planner and returned correct original-query lexical Music results. The configured desktop toggle launched the new version; native Music rows were captured and visually checked, and Escape closed it. This confirms installation and usable fallback, not consistent planner latency or physical Alt+Space delivery. Existing no-exec toggle behavior remains in place.
