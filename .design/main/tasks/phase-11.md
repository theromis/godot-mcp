---
phase: 11
name: "Reality Sync after Ventilation"
status: Todo
subsystem: "addons/godot_mcp/bridge_server.gd, .agents/skills/repo-steward, tests, README"
requires: [9]
provides: []
key_files:
  created: []
  modified: []
patterns_established: []
duration_minutes: ~
---

# Stage 11 Tasks — Reality Sync after Ventilation

**Phase:** 11
**Status:** Todo
**Strategic Goal:** The two places where a specification synced on 2026-11-02 asks more than the merged code does are closed: `godot_set_project_setting` accepts and refuses the same values with and without the editor (CA-4, CA-11), and the repo steward labels a merged pull request whose planning update is still owed until that update is committed.

## Atomic Checklist

<!-- A line is `- [ ]` (open) or `- [x]` (done). A Blocked task keeps `- [ ]`: it is blocked by its `Status` in Detailed Tracking, and the `[!]` the workflows write after "Blocked" spells that status — it is not a checkbox state. -->

- [ ] [T-11A01] Editor: `set_project_setting` converts the text forms of CA-4
- [ ] [T-11B01] Steward: the `status: planning-owed` label, its run step and report line
- [ ] [T-11T01] Validation: pipeline, editor suites, settings parity and steward tests
- [ ] [T-11E01] Carry the addon change to the release lines (after validation and the line catch-up)

## Detailed Tracking

<!-- Optional entry field, created by /magic.run on a task's first dead end — absent means none. A nested list under the entry's field list, one line per abandoned approach, at most five (run.md, Dead-End Record):
  - **Attempts:**
    - {what was tried} → {why it failed}
-->

### [T-11A01] Editor: `set_project_setting` converts the text forms of CA-4

- **Spec:** l1-content-authoring.md CA-4, CA-11; l2-godot-editor-addon.md §6 (open gap); l2-tool-handlers.md §5.5 (setting types)
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** A test written first and red in `tests/editor_e2e_test.rs` (ignored, `--test-threads=1`, the build in `.github/godot-version`): through the editor, `application/boot_splash/bg_color` set to `"#102b38"` is stored as a `Color` (read back with `godot_get_project_settings` and in `project.godot` as `Color(...)`); a named color such as `"red"` is accepted for a `Color`; `"Vector2i(1152, 648)"` is accepted for a `Vector2i` setting; a whole number is stored as an `int`; a string that does not convert (`"not a color"` for a `Color`) is refused with the setting named and `project.godot` unchanged; an Object text is refused. The same inputs give the same accept or refuse through the file fallback (the existing `tests/tools_test.rs` cases, extended where one is missing). `README.md` `godot_set_project_setting` row no longer limits the conversions to "without the editor". GDScript `--check-only` of `bridge_server.gd` and the analyzer audit `addon_scripts_have_no_analyzer_warnings` pass.
- **Handoff:** T-11T01; then remove the open-gap note from l2-godot-editor-addon.md §6 and the "does not convert yet" sentence from l2-tool-handlers.md §5.5 with `/magic.spec`.
- **Notes:** `_coerce_setting` in `bridge_server.gd` keeps its exact-type cases and adds: a `Color` from `Color.html_is_valid` / a named color, and for other non-Object types a `str_to_var` parse accepted only when the result has the setting's type (the conversion the server's `SETTING_TYPE_SCRIPT` makes; parse only types whose text holds numbers or strings, so no object is created). Parameters stay read untyped and validated; one reply per path (BR-3). The old value is restored if saving fails, as now.

### [T-11B01] Steward: the `status: planning-owed` label, its run step and report line

- **Spec:** l2-repo-steward-procedures.md §5.1 (planning after merge), ST-7 row; l2-repo-steward-skill.md §5.4, §5.5 step 7, §5.7
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** Tests written first and red in `tests/steward_test.rs`: `scripts/labels.sh --dry-run` against the fixture repository lists `status: planning-owed` with a colour and a description; `SKILL.md` names the settle step after the pull request step and the `Planning owed` report line; `reference/pull-requests.md` item 8 says to add the label before the merge and remove it after the planning commit, and that a run without the ecosystem keeps it. `cargo test --test steward_test` passes; `bash -n` on every changed script.
- **Handoff:** T-11T01.
- **Notes:** Generic skill files only: nothing names this repository. Messages never mention planning artifacts (SKILL.md rule). The label is set by `act.sh` like the other labels; check that `act.sh` accepts it, and extend its allow-list if it has one.

### [T-11T01] Validation: pipeline, editor suites, settings parity and steward tests

- **Spec:** l1-quality-assurance.md; l1-content-authoring.md CA-4, CA-11; l2-repo-steward-procedures.md §5.1
- **Status:** Todo
- **Assignment:** Agent
- **Verify:** On `master` with the build in `.github/godot-version`, one run after another: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`, `cargo test --all-targets`; `godot --headless --check-only` for every addon script in the AGENTS.md list; `editor_e2e_test` and `addon_export_test` with `--ignored --test-threads=1`; `runtime_e2e_test addon_scripts_have_no_analyzer_warnings`. The Changes field lists the CA-4 inputs and the test that proves each with and without the editor.
- **Handoff:** T-11E01.
- **Notes:** The local editor runs in Russian: `open_scene_returns_what_the_load_errors_dialog_shows` fails on the dialog title and is environmental; name it in the report. Never start two end-to-end runs at once.

### [T-11E01] Carry the addon change to the release lines (after validation and the line catch-up)

- **Spec:** l2-release-branches.md; l1-release-lines.md
- **Status:** Blocked
- **Assignment:** Agent
- **Verify:** On `godot-v4.7`, `godot-v4.6` and `godot-v4.5`, in that order, the product commit of T-11A01 applied with `cherry-pick -x`; on each, clippy, fmt, `cargo test --all-targets`, the GDScript checks, `editor_e2e_test` and `addon_export_test` (ignored) pass with that line's own Godot build.
- **Handoff:** Pushing the branches needs the maintainer's yes in the conversation.
- **Notes:** Blocked by the same cause as T-10E01: the lines must first receive the master work since their last carry. Lift the block by setting this task to `Todo` once that catch-up is done. The steward change (T-11B01) lives in `.agents/` and is not carried.
