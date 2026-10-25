# ===============================================================================
# AGENTS INIT (WINDOWS)
# Universal agent environment initializer.
#
# Links agent-specific config directories to the canonical .agents/ directory
# with junctions (no administrator rights or Developer Mode needed) and
# instruction files to AGENTS.md with hardlinks. The links are local: each one
# is added to .git/info/exclude unless git already tracks or ignores it, and the
# git index is never touched. An existing real directory or file is replaced by
# a link only when it is identical to the canonical content.
#
# Usage:
#   setup_windows.ps1                     # default: claude
#   setup_windows.ps1 -Agents qwen
#   setup_windows.ps1 -Agents claude,qwen # or: setup_windows.ps1 claude qwen
#   setup_windows.ps1 -Agents all
#   setup_windows.ps1 -Check              # verify only, change nothing (exit 1 on problems)
#   setup_windows.ps1 -Force              # replace copies that differ from the canonical content
# PowerShell binds --check and --force to the switches as well.
# ===============================================================================

param(
    [string]$Agents = "",
    [switch]$Check,
    [switch]$Force,
    # Without this, PowerShell drops extra arguments silently: a second agent
    # name or a mistyped option would leave a run that looks fine.
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest = @()
)

$ErrorActionPreference = 'Stop'

function Exit-WithError([string]$message) {
    [Console]::Error.WriteLine("ERROR: $message")
    exit 1
}

# -------------------------------------------------------------------------------
# 1. Repository root and agent registry (agents.json)
# -------------------------------------------------------------------------------

$RepoRoot     = (Get-Item -LiteralPath $PSScriptRoot).Parent.Parent.Parent.Parent.FullName
$RegistryFile = Join-Path $PSScriptRoot "agents.json"

if (-not (Test-Path -LiteralPath (Join-Path $RepoRoot ".agents") -PathType Container)) {
    Exit-WithError "Not a repository root (no .agents directory): $RepoRoot. Run the copy of this script that lives in <repo>/.agents/skills/agents-init/scripts."
}
if (-not (Test-Path -LiteralPath $RegistryFile -PathType Leaf)) {
    Exit-WithError "Registry not found: $RegistryFile"
}

# Every path below is resolved against the repository root, not the caller's cwd.
Set-Location -LiteralPath $RepoRoot
[System.Environment]::CurrentDirectory = $RepoRoot

try {
    $json = Get-Content -LiteralPath $RegistryFile -Raw | ConvertFrom-Json
} catch {
    Exit-WithError "agents.json is not valid JSON: $($_.Exception.Message)"
}

# A registry path must stay inside the repository.
function Test-RelPath([string]$p) {
    if (-not $p) { return $false }
    if ([System.IO.Path]::IsPathRooted($p) -or $p.Contains(':')) { return $false }
    foreach ($segment in ($p -split '[\\/]')) { if ($segment -eq '..') { return $false } }
    return $true
}

$REGISTRY = @{}
foreach ($prop in $json.PSObject.Properties) {
    $cfg = @{
        dir       = $prop.Value.dir
        workflows = $prop.Value.workflows
        skills    = $prop.Value.skills
        rules     = $prop.Value.rules
        files     = if ($prop.Value.files) { @($prop.Value.files) } else { @() }
    }
    $paths = @($cfg.dir) + @($cfg.workflows, $cfg.skills, $cfg.rules | Where-Object { $_ }) + $cfg.files
    foreach ($p in $paths) {
        if (-not (Test-RelPath $p)) { Exit-WithError "agents.json: agent '$($prop.Name)' has an invalid path '$p' (must be relative, inside the repository)." }
    }
    $REGISTRY[$prop.Name] = $cfg
}

$ALL_AGENTS = @($REGISTRY.Keys | Sort-Object)

# -------------------------------------------------------------------------------
# 2. Parse target agents (GNU-style --check / --force are accepted as well)
# -------------------------------------------------------------------------------

$names = @()
foreach ($tok in (@($Agents) + @($Rest) | ForEach-Object { $_ -split "[,\s]+" } | Where-Object { $_ -ne "" })) {
    switch ($tok) {
        '--check' { $Check = $true }
        '--force' { $Force = $true }
        'all'     { $names += $ALL_AGENTS }
        default {
            if ($tok.StartsWith('-')) { Exit-WithError "Unknown option '$tok'. Options: -Agents <list>, -Check, -Force (or --check, --force)." }
            if (-not $REGISTRY.ContainsKey($tok)) { Exit-WithError "Unknown agent '$tok'. Supported: $($ALL_AGENTS -join ', ')" }
            $names += $tok
        }
    }
}
if ($names.Count -eq 0) { $names = @('claude') }
$targets = @($names | Select-Object -Unique)

