# CI & Release Pipeline

**Version:** 1.0.1
**Status:** Stable
**Layer:** implementation
**Implements:** l1-distribution.md

## Overview

The GitHub Actions workflows, helper script, manifests and agent skill that build, verify and publish godot-mcp releases, and the repository hygiene that keeps machine-specific files and development scaffolding out of what is shipped.

## Related Specifications

- [l1-distribution.md](l1-distribution.md) - Parent concept.
- [l2-test-suite.md](l2-test-suite.md) - The checks the pipeline runs.

## 1. Motivation

Release mechanics are spread over several files (two workflows, a shell script, two manifests, a changelog, an agent skill, ignore rules). This specification lists which file enforces which distribution rule.

## 2. Constraints & Assumptions

- Runners: `ubuntu-24.04` (pinned), `windows-latest`, `macos-latest`; actions `checkout@v7`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `softprops/action-gh-release@v3`.
- CI installs Godot `4.5-stable` from the official builds repository for the editor job.
- Release profile: `lto = true`, `codegen-units = 1`, `strip = true`.
- The changelog follows Keep a Changelog headings (`## [X.Y.Z] - YYYY-MM-DD`, `## [Unreleased]`) with compare links at the bottom.
- Assumption (forecast): the specification-driven workflow's automatic changelog writer is switched off for this project, because the changelog doubles as the published release text. Runner-up: keep it on and hand-clean every release. Override: `finalization.autoChangelog` in the workspace configuration.

## 4. Invariant Compliance

| L1 Invariant | Implementation |
| --- | --- |
| DS-1 One version | `tests/version_sync_test.rs` compares `Cargo.toml` and `addons/godot_mcp/plugin.cfg`; the release skill bumps both in one commit. |
| DS-2 Self-contained binaries | Release `build` matrix runs `cargo build --release --locked` for the four target triples with the release profile and archives the single executable per target. |
| DS-3 Gated publication | `release.yml` `verify` job: tag must equal `v{Cargo.toml version}`, `release-notes.sh` must find a non-empty section, then fmt/clippy/test; packaging and builds depend on it. `ci.yml` also fails on Linux when the current version has no changelog section. |
| DS-4 User-facing notes only | Notes are hand-written per release (release skill step 3); the automatic changelog writer of the planning workflow is disabled (see Constraints). |
| DS-5 Separate, editor-only addon | `zip -r godot-mcp-addon.zip addons/godot_mcp`; export exclusion is in the addon (see [l2-godot-editor-addon.md](l2-godot-editor-addon.md)); README documents the disabled-plugin case. |
| DS-6 No machine-specific state | `.gitignore` excludes `.env*` (except `.env.example`), `.mcp.json`, `target/`, `bin/`, `dist/`, `.godot/`, `export_presets.cfg`, local binaries; README tells users to commit `.mcp.json.example` instead. |
| DS-7 Human confirmation | The release skill stops before `git tag`/`git push` and requires explicit approval. |
| DS-8 Scaffold-free artifacts | Binary and addon archives contain only built executables and `addons/godot_mcp`. The engine directories are git-ignored. `.gitattributes` marks the planning directory `export-ignore`, so GitHub's automatic source archives of a tag leave it out. |

## 5. Detailed Design

### 5.1 Files

```plaintext
.github/
├── workflows/ci.yml            fmt, clippy, tests (3 OS), changelog check, Godot 4.5 editor job
├── workflows/release.yml       verify → addon package + notes, 4-platform build → GitHub Release
└── scripts/release-notes.sh    print the changelog section for a tag; fail if missing/empty
.agents/skills/release/SKILL.md step-by-step release procedure for agents
CHANGELOG.md                     user-facing notes, one section per version
Cargo.toml / Cargo.lock          server version, dependencies, release profile
addons/godot_mcp/plugin.cfg      addon version
.gitignore / .gitattributes      ignore rules, LF line endings
```

## 6. Implementation Notes

- `git archive HEAD | tar -t` listing no planning directory is the check for DS-8.

## 7. Drawbacks & Alternatives

- **release-please / changesets** for automated notes: would generate notes from commit messages, which here are often internal; hand-written notes stay user-focused.

## Canonical References

| Alias | Path | Purpose |
| --- | --- | --- |
| `[CI]` | `.github/workflows/ci.yml` | Push/PR pipeline |
| `[RELEASE]` | `.github/workflows/release.yml` | Tag-triggered release |
| `[NOTES]` | `.github/scripts/release-notes.sh` | Changelog section extraction |
| `[SKILL]` | `.agents/skills/release/SKILL.md` | Agent release procedure |
| `[CHANGELOG]` | `CHANGELOG.md` | Release notes |
| `[MANIFEST]` | `Cargo.toml` | Version and release profile |
| `[LOCK]` | `Cargo.lock` | Pinned dependency graph |
| `[IGNORE]` | `.gitignore` | Repository hygiene |
| `[ATTR]` | `.gitattributes` | Line endings, archive attributes |
| `[LICENSE]` | `LICENSE` | MIT license shipped with the source |

## Document History

| Version | Date | Author | Description |
| --- | --- | --- | --- |
| 1.0.0 | 2026-10-16 | Agent (magic.analyze first-time) | Reverse-engineered from the workflows, script and release skill at 94e4540; DS-8 gap recorded. |
| 1.0.1 | 2026-10-16 | Agent (magic.run) | DS-8 gap closed by the `export-ignore` attribute. |
