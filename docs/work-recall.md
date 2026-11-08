# Work recall: first slice

Work recall is an explicit-file, read-only retrieval boundary. Menu `model::Item` remains action routing; `recall::Record` is never dispatched. Source documents are authoritative; cache is derived. No directory scanning, home crawl, credentials, conversations, generated summaries, external opens, or shell/argv execution.

## Config and CLI

Set `SHADE_RECALL_CONFIG` or use `$XDG_CONFIG_HOME/shadetheory/recall.json` (`~/.config/...` fallback). Example:

```json
{
  "enabled": true,
  "sources": [{
    "id": "music-notes", "title": "Music notes", "project": "Music",
    "path": "/absolute/path/to/notes.md", "kind": "document", "topics": ["Music"]
  }]
}
```

Only explicit UTF-8 `.md`, `.markdown`, or `.txt` regular files up to 512 KiB are accepted; symlinks and unsupported files are rejected. `kind` is document (default), decision, idea, or implementation; none imply verification. The JSON cache is `$XDG_CACHE_HOME/shadetheory/recall.json` (or `~/.cache/...`), atomically replaced and rebuildable. Run `omarchy-menu-rust --work-search 'music automation'` and `--work-inspect music-notes`. Search does a full configured-source reconcile first, so deletions invalidate cached records. Errors are visible. `--work-inspect` prints the original source contents, not a generated digest.

## Record and change envelope

Record schema v1: `id`, `title`, `project`, canonical `source_path`, full bounded source `text`, `modified_unix`, `fingerprint`, explicit `kind`, optional `topics`. Fingerprints are deterministic FNV-1a 64-bit over bytes (change detection, not security). Reconcile emits deterministic upsert/remove events with `source_id`, revision/fingerprint and applies them idempotently against the cache. Full reconcile is recovery authority. Exact lexical search weights title > project > topic > body, sorts ties by source ID, and caps results at 50. Lexical search is offline. Optional local semantic retrieval adds related results while preserving strong lexical matches.

## Scope / remaining

This slice adds source ingestion, a rebuildable cache, hybrid retrieval, and a native Your work view. A background worker publishes lexical results before local semantic results; generation checks reject stale replies. Enter previews the source in process; arrows scroll and Escape returns. External open, agent dispatch, account ingestion, and filesystem listeners remain future work. Sources are reconciled during searches. The panel center sits at 55% of the focused output height, clamped to that output. Existing launch/no-exec behavior remains unchanged.

Issue drafts: [source/index contract](issues/work-recall-source.md), [retrieval](issues/work-recall-retrieval.md), [menu workflow](issues/work-recall-menu.md). Related existing issues: #3/#4 for launch state and policy, #6 for native evidence, #13 for future integrations.

## Local semantics

Add `"ollama": {"enabled": true, "endpoint": "http://127.0.0.1:11434", "model": "bge-large:latest"}` to the configuration to opt in. Only loopback HTTP endpoints are accepted. Each request has a bounded deadline; unavailable embeddings fall back to lexical retrieval. Source vectors are cached by content, endpoint, and model. Documents are bounded and their embedding input is truncated; this first version retrieves documents, not individual chunks. No source text is sent to a cloud embedding service.

## Optional local fetch planning

A separate `planner` configuration object (`enabled`, loopback `endpoint`, generation `model`) enables a bounded query-planning step before local provider retrieval. `ollama` still configures embeddings. Native search publishes original lexical results first. Planner failures use original-query lexical fallback. See [the planner contract and current limits](local-fetch-planner.md). No persistent configuration is enabled automatically.
