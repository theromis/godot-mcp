---
phase: 3
name: "Agent Feedback Channel"
status: Done
subsystem: "src (feedback, issues, config, tools, server), tests, README, .github"
requires: []
provides:
  - Agent feedback channel: godot_note_feedback, godot_draft_feedback, godot_submit_feedback
  - Scrubber, fingerprint and link builder; in-memory journal with a draft state machine
  - GitHub CLI submission with duplicate look-up, consent flag and cancel guard
  - Settings GODOT_MCP_FEEDBACK and GODOT_MCP_GH_PATH; user documentation and issue template
key_files:
  created:
    - src/feedback.rs
    - src/journal.rs
    - src/issues.rs
    - src/tools/feedback.rs
    - examples/fake_gh.rs
    - tests/feedback_test.rs
    - .github/ISSUE_TEMPLATE/agent-feedback.md
  modified:
    - src/tools.rs
    - src/server.rs
    - src/config.rs
    - src/bridge.rs
    - src/cli_runner.rs
    - tests/common/mod.rs
    - tests/stdio_protocol_test.rs
    - README.md
    - CHANGELOG.md
    - AGENTS.md
    - .env.example
patterns_established:
  - Outward-facing actions are a separate tool bound to a stored draft, with an explicit confirmation flag
  - Text bound for a public place is scrubbed at intake and again on the assembled body, and agent text is fenced
  - Test stand-ins for external programs are example targets rebuilt before use
duration_minutes: ~
---

# Stage 3 Tasks — Agent Feedback Channel

**Phase:** 3
**Status:** Done
**Strategic Goal:** An agent can record what is missing, broken or misleading while it works, get one scrubbed and formatted report, and either hand it to the human as text or submit it to the project's public issue tracker after the human's approval, with nothing leaving the machine otherwise.

## Atomic Checklist

<!-- A line is `- [ ]` (open) or `- [x]` (done). A Blocked task keeps `- [ ]`: it is blocked by its `Status` in Detailed Tracking, and the `[!]` the workflows write after "Blocked" spells that status — it is not a checkbox state. -->

- [x] [T-3A01] Scrubber, fingerprint, percent-encoding and link builder (tests first)
- [x] [T-3A02] Feedback journal: notes, evidence, counters, drafts and the report builder
- [x] [T-3B01] GitHub client runner: repository address, duplicate look-up, create, outcome mapping
- [x] [T-3B02] Stand-in `gh` executable and its test helper
- [x] [T-3C01] Feedback mode and `gh` path settings
- [x] [T-3C02] Note tool, catalog by mode, outcome and unknown-tool recording, `initialize` guidance
- [x] [T-3C03] Draft tool: environment block, evidence selection, link and delivery lines
- [x] [T-3C04] Submit tool: consent gate, draft state machine, submission through the stand-in `gh`
- [x] [T-3D01] User documentation, configuration template and the manual-path issue template
- [x] [T-3T01] Validate Phase 3 against the specifications and the verification pipeline

## Detailed Tracking

<!-- Optional entry field, created by /magic.run on a task's first dead end — absent means none. A nested list under the entry's field list, one line per abandoned approach, at most five (run.md, Dead-End Record):
  - **Attempts:**
    - {what was tried} → {why it failed}
-->

### [T-3A01] Scrubber, fingerprint, percent-encoding and link builder (tests first)

- **Spec:** l2-agent-feedback-rust.md §5.5, §5.6, §5.7, §4 (FB-4); l1-agent-feedback.md §3 FB-4
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo test --lib feedback::` passes with one test per scrubber class (control characters, exact bridge secret, project directory, home directory, Windows drive path, UNC path, Unix absolute path, `res://` and `user://` left alone, each credential prefix, long mixed token, mention, issue reference, `<`, `![`), a fixed-vector test for the fingerprint, a percent-encoding test, and link tests (total length ≤ 6,000 bytes, cut body ends with the shortened marker, no link when even the title cannot fit). Mutation check: removing any one scrub step makes its test fail; restore it afterwards.
- **Handoff:** None.
- **Notes:** New file `src/feedback.rs`, pure functions only, no `regex` dependency. Write the failing tests first.
- **Changes:** Added `src/feedback.rs` (scrub in six steps, shape, FNV-1a fingerprint, percent-encoding, link builder) with 26 unit tests; mutation check: disabling any of the 8 scrub steps fails a test. Specification refined to v1.0.1 (brace placeholders, fenced agent text, narrower credential rule).

