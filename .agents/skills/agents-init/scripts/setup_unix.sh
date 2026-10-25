#!/usr/bin/env bash
# ===============================================================================
# AGENTS INIT (UNIX)
# Universal agent environment initializer.
#
# Links agent-specific config directories to the canonical .agents/ directory
# with relative symlinks and instruction files to AGENTS.md with hardlinks. The
# links are local: each one is added to .git/info/exclude unless git already
# tracks or ignores it, and the git index is never touched. An existing real
# directory or file is replaced by a link only when it is identical to the
# canonical content. Works with bash 3.2 (macOS default).
#
# On Windows (Git Bash, MSYS2, Cygwin) the run is handed to setup_windows.ps1:
# symlinks there need administrator rights or Developer Mode, and without them
# `ln -s` silently makes a copy. Junctions need neither.
#
# Usage:
#   setup_unix.sh                 # default: claude
#   setup_unix.sh qwen
#   setup_unix.sh claude qwen
#   setup_unix.sh all
#   setup_unix.sh --check [...]   # verify only, change nothing (exit 1 on problems)
#   setup_unix.sh --force [...]   # replace copies that differ from the canonical content
# ===============================================================================

set -euo pipefail
set -f

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"

case "$(uname -s 2>/dev/null || true)" in
    MINGW*|MSYS*|CYGWIN*)
        agents=""
        flags=""
        for arg in "$@"; do
            case "$arg" in
                --check) flags="$flags -Check" ;;
                --force) flags="$flags -Force" ;;
                -*)      echo "ERROR: Unknown option '$arg'. Options: --check, --force." >&2; exit 1 ;;
                *)       agents="${agents:+$agents,}$arg" ;;
            esac
        done
        # A PSModulePath inherited from PowerShell 7 breaks module loading in
        # Windows PowerShell; without it, Windows PowerShell uses its own default.
        # shellcheck disable=SC2086
        exec env -u PSModulePath powershell.exe -NoProfile -ExecutionPolicy Bypass \
            -File "$(cygpath -w "$SCRIPT_DIR/setup_windows.ps1")" -Agents "${agents:-claude}" $flags
        ;;
esac

REGISTRY_FILE="$SCRIPT_DIR/agents.json"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd -P)"

if [ ! -d "$REPO_ROOT/.agents" ]; then
    echo "ERROR: Not a repository root (no .agents directory): $REPO_ROOT. Run the copy of this script that lives in <repo>/.agents/skills/agents-init/scripts." >&2
    exit 1
fi
if [ ! -f "$REGISTRY_FILE" ]; then
    echo "ERROR: Registry not found: $REGISTRY_FILE" >&2
    exit 1
fi

# Every path below is relative to the repository root, not the caller's cwd.
cd "$REPO_ROOT"

# -------------------------------------------------------------------------------
# 1. Agent registry (agents.json), parsed once with python3, python or jq.
#    A command is only used if it actually runs: on some systems python3 is a
#    stub that exists on PATH but fails when executed.
# -------------------------------------------------------------------------------

PARSER=""
for candidate in python3 python; do
    if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'import json' >/dev/null 2>&1; then
        PARSER="$candidate"
        break
    fi
done
if [ -z "$PARSER" ] && command -v jq >/dev/null 2>&1; then
    PARSER="jq"
fi
if [ -z "$PARSER" ]; then
    echo "ERROR: a working python3, python or jq is required to parse agents.json" >&2
    exit 1
fi

