# Amp orb plan for Shade Menu

Prepared 2026-11-07. Assumption: “Amped orbs” means Amp at ampcode.com, whose official docs use the term Orbs. This is a recommended configuration and concrete handoff plan; no Amp project, orb, account connection, or setup hook has been created from this session.

## Start with two orbs

| Role | Machine recommendation | Mode recommendation | Initial ownership |
|---|---|---|---|
| Build orb | `a1.medium`: 4 CPUs / 8 GB | Amp medium; escalate to high for unresolved concurrency/design problems | R02 preview correctness and portable recall acceptance; its own source/test branch |
| Research/design orb | `a1.small`: 2 CPUs / 4 GB | Amp medium | R16 fetch-plan proposal plus R06 provider evidence; docs-only branch |
| Integrator on Thor | Existing workstation | Piper; verify actual model availability when delegating | Review, shared roadmap/changelog reconciliation, real Hyprland/Alt+Space acceptance, merging accepted work |

Sizes are starting engineering recommendations, not benchmarks. Raise the build orb to `a1.large` only if measured compile memory/time warrants it. Amp lists these sizes in [Sizes & Costs](https://ampcode.com/docs/orbs/sizes-and-costs). Amp modes select model/effort bundles; inspect the actual routing rather than assuming Luna or Sol aliases exist there. [Modes & Models](https://ampcode.com/docs/models-and-subagents).

Keep one code-writing orb initially. Research can proceed independently. Add a second builder only after file boundaries and contracts are stable; do not assign two workers to `src/gui.rs` simultaneously. Reuse or replace finished task threads as appropriate; do not build a permanent dispatcher service just to coordinate two workers.

## Project settings

- Repository: `https://github.com/shadetheory/menu`. Starting code: **`feat/work-recall`**, not the older `master` baseline. Confirm the checkout contains `src/recall.rs`, `src/recall_ui.rs`, and this document before working.
- Ship Behavior: **Push to Branch**. Amp's default Ship commits/rebases/pushes to the base branch; this project should review worker branches before integration. This recommendation does not change your Amp settings automatically. [Shipping Changes](https://ampcode.com/docs/orbs/shipping).
- One branch per task, e.g. `orb/r02-preview-acceptance` and `orb/r06-r16-fetch-design`, created from the latest agreed integration commit. Record that base SHA in the handoff. Do not force-push a shared branch or independently “ship” workers into master.
- Connect only this repository as the required project source. No personal corpus, home-directory mount, production keys, SSH access to Thor/odin, or tailnet connection is needed for the first two tasks.
- Browser profile: **No Profile** for building and public-doc research. Amp profiles preserve browser sign-ins and site data; a dedicated test profile can be selected later when an authorized task needs it. [Profiles](https://ampcode.com/docs/orbs/profiles).
- No always-on app portal is needed for the native Rust app. A docs/diagram portal is optional and must be labeled as documentation. Orb Desktop is a separate Linux environment; it cannot establish the behavior of Thor's compositor or physical shortcut. [Portals & Desktop](https://ampcode.com/docs/orbs/portals).

## Environment preparation — first orb task, R15

Amp documents Debian 12 orbs and executable `.agents/setup` / `.agents/resume` lifecycle hooks. Setup should prepare reusable dependencies; resume should do fast repeatable checks. Validate both in an actual fresh orb before committing them. Do not claim these hooks are installed merely because this plan exists. [Customizing Orbs](https://ampcode.com/docs/orbs/customizing).

Recommended setup contract:

1. Inspect OS, architecture, installed tools, and the checked-out branch. Current Thor verification used `rustc 1.99.0`; reproduce that toolchain with Cargo, rustfmt, and Clippy, then record the orb toolchain explicitly. Do not substitute Debian's default Rust without checking compatibility.
2. Install missing build/native prerequisites from the orb's package manager: C compiler/build tools, `pkg-config`, Wayland and XKB development/runtime libraries, required X11/XCB libraries, plus Python 3, Git, Bash, and `flock`. Resolve exact Debian package names in the orb; `INSTALL.md` records runtime library expectations. No remote installer pipeline. If a downloaded installation script is needed, inspect it and obtain Jordan's approval before running it.
3. Fetch Cargo dependencies using the committed lockfile and warm a debug build. Set `CARGO_BUILD_JOBS=2` on the medium orb initially and keep the Cargo registry/build cache in that orb's snapshot. Avoid a full release build on every wake.
4. Use synthetic Markdown fixtures and temporary config/cache directories for recall tests. Do not copy Jordan's eight private source paths or cache. Fake loopback embedding servers already exercise semantic behavior; a GPU or real Ollama model is unnecessary for the first regression task.
5. Resume checks required tools and repository location, then exits. It must not reset git, pull over in-progress changes, install software, start production services, or launch the menu on Thor.

For headless UI checks, add Sway, wtype, and grim in the isolated orb after confirming support. Use `WLR_BACKENDS=headless`, a private `XDG_RUNTIME_DIR`, and an explicitly created writable `OUT` directory with the existing test scripts. A headless Sway pass and an Orb Desktop/X11 preview are separate evidence from Hyprland acceptance. Do not auto-start a native UI or install into the host prefix from setup.

## First build-orb brief

> Read AGENTS.md, docs/project-brain.md, docs/roadmap.md, and docs/work-recall-validation.md. Start from the current feat/work-recall commit on your own orb/r02-preview-acceptance branch. First reproduce the preview scrolling mismatch: WorkUi::move_down bounds by original lines while paint_work_preview renders wrapped lines. Fix it with a shared, testable layout/scroll contract so all bounded preview text is reachable. Also test whether a late result while previewing resets the selected row on return, and whether correcting a config that failed initial load recovers without restarting. Fix only reproduced failures within R02. Preserve no-exec, generation handling, and original-source attribution. Use synthetic sources. Run targeted regressions, cargo fmt --check, cargo test --locked, and cargo clippy --locked --all-targets -- -D warnings. Update the technical docs and add a dated handoff under docs/handoffs/. Return your base/head SHA, exact changed files, why, checks, remaining gaps, and proposed roadmap/changelog updates. Do not claim physical Alt+Space acceptance from the orb. Push only your task branch when authorized; do not merge master.

These are code-review findings to reproduce, not a claim that existing tests already cover them. The preview wrap-count mismatch is visible directly in the current code; the other cases require targeted confirmation.

## First research/design-orb brief

> Read AGENTS.md and docs/local-fetch-planner.md. Work in your own orb/r06-r16-fetch-design branch; own docs/research/ and a dated docs/handoffs/ report only. Propose a versioned bounded fetch-plan/provider contract: the local model chooses fetches, deterministic code enforces scope/budgets, existing local recall is the first provider, and slow/invalid planning falls back to lexical results. Use sample plans and routing cases, not a new orchestration framework. Separately verify supported web/news, X, and profile-data access using current primary documentation. Record URLs, access/auth requirements, freshness guarantees or gaps, attribution, costs/limits, failure modes, and uncertainty. Do not imply that public X webpage visibility guarantees API access. Do not obtain credentials, provision paid services, or implement runtime adapters. Return the decision packet and proposed R06/R16 acceptance cases. Keep local generation-model choice open until Thor's available models can be evaluated.

## Review and integration loop

Every worker includes documentation with its change. To avoid shared-file conflicts, workers update owned technical docs and put status/priority proposals in their handoff; the integrator applies those proposals to the central roadmap, decision log, and changelog before accepting the branch. This preserves the requirement to document why work occurred without having two workers rewrite the same status table.

Review the diff, test the exact proposed commit, and record what was tested in which environment. On Thor, use a separate clean checkout/worktree, build locally, and verify the candidate before replacing the installed version. Do not treat a Debian-built binary as an Arch/Omarchy release artifact. Keep the previous installed version for rollback.

Minimum portable checks:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Use `--offline` after dependency preparation when appropriate. Run installer/toggle/headless scripts when those behaviors change, with isolated paths and their stated prerequisites. The full continuation sequence and acceptance gates are in [the continuation plan](continuation.md) and [roadmap](roadmap.md).