# -------------------------------------------------------------------------------
# 3. Helpers
# -------------------------------------------------------------------------------

$script:Problems = 0
$script:Changes  = 0
$script:Verify   = [bool]$Check

function Get-Full([string]$rel) { Join-Path $RepoRoot $rel }

# The apply pass reports only what it changed; the verification pass reports every path.
function Write-Status([string]$kind, [string]$path, [string]$note = "", [string[]]$details = @()) {
    if (-not $script:Verify -and $kind -ne 'FIX') { return }
    $color = switch ($kind) {
        'OK'    { 'Green' }
        'FIX'   { 'Cyan' }
        'WARN'  { 'Yellow' }
        default { 'Red' }
    }
    $line = "  [{0}] {1}" -f $kind, $path
    if ($note) { $line += "  ($note)" }
    Write-Host $line -ForegroundColor $color
    foreach ($d in @($details | Select-Object -First 5)) { Write-Host "         $d" -ForegroundColor $color }
    if ($details.Count -gt 5) { Write-Host "         ... and $($details.Count - 5) more" -ForegroundColor $color }
    if ($kind -eq 'FIX') { $script:Changes++ }
    if ($kind -eq 'FAIL') { $script:Problems++ }
}

# Native stderr must not turn into a terminating error under 'Stop' (Windows PowerShell 5.1).
function Invoke-Native([string]$exe, [string[]]$arguments) {
    $saved = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $out = & $exe @arguments 2>$null
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $saved
    }
    [pscustomobject]@{ Code = $code; Out = @($out) }
}

$script:ExcludeFile = $null
$script:GitPrefix   = ''
if (Get-Command git -ErrorAction SilentlyContinue) {
    $r = Invoke-Native 'git' @('-C', $RepoRoot, 'rev-parse', '--git-path', 'info/exclude')
    if ($r.Code -eq 0 -and $r.Out.Count -gt 0 -and $r.Out[0]) {
        $p = [string]$r.Out[0]
        if (-not [System.IO.Path]::IsPathRooted($p)) { $p = Join-Path $RepoRoot $p }
        $script:ExcludeFile = [System.IO.Path]::GetFullPath($p)
        # Exclude patterns are relative to the top of the work tree, which is not
        # the project root when the project is a subdirectory of a larger repository.
        $r = Invoke-Native 'git' @('-C', $RepoRoot, 'rev-parse', '--show-prefix')
        if ($r.Code -eq 0 -and $r.Out.Count -gt 0) { $script:GitPrefix = [string]$r.Out[0] }
    }
}

# Keeps a local link out of `git status` without touching the committed .gitignore.
function Add-LocalExclude([string]$rel) {
    if ($script:Verify -or -not $script:ExcludeFile) { return }
    $gitPath = $rel -replace '\\', '/'
    if ((Invoke-Native 'git' @('-C', $RepoRoot, 'ls-files', '--error-unmatch', '--', $gitPath)).Code -eq 0) { return }
    if ((Invoke-Native 'git' @('-C', $RepoRoot, 'check-ignore', '-q', '--', $gitPath)).Code -eq 0) { return }
    $dir = Split-Path -Parent $script:ExcludeFile
    if (-not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $text = ""
    if (Test-Path -LiteralPath $script:ExcludeFile) {
        $existing = [System.IO.File]::ReadAllText($script:ExcludeFile)
        if ($existing.Length -gt 0 -and -not $existing.EndsWith("`n")) { $text = "`n" }
    }
    # UTF-8 without BOM, LF line endings.
    [System.IO.File]::AppendAllText($script:ExcludeFile, "$text/$($script:GitPrefix)$gitPath`n")
    Write-Status 'FIX' $rel "git-ignored locally in .git/info/exclude"
}

function Test-IsReparse($item) { [bool]($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) }

# Hashes through .NET rather than Get-FileHash: that cmdlet is a module function
# in Windows PowerShell, and a PSModulePath inherited from PowerShell 7 (a parent
# process started from pwsh) keeps the module from loading.
function Get-Sha256([string]$path) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($path)
    try { [System.BitConverter]::ToString($sha.ComputeHash($stream)) }
    finally { $stream.Dispose(); $sha.Dispose() }
}

