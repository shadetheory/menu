# Shade Menu product and architecture plan

Status: proposed product direction. This is not a claim that the experience below is implemented.

## Purpose

Bring prior ideas and working software into the next decision. Alt+Space should let a person find a thread, inspect the original evidence, and take a deliberate next step without reconstructing their context.

The first audience is a builder with several local projects and agents. Music, Automation, Gate, Receiver, Listener, and Pub/Sub are the initial topic vocabulary. Topics are saved searches with editable aliases, not hardcoded application categories or a new taxonomy service.

## Interaction

The existing launcher remains available. Add an explicit **Your work** entry, initially ahead of app shortcuts when enabled. Its empty state shows configured topic picks and a search field labelled **Find an idea or something you built**. Keep this scoped view for the first release so stock action search and semantic retrieval cannot be confused.

Picking Music fills the query and shows original project documents, notes, and ideas. Typing searches titles, project names, aliases, and content; local semantic matching can add related results. Related terms such as playback or audio can connect without exact spelling. Each row shows title, project, source kind, and a relevant excerpt. A small state line says lexical, semantic, searching, or unavailable with a reason. A zero-result search is distinct from an unavailable source.

Enter opens a read-only source preview inside the menu. Show the original path, source date, kind, and bounded source text; support scrolling and return to the same query and selection. Escape goes preview → results → topic picks → launcher root → closed. Held Enter must not activate repeatedly. Updated search results must never inherit an old activation.

After this flow is dependable, add **Open original** through a separately reviewed typed action. Later actions may include open project, return to agent session, and draft a bounded task. Sending, running, and approving are separate actions with their own authority; a search result is never executable instructions.

Music playback is a later capability using an existing music CLI. It is distinct from searching Music-related work. Decisions and live agent status should be added only when a provider can supply current, attributable state.

## Ownership boundaries

1. **Sources** own original documents and facts. Start with an explicit list of local Markdown/text files. Do not recursively crawl a home directory or ingest accounts by default.
2. **Recall index** owns rebuildable records and optional embeddings. Store stable source IDs, paths, content fingerprints, source dates, configured kinds, and topics. A kind of implementation does not mean verified or shipped.
3. **Retrieval** combines lexical candidates and optional local semantic candidates. Exact title/project matches stay prominent. Similarity is relevance, never truth or verification confidence. Source age is visible and should not automatically bury an old useful idea.
4. **Menu presentation** owns query generation, selection, keyboard navigation, previews, and source health. It does no model/network work on the paint/input thread.
5. **Action dispatch** owns permitted side effects. It receives typed, revalidated actions, never commands copied from an indexed document.

Small JSON caches are sufficient for this first explicit corpus. Reuse the configured local embedding service. Add a Qdrant adapter when corpus size or sharing justifies it; do not introduce a broker, new daemon, or distributed database merely to search eight files.

## Receiver, listener, pub/sub

Make change events a module boundary first:

`source observed → SourceChanged → index reconciles → IndexChanged → menu refreshes`

An envelope needs a schema version, source ID, operation, revision, and fingerprint. Repeated identical events are idempotent. Removal invalidates records and derived vectors. Full source reconciliation recovers missed events and is authoritative over notification order. An in-process receiver is sufficient initially. A filesystem listener or existing house event bus can later publish the same source-change contract. Their presence cannot grant a new source permission.

## Three implementation issues

### 1. Preserve original work as source records

Explicit source configuration, bounded ingestion, provenance, atomic derived cache, idempotent change application, removal/update handling, and visible source failures. Prove that editing a file updates its record, removing it removes its result, and changing metadata changes the revision. Missing or invalid sources cannot suppress unrelated healthy results. Sources and private configuration stay outside the public repository.

### 2. Retrieve work by topic and meaning

Offline lexical search, opt-in local embeddings with a strict time budget, model/content-aware vector invalidation, and a generation-scoped background worker. Show lexical results promptly; late semantic replies cannot overwrite a newer query. Deduplicate by source identity. Tests include service outage, invalid vectors, duplicate records, renamed topics, and an old relevant idea that should remain discoverable.

### 3. Make it useful in the real Alt+Space menu

Your work entry, topic picks, results, source preview, scrolling/back navigation, truthful retrieval status, and native keyboard acceptance. Preview must work under `--no-exec` without starting a process. Test Music, Automation, Gate, and Pub/Sub against a small real local corpus and record expected sources. Retain the installed version for rollback before activating a candidate.

Existing GitHub issues #3 (truthful launch state), #4 (toggle execution policy), and #6 (native visual evidence) remain relevant release checks. #13 describes the wider unimplemented integration surface; work recall narrows one part of that direction. #1's approval covered a specific earlier commit, not all future iterations. No issue is closed by this plan.

## Acceptance and expansion

First success: open Alt+Space, choose Your work → Music, recognize a real prior artifact, inspect its source, and return without losing the query. Repeat with Automation and Gate. Modify a source and prove the result changes; disable the embedding service and prove lexical search stays useful; type rapidly and prove old results cannot replace new ones. Validate on the installed compositor and compare app-launch/no-exec behavior with the baseline.

Next: external open and project navigation. Then opt-in session/decision sources. Finally task dispatch and music controls, each as a narrow provider. A plugin marketplace, automatic task execution, generated personal summaries, and universal chat are outside the first release.

### Later: web research in floating apps

Jordan's direction: information at his fingertips, fast enough to replace routine browser search with small floating apps. Type **Elon Musk**, for example, and see a sourced profile, recent news articles, recent X posts, and relevant follow-up searches in the menu. Open an item in a focused floating detail view, then return to the same results without managing browser tabs. Keep a browser escape hatch for full sites and unsupported interactions.

This is a later feature, not part of the local recall release. Start with one person/topic research view and extend the same interaction to other useful floating apps. Provider selection for web, news, X, and profile facts needs a separate capability check; no provider has been selected or promised. See [floating research issue](issues/floating-web-research.md) for the proposed flow and acceptance criteria.

## Current implementation checkpoint

The `feat/work-recall` worktree implements source/cache/CLI code, hybrid retrieval, and a native Work view with in-process source preview. The six topic picks are currently constants; editable picks and aliases described above remain planned (R05). The authoritative priority and dependency order is in [the roadmap](roadmap.md). Live validation and a rollback-safe local installation are recorded in docs/work-recall-validation.md. The issue drafts in this directory are local; they have not been published to GitHub.
