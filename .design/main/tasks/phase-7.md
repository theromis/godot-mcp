---
phase: 7
name: "Controlled Time"
status: Todo
subsystem: "addons/godot_mcp/runtime, src/godot, src/tools, tests, README"
requires: [6]
provides: []
key_files:
  created: []
  modified: []
patterns_established: []
duration_minutes: ~
---

# Stage 7 Tasks — Controlled Time

**Phase:** 7
**Status:** Todo
**Strategic Goal:** Let an agent hold a running game still and advance it by exact or conditional physics ticks over the existing debugger channel, with input scheduled by ticks, time-driven operations refused while held, exactness reported honestly, and the hold always ending safely.

## Atomic Checklist

<!-- A line is `- [ ]` (open) or `- [x]` (done). A Blocked task keeps `- [ ]`: it is blocked by its `Status` in Detailed Tracking, and the `[!]` the workflows write after "Blocked" spells that status — it is not a checkbox state. -->

- [x] [T-7A01] Feasibility: hold loop on the physics-frame signal, channel pumped from the hold
- [ ] [T-7A02] Held step: exact and conditional ticks, exactness report
- [ ] [T-7A03] Operations while held: refusal table and the held single capture
- [ ] [T-7A04] Tick-scheduled input and the pinned visibility tick
- [ ] [T-7B01] Server side: held launch, held-step wait, debugger pause refusal, tool schemas
- [ ] [T-7C01] User-facing text: tool descriptions, README, agent guide
- [ ] [T-7D01] Carry to the release lines
- [ ] [T-7T01] Validation: pipeline, end-to-end suites and invariant coverage

## Detailed Tracking

### [T-7A01] Feasibility: hold loop on the physics-frame signal, channel pumped from the hold

