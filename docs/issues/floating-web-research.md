# Later: fast web research through floating apps

Status: local backlog draft, requested by Jordan. Not implemented or published.

## Outcome

Make web research available from Alt+Space without a pile of browser tabs. Information at Jordan's fingertips: a query opens a small useful app, with keyboard navigation and fast return to his work.

## First flow

1. Type **Elon Musk** and choose web research. Make the online scope clear before submitting; local recall queries remain local.
2. Show a compact sourced profile, recent news articles, and recent X posts. Resolve ambiguous names before combining sources about different people.
3. Show publisher or account, publication time, retrieval time, and source link. Label cached results and unavailable providers honestly.
4. Enter opens a floating detail view. Escape returns to the same query, selected result, and scroll position. Allow related searches, copying a link, and opening the original site.
5. Keep results usable as providers finish independently. A slow X provider must not block news or the profile.

The product direction is broader than people search: replace routine browser workflows with focused floating apps. This issue proves the research interaction first.

## Architecture boundaries

- Use separate web/news, X, and profile providers behind a shared result contract. Verify supported access, freshness, and attribution before choosing services; decide the best profile source during implementation.
- Results carry canonical URL, provider/source ID, result kind, entity identity where known, published/retrieved timestamps, excerpt, and attribution. A profile assertion links to supporting evidence.
- Reuse the menu's query generation, stale-response rejection, preview/back navigation, and typed action boundaries. Run requests outside the input/render thread with cancellation and time budgets.
- Treat posts and articles as source material. Keep posts distinguishable from reported news and sourced profile facts; do not present generated profile claims without evidence.
- Keep transient web results separate from the personal recall corpus. Saving an item to recall is an explicit action. Never send local source text to a web provider implicitly.

## Acceptance

- The example query produces independently attributable profile, news, and X sections when their providers are available.
- Time filters and timestamps make “recent” inspectable; unavailable access is distinct from no results.
- One failed provider leaves other sections usable. Late responses cannot overwrite a newer query.
- A keyboard-only search → detail → back cycle preserves context. Measure time to first useful result and the number of steps against the equivalent browser workflow.
- Opening a full website remains available when the floating view cannot complete the task.

## Sequence

Finish and verify local recall first. Then implement one vertical research flow with real providers and native acceptance evidence before expanding into additional floating apps.