# Absolute target of a junction or symlink; a relative symlink is resolved from its own directory.
function Get-LinkTarget($item) {
    if ($item.LinkType -notin @('Junction', 'SymbolicLink')) { return $null }
    $t = @($item.Target)[0]
    if (-not $t) { return $null }
    if (-not [System.IO.Path]::IsPathRooted($t)) { $t = Join-Path (Split-Path -Parent $item.FullName) $t }
    [System.IO.Path]::GetFullPath($t).TrimEnd('\')
}

function Get-DirState([string]$path, [string]$target) {
    $item = Get-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
    if (-not $item) { return @{ Kind = 'missing' } }
    if (Test-IsReparse $item) {
        $t = Get-LinkTarget $item
        if (-not $t) { return @{ Kind = 'reparse' } }
        if ($t -ine [System.IO.Path]::GetFullPath($target).TrimEnd('\')) { return @{ Kind = 'other-link'; Target = $t } }
        if (-not (Test-Path -LiteralPath $t -PathType Container)) { return @{ Kind = 'broken' } }
        return @{ Kind = 'link' }
    }
    if (-not $item.PSIsContainer) { return @{ Kind = 'file' } }
    return @{ Kind = 'copy' }
}

# Entries of a real directory that are absent from, or differ from, the canonical one.
# Entries that exist only in the canonical directory are fine (git-ignored extras).
# A link inside the copy is never entered: it is fine only if it points at its
# canonical counterpart.
function Get-CopyDrift([string]$copy, [string]$canon, [string]$prefix = "") {
    foreach ($child in Get-ChildItem -LiteralPath $copy -Force) {
        $rel   = if ($prefix) { "$prefix\$($child.Name)" } else { $child.Name }
        $other = Join-Path $canon $child.Name
        if (Test-IsReparse $child) {
            $t = Get-LinkTarget $child
            if (-not $t -or $t -ine [System.IO.Path]::GetFullPath($other).TrimEnd('\')) {
                if ($t) { "link in copy: $rel -> $t" } else { "reparse point in copy: $rel" }
            }
        } elseif ($child.PSIsContainer) {
            Get-CopyDrift $child.FullName $other $rel
        } elseif (-not (Test-Path -LiteralPath $other -PathType Leaf)) {
            "only in copy: $rel"
        } elseif ((Get-Sha256 $child.FullName) -ne (Get-Sha256 $other)) {
            "differs: $rel"
        }
    }
}

# Deletes a junction or symlink itself; what it points to is never entered.
function Remove-Link([string]$path) {
    $item = Get-Item -LiteralPath $path -Force
    if ($item.PSIsContainer) { [System.IO.Directory]::Delete($path) }
    else                     { [System.IO.File]::Delete($path) }
}

# Deletes a real directory. Links inside it are removed as links, never followed.
function Remove-Tree([string]$path) {
    foreach ($child in Get-ChildItem -LiteralPath $path -Force) {
        if (Test-IsReparse $child)   { Remove-Link $child.FullName }
        elseif ($child.PSIsContainer) { Remove-Tree $child.FullName }
        else {
            $child.Attributes = [System.IO.FileAttributes]::Normal
            [System.IO.File]::Delete($child.FullName)
        }
    }
    [System.IO.Directory]::Delete($path)
}

function Sync-DirLink([string]$rel, [string]$canonRel) {
    $path   = Get-Full $rel
    $target = Get-Full $canonRel

    if (-not (Test-Path -LiteralPath $target -PathType Container)) {
        if ($script:Verify) {
            if ((Get-DirState $path $target).Kind -eq 'missing') { Write-Status 'OK' $rel "nothing to link, $canonRel does not exist" }
            else { Write-Status 'FAIL' $rel "canonical $canonRel is missing" }
            return
        }
        # Git does not keep empty directories, so a fresh clone may lack them.
        New-Item -ItemType Directory -Path $target -Force | Out-Null
    }

    $state = Get-DirState $path $target
    switch ($state.Kind) {
        'link' {
            Write-Status 'OK' $rel "junction -> $canonRel"
            Add-LocalExclude $rel
            return
        }
        'file'    { Write-Status 'FAIL' $rel "exists and is not a directory"; return }
        'reparse' { Write-Status 'FAIL' $rel "unsupported reparse point (not a junction or symlink); left alone"; return }
        'copy' {
            $drift = @(Get-CopyDrift $path $target)
            if ($drift.Count -gt 0) {
                if ($script:Verify -or -not $Force) {
                    $note = "copy differs from $canonRel"
                    if (-not $Force) { $note += "; review it, then re-run with -Force to replace it" }
                    Write-Status 'FAIL' $rel $note $drift
                    return
                }
            } elseif ($script:Verify) {
                Write-Status 'WARN' $rel "identical copy of $canonRel, not a link; run without -Check to link it"
                return
            }
            Remove-Tree $path
        }
        default {
            if ($script:Verify) {
                $note = switch ($state.Kind) {
                    'missing'    { "missing; run without -Check to create it" }
                    'broken'     { "broken link" }
                    'other-link' { "link to $($state.Target)" }
                }
                Write-Status 'FAIL' $rel $note
                return
            }
            if ($state.Kind -ne 'missing') { Remove-Link $path }
        }
    }

    $parent = Split-Path -Parent $path
    if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    New-Item -ItemType Junction -Path $path -Target $target | Out-Null
    Write-Status 'FIX' $rel "junction -> $canonRel"
    Add-LocalExclude $rel
}

# The file ID is printed as hex, so the comparison does not depend on the console language or code page.
function Get-FileId([string]$path) {
    $r = Invoke-Native 'fsutil' @('file', 'queryfileid', $path)
    if ($r.Code -eq 0 -and (($r.Out -join ' ') -match '0x[0-9a-fA-F]+')) { return $Matches[0] }
    return $null
}

function Test-SameFile([string]$a, [string]$b) {
    $idA = Get-FileId $a
    return [bool]($idA -and $idA -eq (Get-FileId $b))
}

function Sync-FileLink([string]$rel) {
    $path = Get-Full $rel
    $src  = Get-Full "AGENTS.md"

    if (-not (Test-Path -LiteralPath $src -PathType Leaf)) { Write-Status 'FAIL' $rel "AGENTS.md is missing"; return }

    $item = Get-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
    if ($item -and $item.PSIsContainer) { Write-Status 'FAIL' $rel "exists and is not a regular file"; return }
    if ($item) {
        if (Test-SameFile $path $src) {
            Write-Status 'OK' $rel "hardlink -> AGENTS.md"
            Add-LocalExclude $rel
            return
        }
        $same = (Get-Sha256 $path) -eq (Get-Sha256 $src)
        if (-not $same) {
            if ($script:Verify -or -not $Force) {
                # -Force overwrites this file with AGENTS.md, so say which side was edited last.
                $age  = if ($item.LastWriteTimeUtc -gt (Get-Item -LiteralPath $src).LastWriteTimeUtc) { 'newer' } else { 'older' }
                $note = "differs from AGENTS.md and is $age"
                if (-not $Force) { $note += "; move any changes into AGENTS.md, then re-run with -Force to link it" }
                Write-Status 'FAIL' $rel $note
                return
            }
        } elseif ($script:Verify) {
            Write-Status 'WARN' $rel "identical copy of AGENTS.md, not a hardlink; run without -Check to link it"
            return
        }
        $item.Attributes = [System.IO.FileAttributes]::Normal
        [System.IO.File]::Delete($path)
    } elseif ($script:Verify) {
        Write-Status 'FAIL' $rel "missing; run without -Check to create it"
        return
    }

    $parent = Split-Path -Parent $path
    if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    New-Item -ItemType HardLink -Path $path -Target $src | Out-Null
    Write-Status 'FIX' $rel "hardlink -> AGENTS.md"
    Add-LocalExclude $rel
}

function Invoke-Pass {
    foreach ($agent in $targets) {
        $cfg = $REGISTRY[$agent]
        $dir = $cfg.dir
        if ($script:Verify) { Write-Host "`n[+] Agent: $agent  ->  $dir" -ForegroundColor Magenta }

        if ($cfg.workflows) { Sync-DirLink "$dir\$($cfg.workflows)" ".agents\workflows" }
        if ($cfg.skills)    { Sync-DirLink "$dir\$($cfg.skills)"    ".agents\skills" }
        if ($cfg.rules)     { Sync-DirLink "$dir\$($cfg.rules)"     ".agents\rules" }
        foreach ($f in $cfg.files) { Sync-FileLink ($f -replace '/', '\') }
    }
}

# -------------------------------------------------------------------------------
# 4. Apply, then verify
# -------------------------------------------------------------------------------

try {
    if (-not $Check) {
        Write-Host ">>> Linking agent environment for: $($targets -join ', ')" -ForegroundColor Cyan
        Invoke-Pass
        if ($script:Changes -eq 0) { Write-Host "  nothing to change" }
        $script:Verify = $true
        $script:Problems = 0
        Write-Host ""
    }
    Write-Host ">>> Verification" -ForegroundColor Cyan
    Invoke-Pass
} catch {
    Exit-WithError $_.Exception.Message
}

if ($script:Problems -gt 0) {
    Write-Host "`n>>> $($script:Problems) problem(s) found." -ForegroundColor Red
    exit 1
}
Write-Host "`n>>> Done." -ForegroundColor Green
exit 0
