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
