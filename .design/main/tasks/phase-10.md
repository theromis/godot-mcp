---
phase: 10
name: "Project Adapter"
status: Todo
subsystem: "addons/godot_mcp/runtime, src/tools/game, src/godot, tests, README"
requires: [6, 7]
provides: []
key_files:
  created: []
  modified: []
patterns_established: []
duration_minutes: ~
---

# Stage 10 Tasks — Project Adapter

**Phase:** 10
**Status:** Todo
**Strategic Goal:** A project names one development-only adapter script in `godot_mcp/adapter`; the in-game component loads it with godot-dev-bridge's registration contract, and an agent reads the project's states with `godot_game_state` and runs its commands with `godot_game_command`, in a managed run and in editor Play, held or not, with failures in the project's code reported instead of hidden. An adapter written for godot-dev-bridge works unchanged.

## Atomic Checklist

<!-- A line is `- [ ]` (open) or `- [x]` (done). A Blocked task keeps `- [ ]`: it is blocked by its `Status` in Detailed Tracking, and the `[!]` the workflows write after "Blocked" spells that status — it is not a checkbox state. -->

- [ ] [T-10A01] Feasibility: the logger sees a handler's script error (engine premise)
- [ ] [T-10B01] Runtime: the registry and the output marks (PA-2, PA-5)
- [ ] [T-10B02] Runtime: loading the adapter, its status, the hello and the runtime copy (PA-1, PA-6, PA-10)
- [ ] [T-10B03] Runtime: `project.state` and `project.command` (PA-3, PA-6 to PA-9)
- [ ] [T-10C01] Host: `godot_game_state` and `godot_game_command`, run status and hints (PA-4, TL-1, TL-7)
- [ ] [T-10C02] Editor Play: the adapter answers in a played game (PA-10)
- [ ] [T-10D01] User-facing text: README, agent guide, check lists, changelog
- [ ] [T-10T01] Validation: pipeline, end-to-end suites and invariant coverage
- [ ] [T-10E01] Carry to the release lines (after validation)

## Detailed Tracking

<!-- Optional entry field, created by /magic.run on a task's first dead end — absent means none. A nested list under the entry's field list, one line per abandoned approach, at most five (run.md, Dead-End Record):
  - **Attempts:**
    - {what was tried} → {why it failed}
-->

### [T-10A01] Feasibility: the logger sees a handler's script error (engine premise)

