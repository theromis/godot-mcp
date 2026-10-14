---
name: release
description: Cut a new godot-mcp release - bump the version, run QA gates, tag, and push to trigger the cross-platform GitHub Actions release build. Use when the user asks to "release", "cut a release", "publish a new version", or "ship vX.Y.Z" of godot-mcp.
---

# Release godot-mcp

## Overview

Pushing a tag matching `v*.*.*` triggers `.github/workflows/release.yml`, which first verifies the tag matches `Cargo.toml` and runs fmt/clippy/tests, then builds `godot-mcp` for Windows x86_64, Linux x86_64, macOS x86_64, and macOS aarch64, and attaches the archives to a GitHub Release. This skill walks through the steps in order — skipping the QA gate or getting the tag format wrong produces a broken or missing release.

## Procedure

1. **Determine the new version.** Read the current version from `Cargo.toml` (`[package] version`). If the user didn't specify a target version, ask them, or propose a semver bump (patch/minor/major) based on what changed since the last tag (`git log <last-tag>..HEAD --oneline`). Don't guess silently.

2. **Preflight — all must pass before continuing:**

   ```bash
   git status --porcelain          # must be empty; stop and ask if it isn't
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-targets
   ```

   If a Godot 4.5+ binary is reachable (`GODOT_PATH` exported, or `godot` on `PATH`), also run (the repo root is itself a minimal Godot project so `class_name` resolves — `--path .` is required, and `--import` must run at least once for a fresh `.godot/` cache):

   ```bash
   godot --headless --path . --import
   godot --headless --check-only --path . --script addons/godot_mcp/plugin.gd
   godot --headless --check-only --path . --script addons/godot_mcp/bridge_server.gd
   GODOT_PATH=/path/to/godot cargo test --test editor_e2e_test -- --ignored
   ```

   Stop and report if any check fails. Do not tag a red build.

3. **Bump the version in both manifests** — the server and the addon always ship the same version:

   - `Cargo.toml`: `[package] version = "X.Y.Z"`
   - `addons/godot_mcp/plugin.cfg`: `version="X.Y.Z"`

   Then run `cargo build` once so `Cargo.lock`'s matching entry updates, and `cargo test --test version_sync_test` to confirm the two versions match (the test fails if they drift).

4. **Commit the bump:**

   ```bash
   git add Cargo.toml Cargo.lock addons/godot_mcp/plugin.cfg
   git commit -m "chore(release): bump version to vX.Y.Z"
   ```

5. **Stop and confirm with the user before step 6.** Tagging and pushing is the point of no return: it publishes a public GitHub Release and can't be casually undone. Show them the version and the commit, and get an explicit go-ahead — this is not a step to run automatically even inside this skill.

6. **Tag and push** (only after confirmation):

   ```bash
   git tag -a vX.Y.Z -m "vX.Y.Z"
   git push origin <branch>
   git push origin vX.Y.Z
   ```

7. **Watch the build:**

   ```bash
   gh run watch $(gh run list --workflow=release.yml -L 1 --json databaseId -q '.[0].databaseId')
   ```

   All 6 jobs (`verify`, `addon`, plus the 4 binary matrix targets `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`) must succeed.

8. **Verify the release:**

   ```bash
   gh release view vX.Y.Z
   ```

   Confirm all 5 assets are attached: 4 binary archives (`godot-mcp-<target>.{zip,tar.gz}`) plus `godot-mcp-addon.zip` (the Godot Editor plugin, platform-independent) — see the asset table in `README.md`.

9. **Optional:** refresh the local `bin/` convenience copy (git-ignored) from `target/release` for local MCP client configs. This is a personal dev-environment step, not part of the release itself.

## Notes

- Tag format must be exactly `v*.*.*` (e.g. `v0.2.0`) — the workflow only triggers on that glob.
- If the build fails after the tag is already pushed, prefer fixing forward with a new patch tag over deleting and re-pushing the same tag.
- This procedure never force-pushes, deletes tags, or deletes releases. If the user wants any of that, treat it as a separate, explicitly-confirmed action, not part of a normal release.
