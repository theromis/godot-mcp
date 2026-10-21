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

## Phase 7 — 2026-10-21

- Hold a running game at the start of a physics tick (`rt_hold.gd`): nothing advances, even in nodes that ignore pause, while reads, `set` and `call` with `permit` are still answered; the debugger channel is driven from the hold with `EngineDebugger.line_poll()`.
- Held steps: an exact number of ticks, or until a public property satisfies a condition (no expression, no method call), bounded by `max`; the answer says whether stepping is exact and why not.
- Input for a held game by tick: taps last `hold_ticks` ticks (default 2, measured: just pressed on the second tick), releases fall due at a tick start, ending a hold lets go of everything.
- Operations that need time to pass are refused while held with the step that replaces them; a single capture shows the last drawn frame.
- `godot_run_project` `held`: a game started held at a fixed frame rate equal to the project's tick rate, so its steps are exact; the server keeps a held step's 60 s wait and refuses a debugger `pause` on a held game.
- Found by the tests: a step that did not count the tick it was started on ran one tick too many; the engine keeps `EngineDebugger.is_active()` true after the server's socket closes, so a vanished server cannot be detected from the game (a recorded limitation, with a keepalive as the deferred alternative); a screenshot test that moved one pixel passed only when a counter wrapped.
- Carried to `godot-v4.7`, `godot-v4.6` and `godot-v4.5` as one commit each, verified with each line's pinned build.

## Phase 5 — 2026-10-21

- The portable repo-steward skill: scripts for context, profile, labels, readiness, board and the one way to write to GitHub, instructions and references for triage, replies, pull requests, changes, board, readiness and setup, and templates.
- Installed here with its profile, priorities, workflows (readiness, stale reminders, an agent run that stays off until switched on), issue forms and pull request template; contributor and agent guides describe it; `tests/steward_test.rs` guards the contracts.
- Labels and a public board with fields, views and policies; a release-readiness notice kept current by a workflow.
- Attended acting runs reviewed the contributor series and merged it with the review fixes, carried each change to the release lines, and made the 0.6.0 release at the maintainer's request; a later run posted the released message on the fixed issue and proved that a second pass posts nothing.
- A ruleset now protects `master`; head branches are deleted after merge; first-time contributors need approval for workflow runs.
- Found along the way: a native Windows `gh` cannot open a POSIX temporary path, so the action script translates it; GitHub reserves the field name Type, so the board field is Kind; the token needs the project scope for board steps, and a run without it skips them and says so.

## Phase 8 — 2026-10-21

- Feasibility on a real engine first (Godot 4.8-beta1): the script editor exists without a window, unsaved typing is the code editor's version differing from its saved version, a fragment replacement is one undo step, and saving gives a running tool script the new code. Found false: no call names an editor's resource, so editors are paired with paths through the two lists the script editor offers.
- `godot_read_script`: the text the editor would save (unsaved typing included), where it came from, whether it is unsaved, and a version label (64-bit FNV-1a, the same in the addon and the host). `godot_edit_script`: one fragment that must occur exactly once, an optional version check, one undoable editor action that saves, refused while the editor holds unsaved changes to the script. File fallback through an exclusive temporary file; line endings and byte order mark kept.
- Specifications: Content Authoring 1.1.0 (CA-7 to CA-10), Godot Editor Addon 1.4.2, Tool Handlers 1.3.2, Tool Catalog 1.4.0.
- Found by carrying: Godot 4.5.2, 4.6.3 and 4.7.2 give script tabs no `_tab_name` metadata; the pairing check now applies only where it exists. Found by the security review: the edit's temporary file is now created exclusively under an unpredictable name and a script that is a symbolic link is refused.
- Carried to `godot-v4.7`, `godot-v4.6` and `godot-v4.5` as three commits each, verified with each line's pinned build.