- **Spec:** l2-project-adapter.md §2 (engine premise), §6 step 1; l1-project-adapter.md PA-6
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** A new ignored test in `tests/runtime_e2e_test.rs`, run with `GODOT_PATH` set to the build named in `.github/godot-version` (`4.8-beta1`) and `--ignored --test-threads=1`, pins four facts in a managed run: (1) a GDScript handler that reads a property of a null instance, called through `Callable.call()` from the runtime, produces a logger entry before `call()` returns; (2) that entry carries the engine's script error type; (3) `push_error` in another handler produces an entry of a different type; (4) what `Callable.call()` returns for a handler whose first statement is an `await`. The test passes and its assertions name the four facts.
- **Handoff:** If (1) or (2) does not hold, stop the phase before T-10B03 and run `/magic.spec amend l1-project-adapter.md "PA-6 attaches logged errors without failing the call"`; T-10B01 and T-10B02 do not depend on the outcome. Record the four measured facts in this task's Changes.
- **Notes:** The logger is `OutputLogger` in `addons/godot_mcp/runtime/ops/rt_output.gd` (installed in the runtime's `_enter_tree`). The test may reach the facts through a temporary runtime method or through T-10B01's marks if that task lands first; either way the test stays in the suite.

### [T-10B01] Runtime: the registry and the output marks (PA-2, PA-5)

- **Spec:** l2-project-adapter.md §4 (PA-2, PA-5, PA-6 rows), §5.1
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** Tests written first and red: a fixture adapter (one file kept under `tests/common/`, shared by the runtime and editor end-to-end tests) registers two valid states, two valid commands, a provider that takes an argument, a handler with no argument, an empty name and a name registered twice; a runtime end-to-end test reads the registry through a runtime method and finds the valid ones, the second registration of the repeated name, and the three refused ones listed under `refused` with `{name, kind, reason}`; a command returning `registry.refuse("x")` is told apart from one returning `{"error": "x"}`. `godot --headless --check-only --path . --script addons/godot_mcp/runtime/ops/rt_registry.gd` and the same for `rt_output.gd` report nothing.
- **Handoff:** T-10B02 hands the registry to the adapter; T-10B03 uses the marks.
- **Notes:** `rt_registry.gd`: `add_state(name, provider) -> bool`, `add_command(name, handler, help = "") -> bool`, `refuse(message) -> Dictionary` returning `{"__bad_args__": str(message)}`, names untyped where godot-dev-bridge has them untyped. No `class_name`. `rt_output.gd` gains `mark()` (the next sequence number) and `since(mark)` (the entries logged after it, with their type). Strict typing; parameters read untyped and validated.

### [T-10B02] Runtime: loading the adapter, its status, the hello and the runtime copy (PA-1, PA-6, PA-10)

- **Spec:** l2-project-adapter.md §2 (the setting), §4 (PA-1, PA-6, PA-10 rows)
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** Tests written first and red, in `tests/runtime_e2e_test.rs` (ignored, 4.8-beta1 build): with no `godot_mcp/adapter` the hello's `adapter` is `{configured: false}`; with the fixture adapter by `res://` path and by `uid://` the status is `loaded: true` with its `states` and `commands` (name → help); a setting of `user://x.gd`, a missing file, a script without `register` and a `register` that raises a script error each give `loaded: false` with an `error` that names the cause (the last one keeps the registrations made before the error); an adapter that extends `Node` is freed when the game exits (no leaked-instance line in the output). In `src/godot/runs.rs` the existing unit tests over `RUNTIME_FILES` cover the two new scripts in the private runtime copy. Clippy and fmt clean.
- **Handoff:** T-10B03 answers from the loaded registry; T-10C01 shows the status in `godot_get_run_status`.
- **Notes:** Load in the runtime's `_ready`, before `_send_hello` is scheduled. Resolve `uid://` with `ResourceUID`; accept only `res://` after resolution. The running-game tools never set the setting (PA-1).

### [T-10B03] Runtime: `project.state` and `project.command` (PA-3, PA-6 to PA-9)

- **Spec:** l2-project-adapter.md §4 (PA-3, PA-6 to PA-9 rows), §5.2
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** Tests written first and red, in `tests/runtime_e2e_test.rs` (ignored, 4.8-beta1 build), each named after the rule it pins: an empty `names` reads every provider; an unknown state is an `{error}` entry beside the others' values; a provider that raises a script error fails its own entry only; a command answers `value` and the `errors` logged meanwhile; an unknown command answers `NOT_FOUND` with `data.commands`; a refusal answers `BAD_PARAMS` with its message; a handler's script error answers `GODOT_ERROR`; a state whose JSON text exceeds 262 144 bytes answers `BAD_PARAMS` with `data.sizes`; in a game held with `godot_game_physics` `hold`, `project.state` and `project.command` answer, and a command's `call_deferred` change is visible after one held step and not before; with no adapter, `project.command` answers `UNSUPPORTED` with the status. Arguments arrive as parsed JSON (a `3` is a float).
- **Handoff:** T-10C01 maps these codes to tool answers and hints.
- **Notes:** `rt_project.gd` exports the two methods through `methods()` like the other modules; they stay out of `Hold.refusal`. Values through `rt.encode`. If T-10A01 found the logger blind to script errors, this task waits for the PA-6 amendment.

### [T-10C01] Host: `godot_game_state` and `godot_game_command`, run status and hints (PA-4, TL-1, TL-7)

- **Spec:** l2-project-adapter.md §4 (PA-4 row), §5.2 (recovery column), §5.3; l1-tool-catalog.md §5.1
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** Tests written first and red in `tests/tools_test.rs`: `tools/list` has both tools with their schemas (`names` at most 64 strings of 1 to 256 characters; `name` required, 1 to 256; `args` an object; `run_id`) and annotations `[true, false, true]` and `[false, true, false]`; a schema violation fails before any game is contacted and names the parameter; with no game running both say so and name `godot_run_project`; the recovery for each runtime code of §5.2 is the one the table names (unit test of the mapping). In `tests/runtime_e2e_test.rs` (ignored): `godot_get_run_status` shows the fixture adapter's status for a managed run; `godot_game_command` with `run_id` reaches that run. Clippy and fmt clean; no `unwrap()` or `expect()` in the new code; no lock held across the call into the game.
- **Handoff:** T-10D01 documents the final names and answers.
- **Notes:** The `tools_test.rs` part can start beside track B; the end-to-end part needs T-10B03. Definitions in `src/tools/game/catalog.rs`, handlers in a new `src/tools/game/project.rs`, annotations in `src/tools.rs`. Run selection as `godot_game_get`. The descriptions say that a command does what the project wrote, files included, and point to `adapter` in `godot_get_run_status`.

### [T-10C02] Editor Play: the adapter answers in a played game (PA-10)

- **Spec:** l1-project-adapter.md PA-10; l2-project-adapter.md §4 (PA-10 row)
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** A test written first and red in `tests/editor_e2e_test.rs` (ignored, 4.8-beta1 build, `--test-threads=1`): with the fixture adapter set in the test project, Play through `godot_run_project`, then `godot_game_state` returns the fixture's values and `godot_game_command` runs one of its commands through the relay; `godot_stop_project` ends Play.
- **Handoff:** None.
- **Notes:** After T-10B02, T-10B03 and T-10C01. The relay (`game_relay.gd`) is not changed.

### [T-10D01] User-facing text: README, agent guide, check lists, changelog

- **Spec:** l2-project-adapter.md §5.4
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** `README.md` lists `godot_game_state` and `godot_game_command` and has an adapter section with the setting, the three registry calls, the outcome table, a short example and the note for godot-dev-bridge adapters (set `godot_mcp/adapter` to the same file); the running-game protocol in `AGENTS.md` has the adapter line; `rg -n "rt_registry.gd|rt_project.gd" AGENTS.md CONTRIBUTING.md` shows both scripts in both check lists; `CHANGELOG.md` has an Unreleased entry; the tool descriptions in `src/tools/game/catalog.rs` agree with the README.
- **Handoff:** T-10T01.
- **Notes:** English only. The two check lists change together (AGENTS.md says so). No reference to the planning files in product text.

### [T-10T01] Validation: pipeline, end-to-end suites and invariant coverage

- **Spec:** l1-project-adapter.md PA-1 to PA-10; l2-project-adapter.md §4; l1-quality-assurance.md
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** On `master` with the 4.8-beta1 build: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`, `cargo test --all-targets`; `godot --headless --check-only` for every addon script in the AGENTS.md list, the two new ones included; `editor_e2e_test`, `addon_export_test`, `runtime_e2e_test` (with `addon_scripts_have_no_analyzer_warnings`) and `scenario_e2e_test` with `--ignored --test-threads=1`, run one after another. This task's Changes holds a table from each of PA-1 to PA-10 to the test names that prove it, with no row empty.
- **Handoff:** T-10E01.
- **Notes:** The local editor runs in Russian: `open_scene_returns_what_the_load_errors_dialog_shows` fails on the "Load Errors" text and is environmental; name it in the report. Never start two end-to-end runs at once.

### [T-10E01] Carry to the release lines (after validation)

- **Spec:** l2-release-branches.md; l1-release-lines.md; l2-project-adapter.md §5.5
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** On `godot-v4.7`, `godot-v4.6` and `godot-v4.5`, in that order, the product commits applied with `cherry-pick -x`; on each, clippy, fmt, `cargo test --all-targets`, the GDScript checks, `editor_e2e_test`, `addon_export_test` and `runtime_e2e_test` (ignored) pass with that line's own Godot build (4.7.2, 4.6.3, 4.5.2 are installed locally), one line after another.
- **Handoff:** Pushing the branches needs the maintainer's yes in the conversation.
- **Notes:** Product files only, not the planning files. Nothing here should need a call missing from 4.5.0; if a line's engine refuses one, guard it with `has_method()` + `call()`.
