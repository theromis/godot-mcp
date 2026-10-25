---
name: agents-init
description: >
  Universal agent environment initializer. Links agent config directories
  (.claude/, .qwen/, ...) to the canonical .agents/ workflows, skills and rules
  with junctions/symlinks, and instruction files (CLAUDE.md, ...) to AGENTS.md
  with hardlinks. Use it to set up or check a fresh clone or worktree.
argument-hint: "[claude|gemini|qwen|cursor|copilot|codex|kilocode|lingma|all] [--check] [--force] (default: claude)"
---

# Agents Init Skill

Sets up the mapping between the canonical `.agents/` directory and
agent-specific config directories/files. Works for **any project** — just
place `.agents/` at the repo root and run the skill.

## Usage

```
/agents-init              # default: claude  (-> .claude/)
/agents-init qwen         # only qwen        (-> .qwen/  + QWEN.md)
/agents-init claude qwen  # both agents
/agents-init all          # every supported agent
/agents-init --check      # verify only, change nothing (exit 1 on problems)
/agents-init --force      # replace copies that differ from the canonical content
```

## Running the Skill

Run the script for the current shell and pass the arguments through. Each
script finds the repository root from its own location, so the working
directory does not matter.

| Shell | Command |
| --- | --- |
| PowerShell (Windows) | `powershell -NoProfile -ExecutionPolicy Bypass -File .agents/skills/agents-init/scripts/setup_windows.ps1 [<agents>] [--check] [--force]` |
| Bash (Linux, macOS, Git Bash on Windows) | `bash .agents/skills/agents-init/scripts/setup_unix.sh [<agents>] [--check] [--force]` |

- Both scripts take the same arguments: agent tokens from the table below,
  separated by spaces or commas (none means `claude`), plus `--check` and
  `--force`. The PowerShell script also takes `-Agents <list>`, `-Check` and
  `-Force`. An unknown token is an error, not a silent default run.
- Git Bash, MSYS2 and Cygwin hand the run to the PowerShell script, so Windows
  always gets junctions.
- Show the user every `[FIX]`, `[WARN]` and `[FAIL]` line. Exit code 1 means at
  least one `[FAIL]`.
- Never add `-Force` / `--force` on your own. It overwrites a copy that differs
  from the canonical content, and that difference may be someone's work: quote
  the `[FAIL]` lines and ask first.

## Supported Agents

| Token | Config dir | Linked directories | Instruction files |
| --- | --- | --- | --- |
| `claude` | `.claude/` | `commands`, `skills`, `rules` | *(none, see below)* |
| `gemini` | `.agents/` | *(none)* | `GEMINI.md` |
| `qwen` | `.qwen/` | `commands`, `skills`, `rules` | `QWEN.md` |
| `cursor` | `.cursor/` | `rules` | *(none)* |
| `copilot` | `.github/` | `prompts`, `skills`, `rules` | `.github/copilot-instructions.md` |
| `codex` | `.codex/` | `prompts`, `skills`, `rules` | `CODEX.md` |
| `kilocode` | `.kilocode/` | `workflows`, `skills`, `rules` | *(none)* |
| `lingma` | `.lingma/` | `commands`, `skills`, `rules` | `LINGMA.md` |

> **Default** (no argument) = `claude`.

Claude Code reads `CLAUDE.md`, not `AGENTS.md`. The tracked `CLAUDE.md` of a
repository is a one-line stub, `@AGENTS.md`, which Claude Code expands on
read, so no link and no second copy of the text is needed.

## Canonical Source

`.agents/` is always the single source of truth:

```
.agents/
  workflows/   ← agent "commands"
  skills/      ← agent skill libraries
  rules/       ← shared rule files
```

## Bootstrap (First Run)