### [T-3A02] Feedback journal: notes, evidence, counters, drafts and the report builder

- **Spec:** l2-agent-feedback-rust.md §5.3, §5.6, §2 (limits), §4 (FB-1, FB-5, FB-6, FB-11); l1-agent-feedback.md §5.3
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo test --lib feedback::` passes with tests for: the 21st note rejected with a message naming the limit 20; each field limit; the 11th draft rejected; evidence deduplicated by tool, kind and message with a count and evicting the oldest beyond 50; counters per tool; report section order and the `written by the agent` / `observed by the server` labels; `missing_feature` drafts include unknown-tool evidence and other kinds do not; a body over 24,000 characters is halved down to fit with the shortened note; the draft state moves as in the specification. `cargo clippy --all-targets --all-features -- -D warnings` is clean for the module.
- **Handoff:** None.
- **Notes:** Locks go through `util::lock`; no lock is held across an `await`; no `unwrap` or `expect` outside tests. Depends on T-3A01.
- **Changes:** Added `src/journal.rs` (notes, evidence ring with counts, per-tool counters, drafts with a Pending/Submitting/Submitted state machine, fenced report builder with size shortening, fingerprint from stable facts) with 30 unit tests, poison recovery included. The journal lives in its own module next to `feedback.rs` (text helpers); the module map in the specification needs this note.

### [T-3B01] GitHub client runner: repository address, duplicate look-up, create, outcome mapping

- **Spec:** l2-agent-feedback-rust.md §5.8, §4 (FB-3, FB-7, FB-9), §2 (time limits); l1-agent-feedback.md §3 FB-3, FB-9
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo test --lib issues::` passes: `https://github.com/zeegzoog/godot-mcp` (also with a trailing `.git` or `/`) gives `zeegzoog/godot-mcp`; outcome mapping covers an issue address on the last stdout line, exit 0 without an address (unknown), the log-in message, a network message, a rate-limit message and any other failure; the duplicate look-up counts a hit only when the body contains the fingerprint; the argument vectors match the contract table (title as one `--title=` argument, body only on stdin, no label option).
- **Handoff:** None.
- **Notes:** New file `src/issues.rs`. The child is awaited with a time limit and `kill_on_drop`; on Windows the server's own standard handles are made non-inheritable first (reuse the helper in `cli_runner.rs`, extracting it if needed). Prompts, colour and the update notice are switched off through the environment.
- **Changes:** Added `src/issues.rs` (repository address from the manifest, `gh` lookup without starting it, fixed argument vectors, duplicate parsing, outcome mapping with a timeout reported as unknown, awaited child with `kill_on_drop` and non-inheritable standard handles) with 14 unit tests; `make_std_handles_private` in `cli_runner.rs` made crate-visible for reuse.

### [T-3B02] Stand-in `gh` executable and its test helper

- **Spec:** l2-agent-feedback-rust.md §5.10; l2-test-suite.md §4 (QA-4)
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo build --examples` builds the stand-in; a test in `tests/common` or `tests/feedback_test.rs` runs it directly and asserts that it records its arguments one per line, writes standard input to a file, and returns the scripted stdout, stderr, exit code and delay.
- **Handoff:** Resolve the open note in l2-agent-feedback-rust.md §5.10 (how the stand-in is provided on Windows) via `/magic.spec amend l2-agent-feedback-rust.md`.
- **Notes:** A batch file is not an option on Windows (the runtime refuses to start one with arguments it cannot escape safely), so the stand-in is a small Rust example target found next to the test executable. Behaviour is scripted through environment variables of the server process. Not built by `cargo build`, so release artifacts stay one executable per platform.
- **Changes:** Added `examples/fake_gh.rs` (records arguments and standard input, answers from environment variables, exits 64 for anything the channel never issues) and `FakeGh`, `fake_gh()`, `as_pairs()` in `tests/common/mod.rs` (the helper builds the example once when the test run did not); 4 tests in `tests/feedback_test.rs`; a plain `cargo build` does not produce the stand-in.

### [T-3C01] Feedback mode and `gh` path settings

- **Spec:** l2-agent-feedback-rust.md §2 (configuration), l1-mcp-server.md §5.3, l2-rust-stdio-server.md §4 (MCP-7)
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo test --lib config::` and `cargo test --test stdio_protocol_test` pass new tests: `GODOT_MCP_FEEDBACK` unset or empty selects `full`; `OFF`, `Draft`, `full` are accepted in any case; `bogus` selects `full` and writes a warning to standard error only; `GODOT_MCP_GH_PATH` empty counts as unset; `--feedback` and `--gh-path` flags work and the environment values lose to flags.
- **Handoff:** None.
- **Notes:** Parser functions are infallible like the existing path parsers. Touches `src/config.rs` only.
- **Changes:** `Config` gained `feedback` (`FeedbackMode` off/draft/full, any case, empty or unknown selects full with a stderr warning) and `gh_path`; 4 unit tests plus a protocol test that the warning goes to standard error only; test sessions now clear both variables.

