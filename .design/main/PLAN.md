# Implementation Plan

**Version:** 1.1.0
**Generated:** 2026-10-16
**Based on:** .design/main/INDEX.md v1.2.0
**Status:** Active

## Overview

The registry was bootstrapped from existing code (godot-mcp 0.3.0 plus unreleased changes at commit 94e4540), so Phases 0 and 1 describe what is already implemented and verified. Phase 2 closes the gaps the first analysis found: a shipped-artifact hygiene gap, a stale comment, an unverified security property, and contributor guidance for the specification-driven workflow. Phase 3 adds the agent feedback channel: tools that let an agent record what is missing or broken and hand the maintainers a scrubbed report as text or as an approved issue submission. Roadmap concepts wait in the Backlog until their open questions are answered.

## Phase 0 — Requirements (Layer 1: Concept)

*Abstract specifications — technology-agnostic contracts. Implemented before the registry existed.*

- [x] **MCP Server Core** ([l1-mcp-server.md](specifications/l1-mcp-server.md)) [L1]
- [x] **Live Editor Bridge** ([l1-editor-bridge.md](specifications/l1-editor-bridge.md)) [L1]
- [x] **Tool Catalog** ([l1-tool-catalog.md](specifications/l1-tool-catalog.md)) [L1]
- [x] **Quality Assurance** ([l1-quality-assurance.md](specifications/l1-quality-assurance.md)) [L1]
- [ ] **Release & Distribution** ([l1-distribution.md](specifications/l1-distribution.md)) [L1] — DS-8 open (Phase 2)

## Phase 1 — Baseline Implementation (Layer 2)

*Reverse-engineered from the code; compliance tables verified against the source.*

- [x] **Rust Stdio Server** ([l2-rust-stdio-server.md](specifications/l2-rust-stdio-server.md)) [L2]
- [x] **Bridge Client (Rust)** ([l2-bridge-client-rust.md](specifications/l2-bridge-client-rust.md)) [L2]
- [x] **Tool Handlers (Rust)** ([l2-tool-handlers.md](specifications/l2-tool-handlers.md)) [L2]
- [ ] **Godot Editor Addon** ([l2-godot-editor-addon.md](specifications/l2-godot-editor-addon.md)) [L2] — token-file permission check and dev-project comment open (Phase 2)
- [ ] **CI & Release Pipeline** ([l2-ci-release-pipeline.md](specifications/l2-ci-release-pipeline.md)) [L2] — DS-8 gap open (Phase 2)
- [ ] **Test Suite** ([l2-test-suite.md](specifications/l2-test-suite.md)) [L2] — contributor guide lacks the specification workflow (Phase 2)

## Phase 2 — Alignment & Hardening

*Closes the gaps recorded by the first analysis. See [TASKS.md](TASKS.md).*

- [ ] Keep the planning directory out of source archives (DS-8)
- [ ] Prove the token file is owner-only on Unix (BR-2)
- [ ] Correct the stale class_name note in the development project
- [ ] Document the specification-driven workflow for contributors

## Phase 3 — Agent Feedback Channel (Layers 1 and 2)

*Specified together: the concept first, then its Rust realization. See [TASKS.md](TASKS.md).*

- [x] **Agent Feedback Channel** ([l1-agent-feedback.md](specifications/l1-agent-feedback.md)) [L1]
- [x] **Agent Feedback Channel (Rust)** ([l2-agent-feedback-rust.md](specifications/l2-agent-feedback-rust.md)) [L2]
- [x] Record, scrub, draft and submit with the human's approval; documented for users and covered by tests

## Backlog

<!-- Registered specifications waiting for prioritization (Draft or non-critical Stable) -->
- [l1-runtime-observability.md](specifications/l1-runtime-observability.md) *(Parked — ownership of runtime access undecided; see its TBD markers)*
- [l1-content-authoring.md](specifications/l1-content-authoring.md) *(Parked — update format and headless support undecided; see its TBD markers)*
