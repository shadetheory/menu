# Working on Shade Menu

## Start here

Read [the project brain](docs/project-brain.md), [the ordered backlog](docs/roadmap.md), and recent [change history](CHANGELOG.md) before choosing work. Inspect current code, git status, and relevant tests before acting. Chat and agent memory can provide leads; durable project facts belong in this repository.

## Record work as part of doing it

Jordan requested this on 2026-11-07: important project facts go directly into the repo; changes record why they occurred; future work is documented, prioritized, and ordered by dependencies.

For every meaningful change, including configuration, architecture, behavior, documentation, and installation:

1. Identify the roadmap item and prerequisites. Add new work to the roadmap before implementing it. Independent work may proceed while another item is blocked; record the reason for a priority change.
2. Update the relevant technical or product document in the same change. Correct conflicting statements rather than adding another contradictory note.
3. Add a dated CHANGELOG entry: **what changed, why, affected files/surfaces, verification, and remaining limits**. Describe observed results; never upgrade a plan into a completed feature.
4. Update roadmap status and acceptance evidence. Record new follow-ups with priority and dependency IDs. A blocker includes the next concrete check, not just “blocked.”
5. Record consequential choices and their rationale in `docs/decisions.md`. Distinguish Jordan's request, an engineering choice, a proposal, and an unresolved question. Supersede old decisions explicitly.
6. Before handing off, review the diff and documentation links. State what is local, committed, installed, or published. Do not claim tests, automation, hooks, or native behavior were verified without evidence.

Keep each fact in one primary document and link to it elsewhere. The project brain is the entry point, the roadmap owns future work and priority, the decision log owns rationale, and the changelog owns the history of completed changes. This is a contributor workflow; no automatic conversation capture, Git hook, or background synchronization is installed.

## Engineering boundaries

- Source documents are authoritative; vectors and caches are derived. Keep retrieved text separate from executable actions.
- Preserve launch permission behavior and the read-only recall flow. Never interpolate search text into shell commands.
- Prefer existing services and small interfaces. Receiver/listener/pub-sub vocabulary does not require a new broker.
- Keep original source attribution and honest retrieval/provider status. Similarity is relevance, not factual confidence.
- Keep credentials, private source contents, conversations, and desktop captures out of commits. Record relevant findings and reproducible checks instead.
- Run checks appropriate to the change. Documentation-only changes need a diff and link review, not a full application test rerun.
- Check `git remote -v` before committing or pushing. Local documentation updates do not authorize publishing, messaging, or pushing another repository. Preserve unrelated work.

## Finish a work item

A completed item has an implementation (when applicable), a reviewed diff, appropriate passing checks, updated rationale/history, and satisfied dependencies and acceptance criteria. If evidence is missing, retain an honest partial or blocked status. Do not hide unfinished work in a final chat response.

## Orb work

Use [the orb plan](docs/orb-plan.md) for environment preparation, task boundaries, and handoff evidence. Workers update owned technical docs and include proposed central roadmap/changelog changes in their handoff; the integrator reconciles those shared files before accepting the branch. Cloud checks cannot establish Thor's physical shortcut behavior. The next architectural direction is [local model fetch planning](docs/local-fetch-planner.md); it is not implemented merely because embeddings exist.