### [T-3C02] Note tool, catalog by mode, outcome and unknown-tool recording, `initialize` guidance

- **Spec:** l2-agent-feedback-rust.md §5.2, §5.4, §5.9, §4 (FB-5, FB-6, FB-10, FB-11); l1-tool-catalog.md §5.1; l1-mcp-server.md §2
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo test --test stdio_protocol_test` and `cargo test --test feedback_test` pass new tests: `tools/list` shows three feedback tools in `full`, two in `draft`, none in `off`; `initialize` carries `instructions` of at most 150 words that name the tools allowed by the mode, never name the submit tool in `draft`, and are absent in `off`; `godot_note_feedback` stores a note, answers with its id and the undrafted list, rejects bad enumerations and over-long fields with the named limit; a failing host tool call and a call to an unknown tool name are recorded as evidence (read back through the draft tool in T-3C03, or through a test accessor); the result of the same failing host tool is identical with the channel in `full` and in `off`.
- **Handoff:** None.
- **Notes:** Resolve the open note about how the mode reaches the static catalog (process-wide cell or catalog owned by the tool manager); record the choice in l2-agent-feedback-rust.md via `/magic.spec amend`. `server.rs` `initialize` is currently an associated function without access to state; give it what it needs. Serialize with T-3C01 (shared files).
- **Changes:** Added `src/tools/feedback.rs` (catalog entries by mode, guidance text, note handler, outcome and unknown-tool recording, project memory for scrubbing); `ToolManager` gained feedback state, `configure_catalog`, `with_feedback` and an `observe` hook that never alters a result; `initialize` answers carry `instructions` and capture the client; the catalog mode is a process-wide cell set before first use; 9 channel tests and 4 protocol tests green. Draft and submit handlers are placeholders until the next two tasks. Evidence read-back is verified through the draft tool.

### [T-3C03] Draft tool: environment block, evidence selection, link and delivery lines

- **Spec:** l2-agent-feedback-rust.md §5.4, §5.6, §5.7, §4 (FB-4, FB-6, FB-7, FB-8); l1-agent-feedback.md §5.2, §5.3, §5.4
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo test --test feedback_test` passes new tests with the mock editor and without one: the environment block shows server version, addon version read from the project's addon manifest (and `unknown` when the project does not resolve), engine version, platform, bridge state, client name and version; a bridge secret, a home path and a project path placed in a note never appear in the answer; mentions are neutralized; the answer contains the report in a fenced block, the link, the fingerprint, the `delivery` lines (submit listed only in `full` mode with `gh` found), the notes left out and the next-step instruction; a draft without notes is an error that says to record a note first; the environment look-ups finish within their 5 s budget when the editor stalls.
- **Handoff:** None.
- **Notes:** The addon manifest is read through `resolve_in_project`, read-only. The look-ups must not hold the journal lock.
- **Changes:** Draft handler in `src/tools/feedback.rs` (environment from the editor or the engine binary within a 5 s shared budget, addon version from the project manifest, delivery lines by mode and client availability, next-step instructions); 13 channel tests; mutation check of the three recording hooks. A privacy fix found by the tests: the home directory is shortened to `~` only once the project directory is known, otherwise such paths are hidden completely (the specification needs this note).

### [T-3C04] Submit tool: consent gate, draft state machine, submission through the stand-in `gh`

