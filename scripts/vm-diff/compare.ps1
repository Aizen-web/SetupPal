# Diffs two snapshots from snapshot.ps1. Exit 0 when empty; exit 1 when differences remain.
# Usage: powershell -File scripts/vm-diff/compare.ps1 -Before before.json -After after.json

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Before,
    [Parameter(Mandatory = $true)][string]$After,
    [switch]$AllowProcessDiff
)

$ErrorActionPreference = "Stop"

function Load-Snap([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { throw "Missing snapshot: $Path" }
    return Get-Content -LiteralPath $Path -Raw -Encoding utf8 | ConvertFrom-Json
}

function Diff-StringLists($label, $a, $b) {
    $setA = [System.Collections.Generic.HashSet[string]]::new([string[]]@($a | ForEach-Object { "$_" }))
    $setB = [System.Collections.Generic.HashSet[string]]::new([string[]]@($b | ForEach-Object { "$_" }))
    $added = @($setB | Where-Object { -not $setA.Contains($_) })
    $removed = @($setA | Where-Object { -not $setB.Contains($_) })
    if ($added.Count -or $removed.Count) {
        return [PSCustomObject]@{ Label = $label; Added = $added; Removed = $removed }
    }
    return $null
}

function Diff-Paths($label, $a, $b) {
    $namesA = @($a | ForEach-Object { $_.FullName })
    $namesB = @($b | ForEach-Object { $_.FullName })
    return Diff-StringLists $label $namesA $namesB
}

function Split-PathEntries([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return @() }
    return @($Value -split ';' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}

$before = Load-Snap $Before
$after = Load-Snap $After
$diffs = @()

$diffs += @(Diff-StringLists "wingetList" $before.wingetList $after.wingetList | Where-Object { $_ })
$diffs += @(Diff-StringLists "userPath" (Split-PathEntries $before.userPath) (Split-PathEntries $after.userPath) | Where-Object { $_ })
$diffs += @(Diff-StringLists "machinePath" (Split-PathEntries $before.machinePath) (Split-PathEntries $after.machinePath) | Where-Object { $_ })
$diffs += @(Diff-Paths "localPrograms" $before.localPrograms $after.localPrograms | Where-Object { $_ })
$diffs += @(Diff-Paths "programFiles" $before.programFiles $after.programFiles | Where-Object { $_ })
$diffs += @(Diff-Paths "programFilesX86" $before.programFilesX86 $after.programFilesX86 | Where-Object { $_ })
$diffs += @(Diff-Paths "startMenuUser" $before.startMenuUser $after.startMenuUser | Where-Object { $_ })
$diffs += @(Diff-Paths "startMenuMachine" $before.startMenuMachine $after.startMenuMachine | Where-Object { $_ })
$diffs += @(Diff-Paths "venvs" $before.venvs $after.venvs | Where-Object { $_ })
$diffs += @(Diff-Paths "setuppalWork" $before.setuppalWork $after.setuppalWork | Where-Object { $_ })

if (-not $AllowProcessDiff) {
    $procA = @($before.installerProcesses | ForEach-Object { "$($_.ProcessName):$($_.Id)" })
    $procB = @($after.installerProcesses | ForEach-Object { "$($_.ProcessName):$($_.Id)" })
    # After a clean Rewind, orphan winget/msiexec/unins must be gone.
    $orphans = @($after.installerProcesses | Where-Object {
        $_.ProcessName -match '^(winget|msiexec)$' -or $_.ProcessName -like 'unins*'
    })
    if ($orphans.Count) {
        $diffs += [PSCustomObject]@{
            Label   = "installerProcesses"
            Added   = @($orphans | ForEach-Object { "$($_.ProcessName) pid=$($_.Id)" })
            Removed = @()
        }
    }
}

if ($diffs.Count -eq 0) {
    Write-Host "Snapshots match (no leftover diffs)." -ForegroundColor Green
    exit 0
}

Write-Host "Differences found:" -ForegroundColor Yellow
foreach ($d in $diffs) {
    Write-Host "=== $($d.Label) ==="
    if ($d.Added.Count) {
        Write-Host "  Added:"
        $d.Added | ForEach-Object { Write-Host "    + $_" }
    }
    if ($d.Removed.Count) {
        Write-Host "  Removed:"
        $d.Removed | ForEach-Object { Write-Host "    - $_" }
    }
}
exit 1
