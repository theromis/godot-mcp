---
phase: 12
name: "Reality Sync after 0.13.0"
status: Done
subsystem: ".design/main/specifications, CHANGELOG.md, README.md, SECURITY.md, CONTRIBUTING.md, AGENTS.md, docs"
requires: [11]
provides:
  - "CA-17 names the written and used kinds of a reference to a deleted file"
  - "every Canonical References row is one tracked path"
  - "the changelog keeps the released 0.13.0 apart from the unreleased changes"
key_files:
  created: []
  modified: []
patterns_established: []
duration_minutes: ~
---

# Stage 12 Tasks — Reality Sync after 0.13.0

**Phase:** 12
**Status:** Done
**Strategic Goal:** The specifications, the changelog and the guides say what the code does after the pull requests landed on 2026-11-04: the deletion requirement knows the written and used kinds of a reference, every path in a Canonical References table is a path, the changelog keeps the released and the unreleased apart, and the guides name the scenario tests, the build version and the new tools.

## Atomic Checklist

<!-- A line is `- [ ]` (open) or `- [x]` (done). A Blocked task keeps `- [ ]`: it is blocked by its `Status` in Detailed Tracking, and the `[!]` the workflows write after "Blocked" spells that status — it is not a checkbox state. -->

- [x] [T-12A01] Specifications: CA-17 reference kinds, Canonical References, delta markers
- [x] [T-12B01] Guides and changelog: unreleased entries, new tools, version a build reports, scenario tests, parity
- [x] [T-12T01] Validation: pipeline, markdown lint, addon suites, path and link checks

## Detailed Tracking

### [T-12A01] Specifications: CA-17 reference kinds, Canonical References, delta markers

- **Spec:** l1-content-authoring.md CA-17; l1-tool-catalog.md; l2-tool-handlers.md §5.10
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `node .magic/scripts/executor.js check-prerequisites --json --verify-headers --workspace=main` reports `ok: true` with 31 specs and no warning; a script that reads each Canonical References table by its `Path` column finds no cell with two paths and no path that is not tracked; no spec body outside Document History contains `[ADDED]` or `[MODIFIED]`.
- **Handoff:** T-12T01.
- **Notes:** `Implements:` takes one parent, so `l1-content-authoring.md` and `l1-project-export.md` stay without an L2 of their own: `l2-tool-handlers.md` §4 realizes them and lists both in Related Specifications. A stub L2 would repeat that table.
- **Changes:** CA-17 and the catalog row name the `written` and `used` kinds; the classifier and its shared cases are Canonical References; the two-path cells are two rows; the modules merged without an entry have rows; 23 specs lost their delta markers; patch and minor versions and the registry (1.9.4) follow.

### [T-12B01] Guides and changelog: unreleased entries, new tools, version a build reports, scenario tests, parity

- **Spec:** l1-community-health.md; l1-distribution.md; l2-community-health-files.md
- **Status:** Done
- **Assignment:** Agent
- **Verify:** Every `godot_` tool name in `README.md`, `AGENTS.md`, `CONTRIBUTING.md` and `docs/game-checks.md` is in the live `tools/list`; the 0.13.0 section of `CHANGELOG.md` holds only what `git merge-base --is-ancestor <commit> v0.13.0` confirms; no document describes using a product outside the technology scope.
- **Handoff:** T-12T01.
- **Notes:** The tag `v0.13.0` is at `0785e96`; the post-release entries (startup gate, uid cache, written references, covered window) and the two new tools moved to `[Unreleased]`. The CI workflow is unchanged on purpose: the scenario suite runs locally when its area changes, like the export suite (l2-test-suite.md).
- **Changes:** `CHANGELOG.md`; `README.md` (the third-party compatibility sentence removed, the safety of the two windowed tools); `SECURITY.md` (the reported version carries the commit); `CONTRIBUTING.md` and `AGENTS.md` (scenario and version tests, the agent setup skill); `docs/feature-parity.md` (the import row, the capabilities added after the comparison, source map, tests, gaps); one comment in `addons/godot_mcp/runtime/ops/rt_registry.gd`.

### [T-12T01] Validation: pipeline, markdown lint, addon suites, path and link checks

- **Spec:** l1-quality-assurance.md
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets`, `npx markdownlint-cli2` over the markdown files, the `--check-only` of every addon script, and with `GODOT_PATH` the editor, addon export and analyzer warning suites, one after another.
- **Handoff:** none.
- **Notes:** - **Changes:** On `master` with Godot 4.8.beta2: fmt and clippy clean; `cargo test --all-targets` all passed; markdownlint 0 issues in 114 files; 21 of 21 addon scripts pass `--check-only`; the analyzer warning audit and `addon_export_test` pass; `scenario_e2e_test` 5 of 5; `editor_e2e_test` 44 passed and 2 failed, `open_scene_returns_what_the_load_errors_dialog_shows` (the Russian dialog title) and `close_editor_quits_the_editor_and_its_process_exits` (the answer names the PID of the editor process, the test expects the one of its console wrapper); the second fails the same way on the tree without these changes.

The local editor runs in Russian and Godot is 4.8.beta2 while `.github/godot-version` names 4.8-beta1; name both in the report.
