# Project Context

**Generated:** 2026-10-21

## Active Technologies

- Rust

## Core Project Structure

```plaintext
.
├── .agents/
│   ├── rules/
│   ├── skills/
│   └── workflows/
├── .design/
│   ├── .version
│   ├── INDEX.md
│   ├── RULES.md
│   ├── main/
│   └── workspace.json
├── .env.example
├── .gitattributes
├── .github/
│   ├── ISSUE_TEMPLATE/
│   ├── PULL_REQUEST_TEMPLATE.md
│   ├── dependabot.yml
│   ├── godot-version
│   ├── lines
│   ├── scripts/
│   ├── steward-priorities.md
│   ├── steward.yml
│   └── workflows/
├── .gitignore
├── .magic/
├── .markdownlint-cli2.jsonc
├── .vscode/
├── AGENTS.md
├── CHANGELOG.md
├── CONTRIBUTING.md
├── Cargo.lock
├── Cargo.toml
├── LICENSE
├── README.md
├── SECURITY.md
├── addons/
│   └── godot_mcp/
├── docs/
│   └── feature-parity.md
├── examples/
│   └── fake_gh.rs
├── project.godot
├── src/
│   ├── bridge.rs
│   ├── cli_runner.rs
│   ├── config.rs
│   ├── error.rs
│   ├── feedback.rs
│   ├── godot/
│   ├── issues.rs
│   ├── journal.rs
│   ├── lib.rs
│   ├── lsp/
│   ├── main.rs
│   ├── paths.rs
│   ├── protocol.rs
│   ├── server.rs
│   ├── tools/
│   ├── tools.rs
│   ├── util.rs
│   └── validation.rs
└── tests/
    ├── addon_export_test.rs
    ├── bridge_client_test.rs
    ├── common/
    ├── detach_test.rs
    ├── editor_e2e_test.rs
    ├── feedback_test.rs
    ├── runtime_e2e_test.rs
    ├── server_test.rs
    ├── stdio_protocol_test.rs
    ├── steward_test.rs
    ├── tools_test.rs
    └── version_sync_test.rs
```

## Recent Changes

- Held steps: an exact number of ticks, or until a public property satisfies a condition (no expression, no method call), bounded by `max`; the answer says whether stepping is exact and why not.
- Input for a held game by tick: taps last `hold_ticks` ticks (default 2, measured: just pressed on the second tick), releases fall due at a tick start, ending a hold lets go of everything.
- Operations that need time to pass are refused while held with the step that replaces them; a single capture shows the last drawn frame.
- `godot_run_project` `held`: a game started held at a fixed frame rate equal to the project's tick rate, so its steps are exact; the server keeps a held step's 60 s wait and refuses a debugger `pause` on a held game.
- Found by the tests: a step that did not count the tick it was started on ran one tick too many; the engine keeps `EngineDebugger.is_active()` true after the server's socket closes, so a vanished server cannot be detected from the game (a recorded limitation, with a keepalive as the deferred alternative); a screenshot test that moved one pixel passed only when a counter wrapped.
- Carried to `godot-v4.7`, `godot-v4.6` and `godot-v4.5` as one commit each, verified with each line's pinned build.

### Phase 5 — 2026-10-21

- The portable repo-steward skill: scripts for context, profile, labels, readiness, board and the one way to write to GitHub, instructions and references for triage, replies, pull requests, changes, board, readiness and setup, and templates.
- Installed here with its profile, priorities, workflows (readiness, stale reminders, an agent run that stays off until switched on), issue forms and pull request template; contributor and agent guides describe it; `tests/steward_test.rs` guards the contracts.
- Labels and a public board with fields, views and policies; a release-readiness notice kept current by a workflow.
- Attended acting runs reviewed the contributor series and merged it with the review fixes, carried each change to the release lines, and made the 0.6.0 release at the maintainer's request; a later run posted the released message on the fixed issue and proved that a second pass posts nothing.
- A ruleset now protects `master`; head branches are deleted after merge; first-time contributors need approval for workflow runs.
- Found along the way: a native Windows `gh` cannot open a POSIX temporary path, so the action script translates it; GitHub reserves the field name Type, so the board field is Kind; the token needs the project scope for board steps, and a run without it skips them and says so.
