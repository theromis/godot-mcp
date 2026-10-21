---
phase: 7
name: "Controlled Time"
status: In Progress
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
**Status:** In Progress
**Strategic Goal:** Let an agent hold a running game still and advance it by exact or conditional physics ticks over the existing debugger channel, with input scheduled by ticks, time-driven operations refused while held, exactness reported honestly, and the hold always ending safely.

## Atomic Checklist

<!-- A line is `- [ ]` (open) or `- [x]` (done). A Blocked task keeps `- [ ]`: it is blocked by its `Status` in Detailed Tracking, and the `[!]` the workflows write after "Blocked" spells that status — it is not a checkbox state. -->

- [x] [T-7A01] Feasibility: hold loop on the physics-frame signal, channel pumped from the hold
- [x] [T-7A02] Held step: exact and conditional ticks, exactness report
- [x] [T-7A03] Operations while held: refusal table and the held single capture
- [x] [T-7A04] Tick-scheduled input and the pinned visibility tick
- [x] [T-7B01] Server side: held launch, held-step wait, debugger pause refusal, tool schemas
- [x] [T-7C01] User-facing text: tool descriptions, README, agent guide
- [x] [T-7T01] Validation: pipeline, end-to-end suites and invariant coverage
- [ ] [T-7D01] Carry to the release lines (after validation)

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
- **Status:** Done
- **Assignment:** Agent
- **Verify:** Ignored tests in `tests/runtime_e2e_test.rs`, written first: `step` with `ticks: 5` on a held game moves the fixture counter by exactly 5 and reports `ticks_run: 5`; two steps of 3 equal one of 6; `until` that already holds runs 0 ticks with `until_met: true`; `until` that never holds stops at `max` with `until_met: false`; a malformed `until` is refused with no tick run; `until` on a free game is refused with the `hold` hint; a hold entered on a free game reports `deterministic: false` with `deterministic_reason: "not_started_held"`. The pause-based step tests from pull request 18 still pass unchanged.
- **Handoff:** T-7T01 checks `deterministic: true` once T-7B01 is in.
- **Notes:** Reuse the property-wait parsing and comparison of `rt_input.gd` for `until`; do not duplicate it. `ticks` 1 to 600 when held, `max` default 600, at most 3600. A hold entered on a tree paused by the pause-based step clears that pause first.
- **Attempts:**
  - A step that returned from the hold loop without counting the tick it was started on → ran one tick more than asked (5 asked, 6 moved); the tick at the hold point is the first tick of the step, so leaving the hold now decides for that tick through the same check as every later tick start.