The skill cannot be invoked as `/agents-init` until `.claude/skills/` exists,
and a fresh clone or a new worktree has no links. Run the script yourself the
first time:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .agents/skills/agents-init/scripts/setup_windows.ps1
```

```bash
bash .agents/skills/agents-init/scripts/setup_unix.sh
```

`-ExecutionPolicy Bypass` applies to that one run only. Without it, the default
Windows policy (Restricted) refuses to run any script.

On Linux and macOS the script needs bash 3.2 or newer and a working `python3`,
`python` or `jq` to read the registry.

## What Each Run Does

1. Parses target agents from arguments (default: `claude`).
2. Makes sure `.agents/workflows`, `.agents/skills` and `.agents/rules` exist
   (git does not keep empty directories, so a fresh clone may lack them).
3. For each path of each target agent:
   - Already the right link: left alone.
   - Missing, broken, or a link to somewhere else: a junction (Windows) or a
     relative symlink (Unix) is created in its place. An old link is removed as
     a link; what it pointed to is never touched.
   - A real directory (for example a leftover copy): replaced by the link only
     if every file in it is identical to the canonical one and every link in it
     points at its canonical counterpart. Otherwise it is reported and left
     alone until you review it and re-run with `-Force` / `--force`.
   - On Unix, a real directory that git still tracks stays as it is: git does
     not look through a symlink and would report every file in it as deleted.
     Untrack it first (see Git Policy).
   - Instruction files (`CLAUDE.md`, `QWEN.md`, ...) become hardlinks to
     `AGENTS.md`, with the same identical-or-refuse rule. A file that differs is
     reported as newer or older than `AGENTS.md`, because `-Force` / `--force`
     overwrites it with `AGENTS.md`.
   - A link that git neither tracks nor ignores is added to
     `.git/info/exclude`, so it does not show in `git status`.
4. Checks every path again and prints one line for each: `[OK]`, `[WARN]` (an
   identical copy instead of a link) or `[FAIL]`. The exit code is 1 if any
   path is `[FAIL]`. `-Check` / `--check` runs only this step.

## Git Policy

`.agents/` is the only committed copy of workflows, skills and rules. The links
in the agent directories (`.claude/skills`, `.qwen/skills`, ...) are local and
never committed, so the same files are not stored twice. `.gitignore`
re-includes only the repository's own skills, and only under `.agents/skills/`.

- CI does not need the links: workflows read `.agents/skills/...` by path.
- A fresh clone or a new worktree has no links. Run the script once; until then
  `-Check` reports `[FAIL] ... missing`.
- Instruction files (`QWEN.md`, `GEMINI.md`, ...) are hardlinks to `AGENTS.md`
  made locally and excluded from git status. A hardlink lasts only while every
  writer edits the file in place. `git checkout`, `pull` and `rebase` replace
  files, and so does the file editing of Claude Code: after such a write the two
  names are separate files, and the next edit reaches only one of them. `-Check`
  reports the difference; re-run with `-Force` / `--force` once `AGENTS.md` holds
  the newer text. Prefer an import line (as `CLAUDE.md` does) for agents that
  support one.
- The scripts never touch the git index. To untrack a path left over from an
  older layout, name its files rather than the directory (see Windows Notes).

## Agent Registry

Agent configuration is defined in [`agents.json`](scripts/agents.json).
To add a new agent — edit only that file; the scripts read it automatically.
Every path must be relative and stay inside the repository; the scripts refuse
the registry otherwise.

```jsonc
{
    "claude": {
        "dir": ".claude",              // config directory at repo root
        "workflows": "commands",       // link name for .agents/workflows (null = skip)
        "skills": "skills",            // link name for .agents/skills (null = skip)
        "rules": "rules",              // link name for .agents/rules (null = skip)
        "files": ["CLAUDE.md"],        // hardlinks -> AGENTS.md, paths relative to the repo root
        "description": "Claude Code (Anthropic)"
    },
    ...
}
```

## Resources

- [agents.json](scripts/agents.json)                     — agent registry
- [scripts/setup_windows.ps1](scripts/setup_windows.ps1) — PowerShell (Windows)
- [scripts/setup_unix.sh](scripts/setup_unix.sh)         — Bash (Unix/macOS; hands off to PowerShell on Windows)

## Windows Notes

- Junctions and hardlinks need no administrator rights or Developer Mode;
  symlinks do, which is why nothing on Windows creates symlinks.
- A link is deleted with `Directory.Delete` / `File.Delete` on the link itself,
  which never enters the directory it points to.
- When you untrack a linked path by hand, list specific files rather than a
  directory, so git cannot traverse a junction into the canonical sources:

  ```bash
  git rm --cached --ignore-unmatch .claude/skills/release/SKILL.md
  ```