- **Spec:** l2-held-stepping.md §2, §5.3, §5.4, §6 step 1; l1-controlled-time.md CT-2, CT-9
- **Status:** Done
- **Assignment:** Agent
- **Verify:** New ignored tests in `tests/runtime_e2e_test.rs`, written first and failing before the runtime change, then green with `GODOT_PATH` set to 4.8-beta1: a held game answers `godot_game_get` and `godot_game_snapshot`; a fixture's `_physics_process` counter, a `Timer` and a `PROCESS_MODE_ALWAYS` node's `_process` counter do not change across two reads 500 ms apart while held; `resume` lets them advance again; a held game stopped with `godot_stop_project` exits; closing the window of a held windowed game ends the run. `godot --headless --check-only` passes for every addon script.
- **Handoff:** T-7A02, T-7A03 and T-7A04 start only after this is Done, and run one after another: all three edit `rt_hold.gd` and `mcp_runtime.gd`.
- **Notes:** Three engine facts are unverified (the order of `physics_frame`, the close request delivered from `DisplayServer.process_events()` inside the hold, one poll per 2048 `line_poll()` calls). If any test shows a fact false and no in-spec fix exists, set this task `Blocked` with the observation; the reason is a specification gap in l2-held-stepping.md. The first hold must send `mcp:hello` when it has not gone out. New script `addons/godot_mcp/runtime/ops/rt_hold.gd`: add it to the GDScript syntax-check list in AGENTS.md and CONTRIBUTING.md (they must stay equal), and to the runtime file list the server embeds and copies for a run (the `include_str!` table used by `ensure_runtime` in `src/godot/runs.rs`).
- **Changes:** New `addons/godot_mcp/runtime/ops/rt_hold.gd` (hold loop on the physics-frame signal, channel pumped with `EngineDebugger.line_poll()`, served-request list, hold status); `mcp_runtime.gd` (close request and exit end the hold, hello sent once and from the hold point, refusal of unserved requests while held); `rt_live.gd` (`hold`, `resume`, `status` of `game.physics`); `src/godot/runs.rs` (the new file in the embedded runtime list); the new script in the syntax-check lists of AGENTS.md, CONTRIBUTING.md and ci.yml. Three ignored tests in `tests/runtime_e2e_test.rs`, red first, then green: 9 of 9 in the runtime suite on Godot 4.8-beta1, the hold tests 3 times in a row; fmt, clippy, default tests and every addon script check clean.
- **Findings:** Confirmed on a real engine: the hold stands still (physics ticks, process frames, a timer, a `PROCESS_MODE_ALWAYS` node and the engine's own frame counters did not move across 500 ms); reads, `game.call` and `game.set` are answered from the hold; `resume` lets everything run again; stopping a held run ends it; closing the window of a held windowed game (Windows, `CloseMainWindow`) ends the run, so the close request does reach the game from inside the hold. **Not as the specification says:** `EngineDebugger.is_active()` stays true after the debugger peer closes its socket (probe: a plain TCP server destroyed the connection at 2.5 s, the game printed `active=true` for the 5 s after), and the script API has no other peer check, so the game cannot learn that the server is gone. The "channel gone" clause of CT-9 and of l2-held-stepping.md's CT-9 row cannot be met as written. Candidates: a keepalive message from the server with an in-game silence limit; a lease on the hold; accepting a frozen but closable window after a hard-killed server (a graceful server exit already kills the game). The choice is a specification amendment and belongs to T-7B01 if a keepalive is chosen. Until then T-7T01 cannot map CT-9 to a test.
- **Notes (run):** The first two tests of the first run after a fresh build failed with "the game did not load the MCP runtime" (the 8 s hello wait on a cold start) and passed in 6 of 6 later runs; not caused by the hold. Held steps return an error for now (T-7A02). Clearing a pause left by the pause-based step, when a hold starts, is not done yet (T-7A02).

### [T-7A02] Held step: exact and conditional ticks, exactness report

- **Spec:** l2-held-stepping.md §5.2, §5.6; l1-controlled-time.md CT-3, CT-4, CT-5, CT-6
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** Ignored tests in `tests/runtime_e2e_test.rs`, written first: `step` with `ticks: 5` on a held game moves the fixture counter by exactly 5 and reports `ticks_run: 5`; two steps of 3 equal one of 6; `until` that already holds runs 0 ticks with `until_met: true`; `until` that never holds stops at `max` with `until_met: false`; a malformed `until` is refused with no tick run; `until` on a free game is refused with the `hold` hint; a hold entered on a free game reports `deterministic: false` with `deterministic_reason: "not_started_held"`. The pause-based step tests from pull request 18 still pass unchanged.
- **Handoff:** T-7T01 checks `deterministic: true` once T-7B01 is in.
- **Notes:** Reuse the property-wait parsing and comparison of `rt_input.gd` for `until`; do not duplicate it. `ticks` 1 to 600 when held, `max` default 600, at most 3600. A hold entered on a tree paused by the pause-based step clears that pause first.

### [T-7A03] Operations while held: refusal table and the held single capture

- **Spec:** l2-held-stepping.md §5.5; l1-controlled-time.md CT-8
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** Ignored tests in `tests/runtime_e2e_test.rs`, written first: on a held game `godot_game_wait`, `godot_game_observe`, `godot_game_screenshot` with `frames: 2` or `camera`, and `godot_game_input` with a `wait`, `text` or `swipe` step each return an error within 2 s that names the held-step replacement, and the game stays held; `godot_game_screenshot` with one frame on a held windowed game returns a PNG equal to a capture taken right before the hold; `godot_game_set` and `godot_game_call` (with `permit`) are served.
- **Handoff:** None.
- **Notes:** Refuse in `_dispatch` before a job is created: no `_pending` result may be produced while held. The windowed checks run where `runtime_e2e_test.rs` already runs its windowed screenshot test.

### [T-7A04] Tick-scheduled input and the pinned visibility tick

- **Spec:** l2-held-stepping.md §2 (Assumption), §5.5, CT-7 row of §4; l1-controlled-time.md CT-7, CT-9
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** Ignored tests in `tests/runtime_e2e_test.rs`, written first: a fixture records, per physics tick, `Input.is_key_pressed` and `is_action_just_pressed`. On a held game a `tap` followed by held steps of 1 tick each shows the press on the measured tick and the release after `hold_ticks`; the same tap followed by one step of 4 ticks gives the same per-tick record. `godot_game_input_release` and `resume` clear scheduled releases; a game stopped while holding a key leaves nothing held.
- **Handoff:** If the measured visibility tick is not 2, record the value in this task's Notes; the default `hold_ticks` becomes that value, and l2-held-stepping.md §2 needs an amendment through `/magic.spec`.
- **Notes:** Allowed held steps: `key`, `mouse_button`, `touch`, `joy_button` (press, release, tap), `click`, `click_node`, `mouse_move`, `joy_axis`, `time_scale`. Releases fall due at the tick start, before the tick runs.

### [T-7B01] Server side: held launch, held-step wait, debugger pause refusal, tool schemas

- **Spec:** l2-held-stepping.md §4 (CT-1, CT-6, CT-10 rows), §5.2, §5.6
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** `cargo test --all-targets` with new tests written first: in `tests/tools_test.rs`, `godot_run_project` with `held: true` and no `mode: "process"` is refused with that hint, and with it a stand-in engine that writes its arguments to a file (a new helper beside `fake_godot` in `tests/common/mod.rs`, which records nothing today) receives `--fixed-fps 60`, `--godot-mcp-fixed-fps=60` and `--godot-mcp-hold` for a project without the setting and the project's value when it is set; in `tests/server_test.rs`, the `godot_game_physics` schema lists `hold`, `until` and `max`. `runtime_e2e_test.rs` (ignored): `godot_debug_control` `pause` on a held process run is refused; a held step that outlives the server wait returns the "finishes and holds again" error. `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` clean.
- **Handoff:** T-7A02's `deterministic: true` case in T-7T01.
- **Notes:** Read the tick rate from `project.godot` on the host (`physics/common/physics_ticks_per_second`); only integers reach the command line (TL-5). The held-step wait is 60 s; other runtime calls keep 20 s. Keep the hold state the runtime reports on the `Run`; never hold a lock across the runtime round-trip. Runs in parallel with T-7A01: it touches `src/` and the Rust tests only, but both tasks edit `src/godot/runs.rs` (T-7A01 adds one line to the runtime file table), so whichever lands second rebases on the other.

### [T-7C01] User-facing text: tool descriptions, README, agent guide

- **Spec:** l2-held-stepping.md §6 step 4; l1-controlled-time.md §3
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** The `godot_game_physics` and `godot_run_project` descriptions in `src/tools/game.rs` and `src/tools.rs`, the README tool rows and its "Checking a running game" section, and the AGENTS.md Runtime Monitoring step describe hold, the held step, `until`, `deterministic` and the refused operations; `cargo test --all-targets` passes (the catalog tests read the descriptions); no file outside `.design/` names a design file, task ID or phase.
- **Handoff:** None.
- **Notes:** Starts after T-7A02, T-7A03, T-7A04 and T-7B01 are Done, so the text matches what was built (measured `hold_ticks` included). SECURITY.md stays true: no new channel, no evaluator, loopback only.

### [T-7D01] Carry to the release lines

- **Spec:** l2-held-stepping.md §5.7; l2-release-branches.md
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** On `godot-v4.7`, `godot-v4.6` and `godot-v4.5`, in that order, the change applied as new commits naming its origin; on each, `cargo test --all-targets`, clippy, fmt, the GDScript syntax checks, and `editor_e2e_test`, `addon_export_test` and `runtime_e2e_test` (ignored) pass with that line's Godot build from its `.github/godot-version`.
- **Handoff:** Pushing the branches needs the maintainer's yes in the conversation.
- **Notes:** No `has_method()` guard is expected (every call exists in 4.5.0); if a line's build fails on one, add the guard on that line and record it in the line's guard list.

### [T-7T01] Validation: pipeline, end-to-end suites and invariant coverage

- **Spec:** l1-controlled-time.md CT-1 to CT-10; l2-held-stepping.md §4
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** On `master` with 4.8-beta1: the full AGENTS.md pipeline (clippy, fmt, `cargo test --all-targets`, every GDScript `--check-only`), `editor_e2e_test`, `addon_export_test` and `runtime_e2e_test` with `--ignored`; a process run started with `held: true` reports `deterministic: true` and `deterministic_reason` empty; a held hold on editor Play reports `deterministic: false` (`not_started_held`); a table in this task's Notes maps every invariant CT-1 to CT-10 to the test that proves it, with no row empty.
- **Handoff:** `/magic.run` phase completion.
- **Notes:** Any invariant without a test is a failure of this task, not a note.
