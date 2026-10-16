# Workspace Phase Journal

Internal journal of finished phases. User-facing release notes are in the root `CHANGELOG.md`.

## Phase 3 — 2026-10-16

- Scrubber, fingerprint, percent-encoding and link builder (`src/feedback.rs`), with a mutation check on every scrub step.
- In-memory journal of notes, evidence, counters and drafts with a Pending / Submitting / Submitted state machine and a fenced report builder (`src/journal.rs`).
- GitHub CLI runner with duplicate look-up, fixed argument vectors and outcome mapping (`src/issues.rs`); stand-in `gh` example target and test helper.
- Settings `GODOT_MCP_FEEDBACK` and `GODOT_MCP_GH_PATH`; catalog and guidance by mode; outcome and unknown-tool recording that never alters a result.
- Tools `godot_note_feedback`, `godot_draft_feedback` and `godot_submit_feedback` (consent flag, concurrency and cancellation safe).
- README, configuration template, changelog entry, agent protocol item and a manual-path issue template.
- Found by the tests: a project under the home directory would have shown its folder name; the home directory is now shortened only once the project is known.
