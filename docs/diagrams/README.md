# Continuation diagram evidence

The GitHub-readable dependency diagram is in [continuation.md](../continuation.md). The standalone [interactive HTML](continuation.html) shows the worker/integrator loop; download it or open it from a clone. [Specification](continuation.workflow.json) and [browser receipt](continuation.visual-check.json) are included so a future worker can reproduce and improve it.

Generated with the local Archify skill, version 2.17, on 2026-11-07. Arrows without labels join consecutive steps whose endpoints already express the operation: build → checks and provider study → evidence. No protocol or additional action is omitted.

```text
diagram_type: workflow
output: docs/diagrams/continuation.html
specification_sha256: 073b0ba373d4e510c4ca4bc5b357d79e2d39eec2db33845bc0b68017f034c091
artifact_sha256: c5165387bfa3a0f978c92c5ceffbd420fcc44e226c322fa751e6cbb0f86fb706
validation: 9/9 showcase, 0 composition errors, 0 warnings
browser_evidence: failed
visual_review: failed (first-screen desktop fit)
correction_rounds: 2
```

Deterministic diagram checks passed. Headless Chromium measured all four required viewports and captured light/dark evidence. The final HTML needs vertical scrolling at 1440×900, 1600×1000, and 1920×1080; 2048×1320 fits. This is an unresolved presentation limitation, not a passing first-screen acceptance claim. Screenshot inspection confirmed readable nodes and separated research arrows at the largest size. The earlier layout fit smaller viewports but had an ambiguous branch corridor and excessive empty space; the final artifact favors unambiguous routing and records the remaining fit defect.

Continue any layout repair under R00 without changing the workflow meaning. Prefer the Mermaid diagram in the continuation document for compact GitHub reading. No native-menu behavior is tested by these browser screenshots.