- **Changes:** `rt_hold.gd` (step counter, condition read before every tick, exactness with `deterministic_reason`, bounds 1 to 600 ticks and 1 to 3600 `max`), `rt_input.gd` (`parse_condition` and `read_condition` share the wait's lookup and comparison, private names refused before any tick), `rt_live.gd` (routing; a pause the pause-based step left is cleared by `hold`; `until` on a free game refused with the hold hint). Three more ignored tests, red first, then green; runtime suite 12 of 12; fmt, clippy and every script check clean.
- **Notes (run):** The launch-time half of `deterministic` (`--godot-mcp-hold`, `--godot-mcp-fixed-fps=`) is read here and set by T-7B01; arming a hold at the first tick for a held launch is part of T-7B01. Nothing asserts `deterministic: true` yet.

### [T-7A03] Operations while held: refusal table and the held single capture

- **Spec:** l2-held-stepping.md §5.5; l1-controlled-time.md CT-8
- **Status:** Done
- **Assignment:** Agent
- **Verify:** Ignored tests in `tests/runtime_e2e_test.rs`, written first: on a held game `godot_game_wait`, `godot_game_observe`, `godot_game_screenshot` with `frames: 2` or `camera`, and `godot_game_input` with a `wait`, `text` or `swipe` step each return an error within 2 s that names the held-step replacement, and the game stays held; `godot_game_screenshot` with one frame on a held windowed game returns a PNG equal to a capture taken right before the hold; `godot_game_set` and `godot_game_call` (with `permit`) are served.
- **Handoff:** None.
- **Notes:** Refuse in `_dispatch` before a job is created: no `_pending` result may be produced while held. The windowed checks run where `runtime_e2e_test.rs` already runs its windowed screenshot test.
- **Changes:** `rt_hold.gd` `refusal()` (the reason and the held replacement for each refused request, empty when served), `mcp_runtime.gd` dispatch uses it before any job is created, `rt_view.gd` captures the last drawn frame at once for a single-frame capture of a held game. Two ignored tests; runtime suite 14 of 14.
- **Notes (run):** The refusal test passed from the start, because the first task already refused everything not on its list; it pins the table so that serving more later cannot loosen it unnoticed. The capture check compares two held captures (no pixel changed) and a capture after three held ticks (pixels changed) instead of a capture taken before the hold, which a running game cannot match. `input.run` is refused as a whole until T-7A04 serves its instant steps.

### [T-7A04] Tick-scheduled input and the pinned visibility tick

- **Spec:** l2-held-stepping.md §2 (Assumption), §5.5, CT-7 row of §4; l1-controlled-time.md CT-7, CT-9
- **Status:** Done
- **Assignment:** Agent
- **Verify:** Ignored tests in `tests/runtime_e2e_test.rs`, written first: a fixture records, per physics tick, `Input.is_key_pressed` and `is_action_just_pressed`. On a held game a `tap` followed by held steps of 1 tick each shows the press on the measured tick and the release after `hold_ticks`; the same tap followed by one step of 4 ticks gives the same per-tick record. `godot_game_input_release` and `resume` clear scheduled releases; a game stopped while holding a key leaves nothing held.
- **Handoff:** If the measured visibility tick is not 2, record the value in this task's Notes; the default `hold_ticks` becomes that value, and l2-held-stepping.md §2 needs an amendment through `/magic.spec`.
- **Notes:** Allowed held steps: `key`, `mouse_button`, `touch`, `joy_button` (press, release, tap), `click`, `click_node`, `mouse_move`, `joy_axis`, `time_scale`. Releases fall due at the tick start, before the tick runs.
- **Changes:** `rt_input.gd` (`parse_steps`, `apply_held_step`, `release_due`, `release_everything`; the schedule is cleared with everything else a release clears), `rt_hold.gd` (`run_input` with `hold_ticks` 1 to 600, default 2; refusal of `wait`, `text` and `swipe` steps; scheduled releases fall due at the start of a tick; ending a hold lets go of all input), `mcp_runtime.gd` (a held game runs the held version of a method). Two ignored tests; runtime suite 16 of 16.
- **Findings:** Measured on Godot 4.8-beta1 and now pinned by `held_tap_lasts_its_ticks_whatever_the_step_sizes`: a press made at the hold point is down from the first tick that follows and is reported as just pressed on the second, so the default of 2 ticks stands and the assumption in the specification is confirmed. A tap gives the same per-tick record whether the ticks come one step at a time or in one step.
- **Notes (run):** The windowed capture test of T-7A03 moved the drawn square by `process_frames % 50` pixels, which at a 128-pixel capture is under one pixel and changed the image only when the counter wrapped; it passed by luck once and failed five times in a row. The fixture now moves 40 pixels per frame (period 20).

### [T-7B01] Server side: held launch, held-step wait, debugger pause refusal, tool schemas

- **Spec:** l2-held-stepping.md §4 (CT-1, CT-6, CT-10 rows), §5.2, §5.6
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo test --all-targets` with new tests written first: in `tests/tools_test.rs`, `godot_run_project` with `held: true` and no `mode: "process"` is refused with that hint, and with it a stand-in engine that writes its arguments to a file (a new helper beside `fake_godot` in `tests/common/mod.rs`, which records nothing today) receives `--fixed-fps 60`, `--godot-mcp-fixed-fps=60` and `--godot-mcp-hold` for a project without the setting and the project's value when it is set; in `tests/server_test.rs`, the `godot_game_physics` schema lists `hold`, `until` and `max`. `runtime_e2e_test.rs` (ignored): `godot_debug_control` `pause` on a held process run is refused; a held step that outlives the server wait returns the "finishes and holds again" error. `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` clean.
- **Handoff:** T-7A02's `deterministic: true` case in T-7T01.
- **Notes:** Read the tick rate from `project.godot` on the host (`physics/common/physics_ticks_per_second`); only integers reach the command line (TL-5). The held-step wait is 60 s; other runtime calls keep 20 s. Keep the hold state the runtime reports on the `Run`; never hold a lock across the runtime round-trip. Runs in parallel with T-7A01: it touches `src/` and the Rust tests only, but both tasks edit `src/godot/runs.rs` (T-7A01 adds one line to the runtime file table), so whichever lands second rebases on the other.
- **Attempts:**
  - Stand-in engine that wrote its arguments with `echo %*> file` in a batch file → the last argument is a port number, and a digit before `>` is a handle redirect; a space was added.
  - Tests that dropped the tool manager at the end of the statement that called it → the manager kills the games it started when dropped, so the stand-in died before writing; the manager now lives until the file is read.
- **Changes:** `src/godot/runs.rs` (`start` takes `held`: `--fixed-fps R` for the project's tick rate read from `project.godot`, default 60, and `--godot-mcp-fixed-fps=R --godot-mcp-hold` after `--`; the run remembers whether the game is held and reports it in its status), `src/tools.rs` (`held` refused without `mode: "process"`; schema), `src/tools/game.rs` (a held step waits 60 s; a timeout says the game finishes the step and holds again; the answer to a physics call keeps the held flag current; `pause` refused on a held run; schemas for `hold`, `until`, `max`, `hold_ticks`), `src/godot/mod.rs` (exports `Run`); runtime: `arm_at_launch` holds a game started held at its first tick, the status carries `deterministic`, `deterministic_reason` and `total_ticks`. New: a stand-in engine that records its arguments in `tests/common/mod.rs`; five tests in `tests/tools_test.rs`, a unit test of the tick-rate reading and one of the timeout message, and `a_held_launch_steps_exactly_and_the_server_knows_it_is_held` (ignored). All green: default suite, clippy, fmt, runtime suite 17 of 17, every script check.
- **Notes (run):** The Verify line asked for an ignored end-to-end test of a held step outliving the server wait. A real 60 s wait in the suite is too slow and a shorter bound would be a setting that exists only for the test, so the message is covered by a unit test and the wait length by reading. `deterministic: true` is now proved on a held launch (10 ticks, 10 frames); the editor Play case is for T-7T01. The silence limit for a vanished server (CT-9, see T-7A01) was not added here: it needs the specification amended first.

### [T-7C01] User-facing text: tool descriptions, README, agent guide

- **Spec:** l2-held-stepping.md §6 step 4; l1-controlled-time.md §3
- **Status:** Done
- **Assignment:** Agent
- **Verify:** The `godot_game_physics` and `godot_run_project` descriptions in `src/tools/game.rs` and `src/tools.rs`, the README tool rows and its "Checking a running game" section, and the AGENTS.md Runtime Monitoring step describe hold, the held step, `until`, `deterministic` and the refused operations; `cargo test --all-targets` passes (the catalog tests read the descriptions); no file outside `.design/` names a design file, task ID or phase.
- **Handoff:** None.
- **Notes:** Starts after T-7A02, T-7A03, T-7A04 and T-7B01 are Done, so the text matches what was built (measured `hold_ticks` included). SECURITY.md stays true: no new channel, no evaluator, loopback only.
- **Changes:** Descriptions of `godot_game_physics`, `godot_game_input`, `godot_game_wait`, `godot_game_observe`, `godot_game_screenshot`, `godot_debug_control` and `godot_run_project`; a "Holding a game" section and two table rows in README.md; a bullet in the Runtime Monitoring step of AGENTS.md. A new test in `tests/tools_test.rs` (red first) reads the descriptions for hold, until, deterministic, the reason names, and the refusals. No design-file, task or phase reference in any product file (scanned).
- **Notes (run):** The README states what was measured: the server vanishing without stopping the run leaves the game held with a window that can be closed. That sentence has to change if the silence limit of T-7A01 is added. SECURITY.md is unchanged and true: no new channel, no evaluator, loopback only.

### [T-7T01] Validation: pipeline, end-to-end suites and invariant coverage

- **Spec:** l1-controlled-time.md CT-1 to CT-10; l2-held-stepping.md §4
- **Status:** Done
- **Assignment:** Agent
- **Verify:** On `master` with 4.8-beta1: the full AGENTS.md pipeline (clippy, fmt, `cargo test --all-targets`, every GDScript `--check-only`), `editor_e2e_test`, `addon_export_test` and `runtime_e2e_test` with `--ignored`; a process run started with `held: true` reports `deterministic: true` and `deterministic_reason` empty; a held hold on editor Play reports `deterministic: false` (`not_started_held`); a table in this task's Notes maps every invariant CT-1 to CT-10 to the test that proves it, with no row empty.
- **Handoff:** `/magic.run` phase completion.
- **Notes:** Any invariant without a test is a failure of this task, not a note.
- **Was blocked:** CT-9 promised that a vanished server ends the hold, detected through `EngineDebugger.is_active()`; measured on Godot 4.8-beta1 (T-7A01), `is_active()` stays true after the debugger peer closes its socket, so the game cannot tell. Resolved by amending both specifications to 1.1.0: CT-9 now states what holds (release, stop, window closed, component gone) and records the limitation, with a keepalive as a deferred alternative and its override. The CT-9 row of the table below follows the new text; there is no channel-gone clause left to test.
- **Verified (Godot 4.8-beta1, `master`):** `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets`, `--check-only` of all twelve addon scripts; `editor_e2e_test` 9 of 9, `addon_export_test` 2 of 2, `runtime_e2e_test` 20 of 20. `deterministic: true` on a held launch (10 ticks, 10 frames); `false` with `not_started_held` on a later hold and on editor Play.
- **Coverage of the invariants:**

  | Invariant | Proved by |
  | --- | --- |
  | CT-1 opt-in | `held_game_stands_still_serves_reads_and_resumes` (resume runs freely again), `headless_game_waits_for_a_value_a_new_scene_and_real_physics_ticks` (the pause-based step unchanged), `held_step_stops_on_a_condition_and_is_bounded` (`until` refused on a free game), `a_run_that_is_not_held_gets_no_fixed_rate_and_no_game_arguments` |
  | CT-2 held between requests | `held_game_stands_still_serves_reads_and_resumes`, `held_windowed_game_captures_the_last_drawn_frame`, `editor_play_can_be_held_and_stepped_but_is_not_exact` |
  | CT-3 exact ticks | `held_step_runs_exactly_the_ticks_asked_and_reports_exactness`, `a_held_launch_steps_exactly_and_the_server_knows_it_is_held` |
  | CT-4 conditions | `held_step_stops_on_a_condition_and_is_bounded` |
  | CT-5 exactness | `a_held_launch_steps_exactly_and_the_server_knows_it_is_held` (true), `held_step_runs_exactly_the_ticks_asked_and_reports_exactness` (`not_started_held`), `exactness_is_withdrawn_when_the_game_changes_its_tick_rate`, `exactness_is_withdrawn_when_frames_stop_following_ticks_and_stays_withdrawn` |
  | CT-6 time-bounded | `a_held_step_that_times_out_is_still_running_in_the_game` (the message); the 60 s wait is covered by reading only |
  | CT-7 tick-scheduled input | `held_tap_lasts_its_ticks_whatever_the_step_sizes` (also pins the measured visibility tick), `held_input_is_released_when_asked_and_when_the_hold_ends` |
  | CT-8 refusals | `held_game_refuses_every_operation_that_needs_time_and_serves_the_rest`, `held_game_refuses_what_needs_time_to_pass_and_stays_held` |
  | CT-9 ends safely | stop: `held_game_stands_still_serves_reads_and_resumes`; window closed: `closing_the_window_of_a_held_game_ends_the_run` (Windows); input released: `held_input_is_released_when_asked_and_when_the_hold_ends`. A vanished server is a recorded limitation, not an invariant (CT-9, 1.1.0). |
  | CT-10 debugging | `a_breakpoint_hit_during_a_held_step_stops_the_game_as_usual`, `a_held_launch_steps_exactly_and_the_server_knows_it_is_held` (pause refused). The editor debugger's own pause during a hold is the documented limitation and is not tested. |

- **Notes (run):** The editor Play test passed on its first run, because the behaviour already existed when it was written; it pins that behaviour.
- **Changes:** Validation only; no product file changed by this task. Run again after the CT-9 amendment: `cargo fmt --all -- --check`, clippy with `-D warnings`, `cargo test --all-targets` (all suites green), all twelve addon scripts pass `--check-only`, `editor_e2e_test` 9 of 9, `addon_export_test` 2 of 2, `runtime_e2e_test` 20 of 20 on Godot 4.8-beta1 at `master`. No task ID, phase, design path or specification name in any product file (the contributor guide's own rule about `.design/` excepted).

### [T-7D01] Carry to the release lines (after validation)

- **Spec:** l2-held-stepping.md §5.7; l2-release-branches.md
- **Status:** Blocked
- **Assignment:** Agent
- **Verify:** On `godot-v4.7`, `godot-v4.6` and `godot-v4.5`, in that order, the change applied as new commits naming its origin; on each, `cargo test --all-targets`, clippy, fmt, the GDScript syntax checks, and `editor_e2e_test`, `addon_export_test` and `runtime_e2e_test` (ignored) pass with that line's Godot build from its `.github/godot-version`.
- **Handoff:** Pushing the branches needs the maintainer's yes in the conversation. Runs after T-7T01.
- **Notes:** No `has_method()` guard is expected (every call exists in 4.5.0); if a line's build fails on one, add the guard on that line and record it in the line's guard list.
- **Blocked:** T-7T01 has passed. The change is not committed anywhere: the run never commits, and carrying it needs a commit on `master` to copy. It also needs the maintainer's yes to touch `godot-v4.7`, `godot-v4.6` and `godot-v4.5`, and the Godot builds of those lines (4.7.2, 4.6.3, 4.5.2; the minimum-version build is not installed by default). Next: commit the change on `master`, then run this task.
