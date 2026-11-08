# Shade Menu — shared project brain

Last reconciled: 2026-11-07. Repository: `shadetheory/menu`. This is the starting point for a person or agent taking over the project.

## What Jordan wants

The menu should make Jordan fast: recover an old idea or something he built by typing a theme, inspect the original work, and eventually take a useful next step. His initial vocabulary is **Music, Automation, Gate, Receiver, Listener, Pub/Sub**. He requested vectors within the project, a lower/more central menu, and testing against the live desktop menu.

Jordan clarified the central architecture: **the local model controls fetching**. The proposed next layer chooses providers and queries through a typed, bounded fetch plan; current embeddings only rank documents. See [local fetch planner](local-fetch-planner.md).

The later direction is **small floating apps that replace routine browser workflows**. His concrete example is typing **Elon Musk** to see recent news, recent X posts, and a useful sourced profile. This is backlog work; no web/X/profile provider is connected or chosen.

Jordan also requested a shared repo brain: important facts persist here, every meaningful change explains why, and future work has explicit priority and dependency order. [AGENTS.md](../AGENTS.md) defines the upkeep workflow.

These are requests from the project conversation on 2026-11-07. They do not establish a verified set of Jordan's “10 core values.” That earlier discovery request remains open; do not invent values or present engineering choices as personal beliefs.

## Read in this order

| Need | Primary record |
|---|---|
| Continuation diagram and Amp orb setup | [Continuation](continuation.md), [orb plan](orb-plan.md) |
| Next work, priority, dependencies, completion gates | [Roadmap](roadmap.md) |
| What changed and why | [Change history](../CHANGELOG.md) |
| Consequential choices and their authority | [Decisions](decisions.md) |
| Product interaction and intended architecture | [Menu plan](menu-plan.md) |
| Implemented recall contracts and configuration | [Work recall](work-recall.md) |
| Checks, installation evidence, unresolved native behavior | [Validation](work-recall-validation.md) |
| Code ownership and boundaries | [Architecture seams](seams.md) |
| Launch modes and operations | [Runbook](runbook.md), [installation](../INSTALL.md) |

## Current checkpoint

The `feat/work-recall` working tree contains a native Your work view, six fixed topic picks, lexical plus optional local semantic retrieval, and in-process original-source previews. Explicit file ingestion, rebuildable caches, and idempotent source-change events are implemented. A single background worker keeps retrieval off the UI thread and discards stale query responses. Source reconciliation happens on search; no filesystem listener or external pub/sub transport is wired.

The panel center is at 55% of the focused output height, clamped to that output. Jordan requested the lower position; 55% is the current engineering choice, not a user-specified number. Topic source tags are configurable; the six menu picks are currently constants in `src/recall_ui.rs`. Editable picks and aliases are planned.

The latest local install is `0.1.0-fetch-planner-20261107`, with the prior recall version retained. Its opt-in local planner is enabled; the latest installed check exercised lexical fallback after a model timeout. Recorded checks: 66 tests passed, Clippy passed, native source preview inspected, installed launcher → Music → Escape passed. **Physical Alt+Space acceptance remains unverified**: synthetic Alt+Space failed although the installed toggle worked. The duplicate dictation binding was removed from local user configuration. See the validation record for exact limits and evidence.

Jordan authorized publishing the implementation and project context to `feat/work-recall` on 2026-11-07. R03 now records the shared code checkpoint, while R03A retains the native acceptance gate. Check git history/remote state and the publication validation record for exact commit status. No GitHub issue drafts have been posted; master is not replaced by publishing this feature branch.

## Boundaries that matter

The native Rust app runs alongside Omarchy. Stock Super+Space is preserved. The personal Alt+Space toggle starts `--no-exec`. Previewing a recall result cannot execute source text. Optional embeddings use a configured loopback Ollama service; no cloud embedding service or Qdrant adapter is part of this slice. Qdrant remains a possible later adapter, not an operational dependency.

Private source files and machine configuration remain outside the public tree. Store their relevant implementation findings here, not raw conversations or private corpora. The HTML preview is a mock, not the native app. External opens, session/decision providers, agent dispatch, music playback, and floating research are future capabilities with separate completion gates.