- **Spec:** l2-agent-feedback-rust.md §5.8, §4 (FB-2, FB-7, FB-8, FB-9); l1-agent-feedback.md §3 FB-2, FB-7, FB-8, FB-9
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo test --test feedback_test` passes new tests using the stand-in from T-3B02 (`GODOT_MCP_GH_PATH`): `confirmed` missing or false is an error with recovery steps and `gh` is not started; an unknown draft id lists the known ones; created returns the issue address and a second call returns it again without starting `gh`; duplicate with a hit, with and without `allow_duplicate`; failed look-up still creates and says so; not signed in, `gh` missing, exit 0 without an address (unknown), timeout; the recorded argument vector equals the contract and the stdin body equals the draft body byte for byte; two concurrent submits of one draft start exactly one create; a cancelled submit leaves the draft retriable; the server's standard output contains no `gh` output; the tool is absent in `draft` mode.
- **Handoff:** None.
- **Notes:** Switch the draft to `Submitting` under the lock before the first `await`; a drop guard returns it to `Pending` on cancellation. Depends on T-3B01, T-3B02, T-3C03.
- **Changes:** Submit handler (consent flag, draft claimed as Submitting before the first await, drop guard on cancellation, duplicate look-up with allow_duplicate, four outcomes) and `IssueClient::with_limits`; the stand-in learned `FAKE_GH_LIST_HIT_URL` and argument-driven delays, and the test helper always rebuilds it (a stale executable had hidden new behaviour); 15 submission tests (consent, once-only, concurrency, cancellation, argument vectors, stdout purity); mutation check of consent gate, in-progress refusal, final-outcome storage and cancel guard.

### [T-3D01] User documentation, configuration template and the manual-path issue template

- **Spec:** l2-agent-feedback-rust.md §6 (items 5, 6), §5.3 of l1-agent-feedback.md; l1-distribution.md §3 DS-4
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `README.md` lists the three tools in the tools table, describes the feedback channel (what is recorded, what is never gathered, how a report is delivered, the `off` and `draft` modes, the advice not to pre-approve the submit tool) and has the two settings in the configuration table; `.env.example` lists both settings with empty placeholders; `.github/ISSUE_TEMPLATE/agent-feedback.md` has front matter and the report's section headings; `CHANGELOG.md` has an `[Unreleased]` entry written for users; `AGENTS.md` has a short item telling agents when to note and when to draft. `grep -rniE "\.design|PLAN\.md|TASKS\.md|T-[0-9]+[A-Z][0-9]+|phase-[0-9]" README.md CHANGELOG.md .env.example .github/ISSUE_TEMPLATE src tests` prints nothing. The repository's markdown lint settings report no new issues.
- **Handoff:** Tell the maintainer, outside the code, to create the `agent-feedback` label so the template's label applies.
- **Notes:** README and CHANGELOG are user-facing: no mention of the planning workflow. The version is not bumped here; release bumps both manifests together.
- **Changes:** README (three tool rows, two settings, a Feedback section with what is recorded, delivery choices and the advice not to pre-approve the submit tool), `.env.example`, `CHANGELOG.md` Unreleased entry, `AGENTS.md` agent protocol item, `.github/ISSUE_TEMPLATE/agent-feedback.md` (label `agent-feedback`) and `config.yml`; markdown lint clean, no planning references in product files. The specifications were brought in line with the implementation (feedback spec 1.0.2, test suite 1.1.1).

### [T-3T01] Validate Phase 3 against the specifications and the verification pipeline

- **Spec:** l1-quality-assurance.md §3 QA-1, QA-2, QA-5; l1-agent-feedback.md §3 (FB-1 to FB-11)
- **Status:** Done
- **Assignment:** Agent
- **Verify:** `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-targets` pass; the real server binary driven over standard input (note, draft, then submit with `confirmed: false`) behaves as specified and writes nothing but JSON-RPC frames to standard output; no file under `addons/godot_mcp/` changed, so the editor end-to-end tests are not required (QA-3); `node .magic/scripts/executor.js check-prerequisites --json --workspace=main` reports `ok: true`.
- **Handoff:** `/magic.task main` to re-plan once all specs are fully satisfied.
- **Notes:** Run last. Depends on T-3A01..T-3D01. No real issue is created during validation.
- **Changes:** Validation run: `cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings` and `cargo test --all-targets` pass (94 unit, 37 channel, 23 protocol tests among others); no addon file changed, so the editor end-to-end tests were not required; a real-binary session over standard input (initialize, unknown tool, note, draft, submit without consent) produced only JSON-RPC frames, quoted the missing tool, hid the user's name and refused the unconfirmed submit; prerequisites report `ok: true`.