# One line per agent: name|dir|workflows|skills|rules|files (files space-separated).
registry_lines() {
    if [ "$PARSER" = "jq" ]; then
        jq -r 'to_entries[] | [.key, (.value.dir // ""), (.value.workflows // ""), (.value.skills // ""),
               (.value.rules // ""), ((.value.files // []) | join(" "))] | join("|")' "$REGISTRY_FILE"
    else
        "$PARSER" -c '
import json, sys
for name, cfg in json.load(open(sys.argv[1])).items():
    fields = [name, cfg.get("dir") or ""]
    fields += [cfg.get(key) or "" for key in ("workflows", "skills", "rules")]
    fields.append(" ".join(cfg.get("files") or []))
    print("|".join(fields))
' "$REGISTRY_FILE"
    fi
}

if ! REGISTRY="$(registry_lines | tr -d '\r')"; then
    echo "ERROR: agents.json could not be parsed" >&2
    exit 1
fi
ALL_AGENTS="$(printf '%s\n' "$REGISTRY" | cut -d'|' -f1 | tr '\n' ' ')"

# Sets AG_DIR, AG_WORKFLOWS, AG_SKILLS, AG_RULES and AG_FILES for one agent.
load_agent() {
    local name
    while IFS='|' read -r name AG_DIR AG_WORKFLOWS AG_SKILLS AG_RULES AG_FILES; do
        if [ "$name" = "$1" ]; then return 0; fi
    done <<EOF
$REGISTRY
EOF
    return 1
}

# A registry path must stay inside the repository.
valid_rel_path() {
    case "$1" in
        ""|/*|*:*|*\\*) return 1 ;;
    esac
    case "/$1/" in
        */../*) return 1 ;;
    esac
    return 0
}

for agent in $ALL_AGENTS; do
    load_agent "$agent"
    # shellcheck disable=SC2086
    for p in "$AG_DIR" $AG_WORKFLOWS $AG_SKILLS $AG_RULES $AG_FILES; do
        if ! valid_rel_path "$p"; then
            echo "ERROR: agents.json: agent '$agent' has an invalid path '$p' (must be relative, inside the repository)." >&2
            exit 1
        fi
    done
done

# -------------------------------------------------------------------------------
# 2. Parse arguments
# -------------------------------------------------------------------------------

CHECK=0
FORCE=0
TARGETS=""

add_target() {
    case " $TARGETS " in
        *" $1 "*) ;;
        *) TARGETS="${TARGETS:+$TARGETS }$1" ;;
    esac
}

# ${1+"$@"}: old bash versions treat an empty "$@" as unbound under set -u.
for arg in ${1+"$@"}; do
    case "$arg" in
        --check) CHECK=1 ;;
        --force) FORCE=1 ;;
        -*)      echo "ERROR: Unknown option '$arg'. Options: --check, --force." >&2; exit 1 ;;
        all)     for a in $ALL_AGENTS; do add_target "$a"; done ;;
        *)
            case " $ALL_AGENTS " in
                *" $arg "*) add_target "$arg" ;;
                *) echo "ERROR: Unknown agent '$arg'. Supported: $ALL_AGENTS" >&2; exit 1 ;;
            esac
            ;;
    esac
done
[ -n "$TARGETS" ] || TARGETS="claude"

# -------------------------------------------------------------------------------
# 3. Helpers
# -------------------------------------------------------------------------------

CHANGES=0
PROBLEMS=0
VERIFY=$CHECK

# The apply pass reports only what it changed; the verification pass reports every path.
status() {
    if [ "$VERIFY" = 0 ] && [ "$1" != FIX ]; then return 0; fi
    printf '  [%s] %s%s\n' "$1" "$2" "${3:+  ($3)}"
    case "$1" in
        FIX)  CHANGES=$((CHANGES + 1)) ;;
        FAIL) PROBLEMS=$((PROBLEMS + 1)) ;;
    esac
}

# sed reads all of its input, so a long list cannot end the script with SIGPIPE.
details() {
    local n
    if [ "$VERIFY" = 0 ]; then return 0; fi
    printf '%s\n' "$1" | sed -n '1,5s/^/         /p'
    n="$(printf '%s\n' "$1" | wc -l | tr -d ' ')"
    if [ "$n" -gt 5 ]; then echo "         ... and $((n - 5)) more"; fi
}

EXCLUDE_FILE=""
GIT_PREFIX=""
if command -v git >/dev/null 2>&1 && p="$(git -C "$REPO_ROOT" rev-parse --git-path info/exclude 2>/dev/null)"; then
    case "$p" in
        /*) EXCLUDE_FILE="$p" ;;
        *)  EXCLUDE_FILE="$REPO_ROOT/$p" ;;
    esac
    # Exclude patterns are relative to the top of the work tree, which is not the
    # project root when the project is a subdirectory of a larger repository.
    GIT_PREFIX="$(git -C "$REPO_ROOT" rev-parse --show-prefix 2>/dev/null || true)"
fi

is_tracked() { git -C "$REPO_ROOT" ls-files --error-unmatch -- "$1" >/dev/null 2>&1; }

# Keeps a local link out of `git status` without touching the committed .gitignore.
add_local_exclude() {
    if [ "$VERIFY" = 1 ] || [ -z "$EXCLUDE_FILE" ]; then return 0; fi
    if is_tracked "$1"; then return 0; fi
    if git -C "$REPO_ROOT" check-ignore -q -- "$1" 2>/dev/null; then return 0; fi
    mkdir -p "$(dirname "$EXCLUDE_FILE")"
    if [ -s "$EXCLUDE_FILE" ] && [ -n "$(tail -c 1 "$EXCLUDE_FILE")" ]; then
        printf '\n' >> "$EXCLUDE_FILE"
    fi
    printf '/%s%s\n' "$GIT_PREFIX" "$1" >> "$EXCLUDE_FILE"
    status FIX "$1" "git-ignored locally in .git/info/exclude"
}

# Entries of a real directory that are absent from, or differ from, the canonical
# one. Entries that exist only in the canonical directory are fine (git-ignored
# extras). A link inside the copy is never entered: it is fine only if it
# resolves to its canonical counterpart.
copy_drift() {
    local copy="$1" canon="$2" rel
    ( cd "$copy" && find . \( -type f -o -type l \) -print ) | while IFS= read -r rel; do
        rel="${rel#./}"
        if [ -L "$copy/$rel" ]; then
            if ! [ "$copy/$rel" -ef "$canon/$rel" ]; then
                echo "link in copy: $rel -> $(readlink "$copy/$rel")"
            fi
        elif [ ! -f "$canon/$rel" ]; then
            echo "only in copy: $rel"
        elif ! cmp -s "$copy/$rel" "$canon/$rel"; then
            echo "differs: $rel"
        fi
    done
}

sync_dir_link() {
    local path="$1" canon_rel="$2" dir depth prefix expected drift note i
    dir="$(dirname "$path")"

    if [ ! -d "$canon_rel" ]; then
        if [ "$VERIFY" = 1 ]; then
            if [ -e "$path" ] || [ -L "$path" ]; then status FAIL "$path" "canonical $canon_rel is missing"
            else status OK "$path" "nothing to link, $canon_rel does not exist"; fi
            return 0
        fi
        # Git does not keep empty directories, so a fresh clone may lack them.
        mkdir -p "$canon_rel"
    fi

    # Symlink target relative to the link's own directory: one ../ per path segment.
    depth="$(printf '%s' "$dir" | tr -cd '/' | wc -c | tr -d ' ')"
    prefix="../"
    i=0
    while [ "$i" -lt "$depth" ]; do prefix="../$prefix"; i=$((i + 1)); done
    if [ "$dir" = "." ]; then prefix=""; fi
    expected="${prefix}${canon_rel}"

    if [ -L "$path" ]; then
        if [ "$(readlink "$path")" = "$expected" ] && [ -d "$path" ]; then
            status OK "$path" "symlink -> $expected"
            add_local_exclude "$path"
            return 0
        fi
        if [ "$VERIFY" = 1 ]; then status FAIL "$path" "link to $(readlink "$path"), expected $expected"; return 0; fi
        rm -f "$path"
    elif [ -d "$path" ]; then
        drift="$(copy_drift "$path" "$canon_rel")"
        if is_tracked "$path"; then
            # Git does not look through a symlink: replacing a committed directory
            # would show every file in it as deleted. A tracked copy stays a copy.
            if [ -n "$drift" ]; then
                status FAIL "$path" "tracked copy differs from $canon_rel; update it or untrack it"
                details "$drift"
            else
                status WARN "$path" "tracked copy of $canon_rel; git stores it twice, untrack it to use a link"
            fi
            return 0
        fi
        if [ -n "$drift" ]; then
            if [ "$VERIFY" = 1 ] || [ "$FORCE" = 0 ]; then
                note="copy differs from $canon_rel"
                if [ "$FORCE" = 0 ]; then note="$note; review it, then re-run with --force to replace it"; fi
                status FAIL "$path" "$note"
                details "$drift"
                return 0
            fi
        elif [ "$VERIFY" = 1 ]; then
            status WARN "$path" "identical copy of $canon_rel, not a link; run without --check to link it"
            return 0
        fi
        # rm -r removes links inside the copy as links, it never follows them.
        rm -rf "$path"
    elif [ -e "$path" ]; then
        status FAIL "$path" "exists and is not a directory"
        return 0
    elif [ "$VERIFY" = 1 ]; then
        status FAIL "$path" "missing; run without --check to create it"
        return 0
    fi

    mkdir -p "$dir"
    ln -s "$expected" "$path"
    status FIX "$path" "symlink -> $expected"
    add_local_exclude "$path"
}

sync_file_link() {
    local path="$1" src="AGENTS.md" note age

    if [ ! -f "$src" ]; then status FAIL "$path" "AGENTS.md is missing"; return 0; fi
    if [ -d "$path" ]; then status FAIL "$path" "exists and is not a regular file"; return 0; fi

    if [ -f "$path" ]; then
        if [ "$path" -ef "$src" ]; then
            status OK "$path" "hardlink -> AGENTS.md"
            add_local_exclude "$path"
            return 0
        fi
        if ! cmp -s "$path" "$src"; then
            if [ "$VERIFY" = 1 ] || [ "$FORCE" = 0 ]; then
                # --force overwrites this file with AGENTS.md, so say which side was edited last.
                if [ "$path" -nt "$src" ]; then age="newer"; else age="older"; fi
                note="differs from AGENTS.md and is $age"
                if [ "$FORCE" = 0 ]; then note="$note; move any changes into AGENTS.md, then re-run with --force to link it"; fi
                status FAIL "$path" "$note"
                return 0
            fi
        elif [ "$VERIFY" = 1 ]; then
            status WARN "$path" "identical copy of AGENTS.md, not a hardlink; run without --check to link it"
            return 0
        fi
        rm -f "$path"
    elif [ -L "$path" ]; then
        if [ "$VERIFY" = 1 ]; then status FAIL "$path" "broken link"; return 0; fi
        rm -f "$path"
    elif [ -e "$path" ]; then
        status FAIL "$path" "exists and is not a regular file"
        return 0
    elif [ "$VERIFY" = 1 ]; then
        status FAIL "$path" "missing; run without --check to create it"
        return 0
    fi

    mkdir -p "$(dirname "$path")"
    ln "$src" "$path"
    status FIX "$path" "hardlink -> AGENTS.md"
    add_local_exclude "$path"
}

run_pass() {
    local agent f
    for agent in $TARGETS; do
        load_agent "$agent"
        if [ "$VERIFY" = 1 ]; then
            echo ""
            echo "[+] Agent: $agent  ->  $AG_DIR"
        fi
        [ -z "$AG_WORKFLOWS" ] || sync_dir_link "$AG_DIR/$AG_WORKFLOWS" ".agents/workflows"
        [ -z "$AG_SKILLS" ]    || sync_dir_link "$AG_DIR/$AG_SKILLS"    ".agents/skills"
        [ -z "$AG_RULES" ]     || sync_dir_link "$AG_DIR/$AG_RULES"     ".agents/rules"
        for f in $AG_FILES; do sync_file_link "$f"; done
    done
}

# -------------------------------------------------------------------------------
# 4. Apply, then verify
# -------------------------------------------------------------------------------

if [ "$CHECK" = 0 ]; then
    echo ">>> Linking agent environment for: $TARGETS"
    run_pass
    [ "$CHANGES" -gt 0 ] || echo "  nothing to change"
    VERIFY=1
    PROBLEMS=0
    echo ""
fi
echo ">>> Verification"
run_pass

if [ "$PROBLEMS" -gt 0 ]; then
    echo ""
    echo ">>> $PROBLEMS problem(s) found."
    exit 1
fi
echo ""
echo ">>> Done."
