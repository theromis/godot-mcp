---
phase: 8
name: "Script Reading and Editing"
status: Done
subsystem: "addons/godot_mcp, src/tools, tests, README"
requires: [6]
provides:
  - godot_read_script and godot_edit_script: read an existing script as the editor would save it, replace one fragment of it
  - Editor path through the script editor (one undo step, saved, refused over unsaved typing) and a file fallback through an exclusive temporary file
  - The version label (64-bit FNV-1a) computed identically in the addon and the host
  - The change on the 4.7, 4.6 and 4.5 lines
key_files:
  created:
    - addons/godot_mcp/editor/script_ops.gd
    - src/tools/scripts.rs
  modified:
    - addons/godot_mcp/bridge_server.gd
    - src/tools.rs
    - tests/editor_e2e_test.rs
    - tests/tools_test.rs
    - tests/server_test.rs
    - README.md
    - AGENTS.md
    - CONTRIBUTING.md
    - CHANGELOG.md
    - docs/feature-parity.md
    - .github/workflows/ci.yml
patterns_established:
  - A feasibility task on a real engine first; the one assumption that failed (no call names an editor's resource) became a pairing rule with a safe refusal
  - Run every release line with its own pinned engine before calling a change carried: a metadata key present in 4.8 and absent in 4.5 to 4.7 broke the same two tests on all three lines
  - Temporary files next to a target are created exclusively under an unpredictable name, never written through a path that follows links
duration_minutes: ~
---

# Stage 8 Tasks — Script Reading and Editing

**Phase:** 8
**Status:** Done
**Strategic Goal:** Let an agent read an existing GDScript file and change it by replacing one fragment, through the editor when it is connected (undoable, never over a person's unsaved typing) and through the file when it is not, so that agents stop editing scripts behind the editor's back.

## Atomic Checklist

<!-- A line is `- [ ]` (open) or `- [x]` (done). A Blocked task keeps `- [ ]`: it is blocked by its `Status` in Detailed Tracking, and the `[!]` the workflows write after "Blocked" spells that status — it is not a checkbox state. -->

- [x] [T-8A01] Feasibility: script editor facts on a real engine
- [x] [T-8A02] Specification notes for the two commands and the two tools
- [x] [T-8A03] Editor side: `read_script` and the shared script helpers
- [x] [T-8A04] Editor side: `edit_script`
- [x] [T-8B01] Host side: `godot_read_script` and its file fallback
- [x] [T-8B02] Host side: `godot_edit_script` and its file fallback
- [x] [T-8C01] User-facing text: README, agent guide, changelog, parity table
- [x] [T-8T01] Validation: pipeline, end-to-end suites and invariant coverage
- [x] [T-8D01] Carry to the release lines (after validation)

## Detailed Tracking

<!-- Optional entry field, created by /magic.run on a task's first dead end — absent means none. A nested list under the entry's field list, one line per abandoned approach, at most five (run.md, Dead-End Record):
  - **Attempts:**
    - {what was tried} → {why it failed}
-->

### [T-8A01] Feasibility: script editor facts on a real engine

- **Spec:** l1-content-authoring.md §2 (forecast premise), CA-7, CA-9, CA-10
- **Status:** Done
- **Assignment:** Agent
- **Verify:** A new ignored test in `tests/editor_e2e_test.rs`, written first, run with `GODOT_PATH` set to the build named in `.github/godot-version` (4.8-beta1), prints one measured answer per fact below and asserts the ones the design needs: (1) a script opened with `EditorInterface.edit_script` is found among the open script editors, in the editor the test starts; (2) after text is typed into its code editor without saving, the unsaved state is detectable (compare the code editor's version with its saved version, or the text with the file) and the unsaved text is readable; (3) a fragment replacement made on the code editor as one complex operation is undone by one undo step; (4) saving the open script writes the new text to disk and a node running the script gets the new code (`reload`); (5) a script that was not open can be opened, edited and saved the same way; (6) none of this needs a window, or the answer says what it needs. `godot --headless --check-only` still passes for every addon script.
- **Handoff:** T-8A02 starts after this is Done; its text follows the measured answers.
- **Notes:** Every fact is unverified; the pattern from the held-stepping phase applies: if a fact is false and the specification has no in-spec fix, set this task `Blocked` with the observation. The reason is a specification gap in `l1-content-authoring.md` (CA-9/CA-10 and the forecast premise, whose runner-up is leaving the edit unsaved). Use only calls that exist in Godot 4.5.0; anything newer goes through `has_method()` + `call()` (AGENTS.md). Do not add production code in this task beyond what the probe needs; a probe command can live in the test.
- **Changes:** `tests/editor_e2e_test.rs`: the editor start helper takes test-only commands, helper functions and extra fixture files (`start_editor_with`; the old helpers call it); a new ignored test `script_editor_facts_for_reading_and_editing` with `__probe_*` commands that exist only in the test copy of the addon. Red first (the first run failed on a call the engine lacks), then green on 4.8-beta1 in 13 s; fmt and clippy clean; no addon file changed.
- **Findings:** Measured on Godot 4.8-beta1 in the headless editor. (1) The script editor exists without a window: a script opened with `EditorInterface.edit_script` appears in `get_open_script_editors()`. (2) The unsaved state is the code editor version: `get_version()` differs from `get_saved_version()` after typing (2 vs 3), the unsaved text is `get_base_editor().text`, and the loaded script resource keeps the old text (`source_code` does not follow the buffer), so a read of unsaved text must use the code editor. Typing does not touch the file. (3) A replacement made as one complex operation (`begin_complex_operation`, `remove_text`, `insert_text`, `end_complex_operation`) is reversed by one `undo()`, which also returns the version to the saved one. (4) `script.source_code = text`, `ResourceSaver.save`, `tag_saved_version()` and `script.reload(true)` write the file, clear the unsaved state (versions equal) and a `@tool` node running the script answers with the new code. (5) A script that was not open is opened, edited and saved the same way. (6) None of it needs a window. **Not as the specification assumed:** `ScriptEditorBase` has no `get_edited_resource()` (the first run failed on it), and `get_current_editor()` returned null in the headless editor, so an editor cannot be matched to a path by those. What works is the index alignment of `get_open_scripts()` and `get_open_script_editors()` (same order, same length with scripts only) plus the editor's tab name (`_tab_name` metadata, the file name). **Not measured:** a text file or a help page in the tab list (headless `edit_resource` and `goto_help` opened no tab), so the lists could differ in length in a real window. Rule for T-8A02: pair editors with scripts only when both lists have the same length and the tab name starts with the file name, otherwise treat the editor state as unknown and refuse the edit with the fix (close the extra tabs or save and retry). Opening a script to edit it adds a tab in the person's editor; that is visible and acceptable.

### [T-8A02] Specification notes for the two commands and the two tools

- **Spec:** l2-godot-editor-addon.md; l2-tool-handlers.md; l1-tool-catalog.md (catalog table); l1-content-authoring.md CA-7 to CA-10
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `l2-godot-editor-addon.md` and `l2-tool-handlers.md` each amended with a version bump and a Document History row through `/magic.spec` rules: the command and tool names, their parameters and answers, the unsaved-state test, how the editor applies and saves an edit, the version label (one scheme computed identically in the editor and the host, with a shared test vector), line-ending handling, the file fallback (confinement, `.gd` only, never creating a file, whole-file replacement), and the rows of the catalog table; every CA-7 to CA-10 sentence maps to a row. The Post-Update Review (`@role:spec-critic`) passes; `check-prerequisites` is clean; `export-wiki` runs.
- **Handoff:** T-8A03, T-8A04, T-8B01 and T-8B02 start after this is Done.
- **Notes:** Suggested decisions to confirm against T-8A01's answers: the label is the 64-bit FNV-1a hash of the UTF-8 text as 16 hex digits (no new dependency in the host or the addon); caller text and file text are compared with `\r\n` read as `\n`, and a file is written back in the line ending it had; an answer to a read carries `path`, `text`, `source` (`editor` or `file`), `dirty` (the editor's text differs from the file) and `version`; an edit answer carries `path`, `replaced: 1`, `version` and `source`. If the helpers go in a new addon file (for example `editor/script_ops.gd`, loaded by path, no `class_name`) so `bridge_server.gd` does not grow past what one task can review, the spec names it and T-8A03 adds it to the syntax-check lists.
- **Changes:** `l2-godot-editor-addon.md` 1.4.0 (§5.3 script commands: measured facts, pairing rule, `read_script` and `edit_script` steps and answers, new file `editor/script_ops.gd`), `l2-tool-handlers.md` 1.3.0 (§5.4 script tools: parameters, answers, FNV-1a version label with three test vectors, matching, line endings, atomic file fallback), `l1-tool-catalog.md` 1.4.0 (two catalog rows). Each went Stable → RFC → Stable after the review; `check-bloat` and header parity clean. Decisions taken from T-8A01: parameter name `script_path`; both tools refuse files over 1 MiB and invalid UTF-8; an edit is refused when the editor state cannot be matched; the editor path keeps the editor's line endings, the file fallback keeps the file's.
- **Notes (run):** The `[SCRIPTS]` canonical reference for `editor/script_ops.gd` is not added yet because the file does not exist; T-8A03 adds the row to `l2-godot-editor-addon.md` when it creates the file. Review lenses applied: every CA-7 to CA-10 sentence maps to a numbered step or rule; no rule is justified only by a scenario that could happen (each refusal follows from a measured fact or from CA-9).

### [T-8A03] Editor side: `read_script` and the shared script helpers

- **Spec:** l2-godot-editor-addon.md (as amended by T-8A02); l1-content-authoring.md CA-6, CA-7
- **Status:** Done
- **Assignment:** Agent
- **Verify:** New tests in `tests/editor_e2e_test.rs`, written first and red before the handler exists, then green on 4.8-beta1: reading a script that is not open returns the file text with `source: "file"`, `dirty: false` and a `version`; reading a script open with unsaved typing returns that text with `source: "editor"`, `dirty: true` and a different `version`; a path outside the project, a path not ending in `.gd`, a file that does not exist and a wrongly typed parameter each answer with an error that names the fix and does not hang (a reply is always sent). `godot --headless --check-only` passes for every addon script (new script included in the lists of AGENTS.md, CONTRIBUTING.md and `ci.yml`, which must stay equal).
- **Handoff:** T-8A04 builds on the helpers; it starts after this is Done.
- **Notes:** Read parameters untyped and validate them (`PARAM_TYPES`). Paths go through `to_res_path`; the version label is the scheme from T-8A02. Runs in parallel with T-8B01, which touches only `src/` and the Rust tests. When the new file `editor/script_ops.gd` exists, add its row (`[SCRIPTS]`) to Canonical References of `l2-godot-editor-addon.md` (patch bump).
- **Changes:** New `addons/godot_mcp/editor/script_ops.gd` (+ `.uid`): the version label, `read`, the editor pairing rule, file reading with the size, extension and UTF-8 checks. `bridge_server.gd`: `read_script` in `PARAM_TYPES`, the dispatch table and a thin handler with the path check; the script operations are loaded by path on first use. New script in the syntax-check lists of AGENTS.md, CONTRIBUTING.md and `ci.yml` (equal). Tests in `tests/editor_e2e_test.rs`, red first ("Unknown command type"), then green on 4.8-beta1: `read_script_answers_from_the_file_or_from_unsaved_editor_text` (file read, CRLF, the three label vectors, open clean script, unsaved typing, six refusals, still answering) and `the_script_label_matches_its_published_vectors`. `l2-godot-editor-addon.md` 1.4.1 gained the `[SCRIPTS]` reference. Every addon script passes `--check-only`.

### [T-8A04] Editor side: `edit_script`

- **Spec:** l2-godot-editor-addon.md (as amended by T-8A02); l1-content-authoring.md CA-8, CA-9, CA-10, CA-3
- **Status:** Done
- **Assignment:** Agent
- **Verify:** New tests in `tests/editor_e2e_test.rs`, written first and red, then green on 4.8-beta1: an edit whose old text occurs once changes the script, saves it, and is undone by one undo step; an old text that occurs zero times and one that occurs twice are refused with the count and leave the file and the editor untouched; a supplied version label that does not match is refused; a script open with unsaved typing is refused with its path and the fix, and its unsaved text is still there afterwards; old and new text containing quotes, `%`, `{}`, backslashes and a line that looks like GDScript are applied as plain text (nothing executed or interpolated); a `.gd.uid` file and the script's class registration are unchanged; a node using the script runs the new code. `godot --headless --check-only` passes for every addon script.
- **Handoff:** T-8C01 and T-8T01 wait for this and T-8B02.
- **Notes:** Runs after T-8A03 because both edit the addon's handlers. `godot_edit_script` never creates a file (CA-8): a missing file is an error that points to `godot_create_script`. Calls missing from Godot 4.5.0 go through `has_method()` + `call()`.
- **Changes:** `editor/script_ops.gd`: `edit` (checks, editor pairing and the refusals for unknown editor state, unsaved changes and an editor that differs from the file, label and count checks, one complex operation on the code editor, save with `ResourceSaver`, `tag_saved_version`, file system update and `reload`, undo if the save fails). `bridge_server.gd`: `edit_script` in `PARAM_TYPES`, the dispatch table and a thin handler. Test `edit_script_replaces_one_fragment_and_never_overwrites_typing` in `tests/editor_e2e_test.rs`, red first ("Unknown command type"), then green on 4.8-beta1: edit and saved state, one undo step, uid file unchanged, 0 and 2 matches, stale and current label, quotes/`%`/braces/backslashes/code-looking text inserted verbatim, unsaved typing refused with the typed text still in the editor, a running `@tool` node gets the new code and the closed script was opened by the edit, seven refusals. Whole editor suite 12 of 12 on 4.8-beta1; fmt, clippy, every addon script check clean.
- **Notes (run):** The refusal for an editor that cannot be matched to the path (lists of different length, tab name not the file) is in the code but cannot be reached in the headless editor, so it has no end-to-end test; it is the safe branch of the pairing rule (see T-8A01 findings).

### [T-8B01] Host side: `godot_read_script` and its file fallback

- **Spec:** l2-tool-handlers.md (as amended by T-8A02); l1-content-authoring.md CA-6, CA-7; l1-tool-catalog.md TL-4
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo test --all-targets` with new tests written first, red before the code. In `tests/tools_test.rs` with `MockBridge`: the tool forwards `read_script` and returns the editor's answer unchanged; with no editor it reads the file, answers `source: "file"`, `dirty: false` and the same `version` the editor scheme gives for that text (a shared test vector with the addon); a path outside the project, a non-`.gd` path and a missing file are refused with a fix; invalid UTF-8 is refused with a message, not replaced. In `tests/server_test.rs` the catalog lists `godot_read_script` with its schema. `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` are clean; no `unwrap()` or `expect()` in the new code.
- **Handoff:** T-8B02 builds on the file helpers; it starts after this is Done.
- **Notes:** Fallback only when the bridge is unavailable (`McpError::BridgeUnavailable`), as `create_scene` does; any other bridge error is returned as it is. Paths through `resolve_in_project`. Bound the size of a read and say so when it is cut (a script of several megabytes is not an authoring case). Runs in parallel with T-8A03; neither touches the other's files.
- **Changes:** New module `src/tools/scripts.rs` (the script tools do not go into `authoring.rs`, which would pass 700 lines): catalog entry, `godot_read_script` (editor first; only `BridgeUnavailable` reads the file; any other editor error is returned as it is), `label` (FNV-1a), the loader shared with the edit tool (path check, `.gd` only, size, UTF-8, byte order mark and line endings). `src/tools.rs`: module, dispatch and catalog wiring. Tests, red first: three in `tests/tools_test.rs` (file read with the three label vectors and CRLF; five refusals with a fix; editor answer forwarded and an editor error not hidden by a file read) and the catalog assertion in `tests/server_test.rs`. Green; clippy clean.
- **Notes (run):** `l2-tool-handlers.md` still says the tools live in `tools/authoring.rs`; it is corrected with T-8B02 in one patch.

### [T-8B02] Host side: `godot_edit_script` and its file fallback

- **Spec:** l2-tool-handlers.md (as amended by T-8A02); l1-content-authoring.md CA-8, CA-9, CA-10, CA-3
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo test --all-targets` with new tests written first, red before the code. In `tests/tools_test.rs`: with `MockBridge` the tool forwards `edit_script` with the caller's strings as data and returns the editor's refusals (unsaved changes, count, label) unchanged; with no editor the file is changed when the old text occurs once and the answer carries the new `version`; zero matches, two matches, a stale label and a missing file are refused and the file bytes are identical afterwards; a CRLF file keeps CRLF and is matched with `\n` in the old text; the replacement is written as a whole (a temporary file in the same directory, then a rename), so an interrupted write cannot leave half a script, and a failed write leaves the original; an empty old text is refused. Clippy, fmt and the whole default suite are clean.
- **Handoff:** T-8C01 and T-8T01 wait for this and T-8A04.
- **Notes:** The host cannot know that an editor holds unsaved typing when no bridge is connected; CA-9 is then about the editor only, and the fallback says in its answer that no editor was consulted. Never hold a lock across the bridge round-trip.
- **Changes:** `src/tools/scripts.rs`: `godot_edit_script` in the catalog and dispatch; `edit` (editor first, file only on `BridgeUnavailable`), `edit_file` (empty and identical text refused, label check, exactly-once count with overlapping matches as the addon counts them, text given with `
` matches CRLF files), `replace_file` (byte order mark and CRLF kept, temporary file in the same directory then rename, temporary file removed on failure). Six tests in `tests/tools_test.rs`, red first (tool not recognized), then green: replacement and label, eight refusals with the file bytes and directory listing unchanged, current label accepted with text-as-data, CRLF and byte order mark kept across two lines, a read-only script (Windows only) unchanged with no temporary file left, editor answer forwarded with strings as data and an editor refusal not turned into a file write; catalog assertion in `tests/server_test.rs`. Whole default suite green (130 unit, all integration), clippy and fmt clean. `l2-tool-handlers.md` 1.3.1 names `tools/scripts.rs`.
- **Notes (run):** The read-only test needs Windows semantics (renaming over a read-only file fails there; on Unix the directory permission decides), so it is `cfg(windows)`; the Unix write-failure path is the same code and is covered by reading.

### [T-8C01] User-facing text: README, agent guide, changelog, parity table

- **Spec:** l1-tool-catalog.md; l1-content-authoring.md; AGENTS.md "Tool Invocation Protocols"
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `README.md` lists both tools with their editor and file behaviour, the refusal while the editor holds unsaved typing, and the one-fragment rule; the tool and workflow descriptions in the catalog (`src/tools/authoring.rs`) say the same and name the fix for each refusal; AGENTS.md "Validation After Modification" names `godot_edit_script` beside creating a script and asks for `godot_validate_script` after it (CONTRIBUTING.md carries no copy to change, or the same line); `CHANGELOG.md` has an `[Unreleased]` entry in the style of the earlier ones; `docs/feature-parity.md` marks the script read and edit rows with what was checked. The markdown lint configuration passes for the changed files; `cargo test --all-targets` is green.
- **Handoff:** T-8T01.
- **Notes:** Product files never mention planning files, specification names or task IDs (CONTRIBUTING.md). Wording follows the existing README table.
- **Changes:** `README.md` (two rows in the tool table, one line in Security), `AGENTS.md` (Validation After Modification: read, replace one fragment with the version, never rewrite behind the editor, what to do on an unsaved-changes refusal; CONTRIBUTING.md carries no copy of that list), `CHANGELOG.md` (`[Unreleased]` entry), `docs/feature-parity.md` (a script read/edit row and `src/tools/scripts.rs` in the source map). The catalog descriptions in `src/tools/scripts.rs` name the refusals and the fixes. Default suite still green.
- **Notes (run):** `markdownlint-cli2` is not installed on this machine, so the markdown lint was not run; CI runs it. The other three servers' cells in the parity row say "not compared": their tool names were not checked, and the reference folder is empty.

### [T-8T01] Validation: pipeline, end-to-end suites and invariant coverage

- **Spec:** l1-content-authoring.md CA-3, CA-6 to CA-10; l2-godot-editor-addon.md; l2-tool-handlers.md
- **Status:** Done
- **Assignment:** Agent
- **Verify:** On `master` with 4.8-beta1: the full AGENTS.md pipeline (clippy, fmt, `cargo test --all-targets`, every GDScript `--check-only`), `editor_e2e_test` and `addon_export_test` with `--ignored`; the new addon file, if any, is absent from an exported game (`addon_export_test`); a table in this task's Notes maps each of CA-3, CA-6, CA-7, CA-8, CA-9 and CA-10 to the test that proves it, in the editor and in the file fallback, with no row empty.
- **Handoff:** `/magic.run` phase completion, then T-8D01.
- **Notes:** An invariant without a test is a failure of this task, not a note.
- **Changes:** No product change. Evidence on master with Godot 4.8-beta1 (the build in `.github/godot-version`): clippy and fmt clean; `cargo test --all-targets` green (130 unit tests and every integration suite); all 13 addon scripts of the AGENTS.md list pass `--check-only` and the AGENTS.md and CONTRIBUTING.md lists are equal; `editor_e2e_test` 12 of 12 and `addon_export_test` 2 of 2 with `--ignored` (the export test packs the addon including the new file and finds none of it in the game).
- **Notes (run):** Invariant coverage (E = `tests/editor_e2e_test.rs`, H = `tests/tools_test.rs`):

| Invariant | In the editor | In the file fallback |
| --- | --- | --- |
| CA-3 text is data | E `edit_script_replaces_one_fragment_and_never_overwrites_typing` (quotes, `%`, braces, backslashes, code-looking lines inserted as written) | H `edit_script_accepts_the_current_label_and_treats_text_as_data`; H `edit_script_asks_the_editor_first_and_does_not_hide_its_refusals` (strings travel unchanged) |
| CA-6 paths and extension | E `read_script_answers_from_the_file_or_from_unsaved_editor_text` (outside the project, `.txt`, missing, over 1 MiB, wrong type) and the refusals of the edit test | H `read_script_refuses_what_it_should_with_a_fix`; H `edit_script_refuses_what_it_should_and_leaves_the_file_alone` |
| CA-7 read answers | E the same read test (file, CRLF, three label vectors, open clean script, unsaved text with `dirty`) | H `read_script_without_an_editor_reads_the_file_and_labels_it`; H `read_script_asks_the_editor_first_and_does_not_hide_its_errors` |
| CA-8 exactly once, label | E the edit test (0 and 2 matches, stale and current label, empty and identical text) | H `edit_script_refuses_what_it_should_and_leaves_the_file_alone`; H `edit_script_without_an_editor_replaces_one_fragment` |
| CA-9 never over unsaved typing | E the edit test (refused with the typed text still in the editor, file untouched) | H `edit_script_asks_the_editor_first_and_does_not_hide_its_refusals` (an editor refusal is not turned into a file write) |
| CA-10 one undo step, saved, whole-file replace | E the edit test (one undo step, saved state, uid file unchanged, running node has the new code) | H `edit_script_keeps_the_line_endings_and_the_byte_order_mark_of_the_file`; H `edit_script_leaves_the_script_unchanged_when_it_cannot_be_replaced` (Windows only); no temporary file is left after an edit or a refusal |

Not covered by a test, stated here instead of hidden: (1) the editor-state "unknown" refusal (the pairing rule's safe branch), because a help or text tab cannot be opened in the headless editor; (2) the failed-write case on Unix, which is the same code as the Windows case; (3) the markdown lint, not installed here.

### [T-8D01] Carry to the release lines (after validation)

- **Spec:** l2-release-branches.md; l1-release-lines.md
- **Status:** Done
- **Assignment:** Agent
- **Verify:** On `godot-v4.7`, `godot-v4.6` and `godot-v4.5`, in that order, the change applied as new commits naming their origin; on each, `cargo test --all-targets`, clippy, fmt, the GDScript syntax checks, `editor_e2e_test` and `addon_export_test` (ignored) pass with that line's Godot build from its `.github/godot-version`.
- **Handoff:** Pushing the branches needs the maintainer's yes in the conversation. Runs after T-8T01.
- **Notes:** Product files only (not the planning files). If a line's build lacks a call the code uses, add the `has_method()` + `call()` guard on that line and record it in the line's guard list.
- **Changes:** On `master` four local commits: `20f1ead` (the tools), `f5cc52c` (changelog entry, not carried), `109add6` and `66fa106` (the two fixes below). Carried with `git cherry-pick -x` of the three product commits to `godot-v4.7` (`f79b579`, `08abd2f`, `8c390bf`), `godot-v4.6` (`6474470`, `69c1d2b`, `aa37f52`) and `godot-v4.5` (`7c442a3`, `83ab065`, `0eec592`), local, not pushed. On each line with its pinned build (4.7.2, 4.6.3, 4.5.2): fmt, clippy, the default suite (130 unit tests and the integration suites), all 13 addon scripts, `editor_e2e_test` 12 of 12 and `addon_export_test` 2 of 2. The worktrees and the shared build directory were removed afterwards.
- **Notes (run):** The first carry failed on all three lines in the same two end-to-end tests: Godot 4.5.2, 4.6.3 and 4.7.2 give a script tab no `_tab_name` metadata, which 4.8-beta1 does, so the pairing rule always found the editor state unknown. Found only by running each line's own build; fixed on `master` (the tab-name check applies only where the metadata exists; `l2-godot-editor-addon.md` 1.4.2) and carried. While the carry ran, the automated security review flagged the temporary file of the edit fallback (a predictable name written with a link-following call could leave the project); fixed with an exclusive, unpredictable temporary file and a refusal to replace a symbolic link (`l2-tool-handlers.md` 1.3.2), with a Unix-only test that compiles on the Linux target but was not run here. No `has_method()` guard was needed on any line. Pushing the branches needs the maintainer's yes in the conversation.
