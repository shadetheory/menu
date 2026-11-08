# Project decisions

Record date, authority, choice, reason, and consequence. “Engineering choice” means selected for the current implementation, not a personal preference attributed to Jordan. New evidence can supersede a decision with an explicit link.

| ID / date | Authority and status | Choice and why | Consequence / revisit |
|---|---|---|---|
| D01 / 2026-11-07 | Jordan request; accepted direction | Make prior ideas and work discoverable by theme, with vectors in the project. Music, Automation, Gate, Receiver, Listener, and Pub/Sub are the starting vocabulary. | Recall is the first useful product flow. See [plan](menu-plan.md). |
| D02 / 2026-11-07 | Engineering choice; implemented locally | Use an explicit file corpus, JSON derived cache, offline lexical retrieval, and optional loopback Ollama. This proves a small end-to-end flow using existing services. | No home crawl or new database. Add chunking or Qdrant only when measured corpus/retrieval needs justify it. |
| D03 / 2026-11-07 | Engineering choice; implemented locally | Keep recall records separate from action items and show source previews in process. A found document must not become executable authority. | External open and dispatch require typed, revalidated actions; preserve no-exec behavior. |
| D04 / 2026-11-07 | Jordan placement request; engineering geometry | Move the menu lower/more central. Current center is 55% of focused output height, clamped to output bounds. | Native geometry checked; revise from actual UX feedback. |
| D05 / 2026-11-07 | Engineering choice; partial foundation | Implement idempotent source-change records and reconcile on search before adding an external listener or bus. | Event vocabulary exists; no live pub/sub integration should be claimed. |
| D06 / 2026-11-07 | Jordan request; deferred feature | Add floating web research: person/topic → profile, recent news, X posts, detail/back. Reduce the effort of routine browser research. | Providers remain unselected. [Research issue](issues/floating-web-research.md) and roadmap govern implementation. |
| D07 / 2026-11-07 | Jordan request; contributor workflow established | Make the repo the shared brain. Record changes with reasons and maintain future work in priority/dependency order. | [AGENTS.md](../AGENTS.md), changelog, and roadmap must be updated with work. No automatic memory sync is claimed. |
| D08 / 2026-11-07 | Engineering repair; local desktop only | Unbind the duplicate Alt+Space dictation shortcut before binding the menu, preserving other dictation keys. | Reload/config validation passed; shortcut delivery still needs verification. Config backup and findings are in [validation](work-recall-validation.md). |

| D09 / 2026-11-07 | Jordan publication request; supersedes earlier R03 timing | Share all reviewed code and project context on the feature branch now, to support orbs. | Separate publication from R03A native/core acceptance. Master stays unchanged. |
| D10 / 2026-11-07 | Jordan architectural direction; proposed implementation | The local model controls fetching. Use a typed plan and deterministic provider execution; existing recall is the first provider. | R16 precedes floating research runtime integration. Embeddings alone are not a fetch planner. |
| D11 / 2026-11-07 | Engineering recommendation; not configured | Start with one build orb, one research/design orb, and Thor integration. Use branch-based shipping and isolated sources. | Verify setup in Amp and supported models; [orb plan](orb-plan.md) owns exact briefs and configuration. |

## Unresolved decisions

- Best web/news/X/profile providers: verify real supported access and freshness during R06; do not assume availability.
- Whether editable topics, chunk retrieval, or a shared Qdrant index should follow first: use retrieval acceptance evidence from R02.
- Jordan's ten core values and the relationship to shadetheory.dev branding: require evidence and Jordan's review. The current project principles are not a substitute for that work.
