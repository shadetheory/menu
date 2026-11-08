# Project change history

Record completed changes here with their reasons and evidence. Future work and priority belong in [the roadmap](docs/roadmap.md). Dates are work dates, not release claims. Entries below reconstruct the current session from the working tree and recorded checks; earlier history remains in git.

## 2026-11-07 — R03: implementation checkpoint for orb continuation

**Why:** Jordan explicitly requested pushing the implementation and all relevant project context so orbs can continue. This is a feature-branch checkpoint, not a release signoff.

**Changed:** Committed the previously local recall implementation, dependency metadata, and lower panel placement described in the earlier R01/R02 entries. No runtime behavior changed during this publication review.

**Verification:** Re-ran `cargo test --offline --locked` (64 binary tests + 2 integration tests), `cargo clippy --offline --locked --all-targets -- -D warnings`, and `cargo fmt --check`; all passed. Reviewed the actual diff. Fresh-checkout build and publication evidence will be recorded in the validation document.

**Limits:** Native shortcut acceptance remains open. Review found a wrapped-line versus source-line preview scrolling mismatch; the first orb task must reproduce and fix it. Late-preview selection and startup-config recovery also need targeted regressions. Publication does not make these gaps complete.

## 2026-11-07 — R00: shared repo brain and maintenance workflow

**Why:** Jordan requested that important project facts persist directly in the repo, every change explain why it occurred, and future work be documented in dependency and priority order.

**Changed:** Added `AGENTS.md`, the project brain, decision log, ordered roadmap, and this change history. Linked existing product/technical/validation/issue docs into the starting point. Corrected stale README launch-mode/outside-click descriptions and updated the architecture map for recall and Wayland presentation. Clarified that topic picks are currently fixed despite the planned editable-topic experience.

**Verification:** Reviewed against the relevant code and session evidence; 61 local Markdown links checked across 22 files; all 15 roadmap items have valid dependency references and no dependency cycles; `git diff --check` passed. No application behavior changed in this cycle. Existing implementation changes remain separate local work; this checkpoint does not publish or release them.

**Limits:** This is a required contributor workflow, not an automatic capture service or installed Git hook. Native shortcut acceptance remains open (R01). Unrelated personal memory is not copied into the project.

## 2026-11-07 — R01/R02: first local work-recall implementation

**Why:** Jordan asked to build the foundation for finding his old ideas and projects by theme, with vectors within the project, and to test the live menu.

**Changed:** Added `src/recall.rs` (explicit sources, source-change records, atomic cache, lexical/local semantic retrieval), `src/recall_ui.rs` (background worker, query generations, preview state), CLI entry points in `src/main.rs`, native GUI integration in `src/gui.rs` and `src/present_wayland.rs`, and serde dependency metadata. Updated recall docs and drafted three local issues. `src/bots.rs` has a formatting-only change from formatting the crate.

**Verification:** Recorded 64 binary tests and 2 integration tests passing, Clippy passing, real-corpus CLI queries, and native preview inspection. See [validation](docs/work-recall-validation.md) for exact coverage and gaps.

**Limits:** At this original checkpoint implementation was present in the working tree and locally installed but uncommitted; the later R03 entry records its code checkpoint. Source previews are read-only. Topic picks are constants. No external-open, listener, task dispatch, or shared Qdrant service is wired. Semantic relevance still needs acceptance against a wider corpus.

## 2026-11-07 — R01: lower placement and local installation

**Why:** Jordan requested a lower, more central menu and authorized native desktop testing. Testing also revealed duplicate Alt+Space menu/dictation bindings.

**Changed:** `src/place.rs` places the panel center at 55% output height and clamps it to the output. Updated `share/version`, installed a new version under the existing user prefix, and retained the prior version. Backed up the local Hyprland user bindings and explicitly unbound Alt+Space before the existing menu binding; other dictation shortcuts remain.

**Verification:** Geometry checked on the active output; Hyprland reload/config validation passed; installed toggle launched the expected executable, showed Music results, and closed with Escape.

**Limits:** Synthetic Alt+Space still failed; physical key acceptance is unverified. Machine configuration and screenshots stay local. [Validation and recovery](docs/work-recall-validation.md) record the installation, binary digest, backup, and next unresolved check.

## 2026-11-07 — R06/R07: floating research added to later work

**Why:** Jordan wants information at his fingertips through small floating apps that replace routine browser searches. His example: Elon Musk → recent news, recent X posts, and a profile.

**Changed:** Added the flow and architecture boundaries to `docs/menu-plan.md` and `docs/issues/floating-web-research.md`; now tracked with discovery before implementation in the roadmap.

**Verification:** Compared the documented flow to Jordan's request; no provider integration or web behavior is claimed.

**Limits:** Providers, access, and freshness are unresolved. This records future work only.
