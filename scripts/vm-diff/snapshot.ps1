# Captures a before/after snapshot of package list, PATH, install folders, Start Menu, and installer processes.
# Safe to run on any Windows machine — read-only.
# Usage: powershell -File scripts/vm-diff/snapshot.ps1 -OutFile before.json

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$OutFile
)

$ErrorActionPreference = "Continue"

function Get-PathValue([string]$Name, [string]$Hive) {
    try {
        if ($Hive -eq "User") {
            return [Environment]::GetEnvironmentVariable($Name, "User")
        }
        return [Environment]::GetEnvironmentVariable($Name, "Machine")
    } catch {
        return $null
    }
}

function Get-DirListing([string]$Path, [int]$Depth = 2) {
    if (-not (Test-Path -LiteralPath $Path)) { return @() }
    try {
        Get-ChildItem -LiteralPath $Path -Force -ErrorAction SilentlyContinue |
            Select-Object -First 500 |
            ForEach-Object {
                [PSCustomObject]@{
                    FullName = $_.FullName
                    Length   = if ($_.PSIsContainer) { $null } else { $_.Length }
                    Kind     = if ($_.PSIsContainer) { "dir" } else { "file" }
                }
            }
    } catch {
        return @()
    }
}

function Get-WingetList {
    try {
        $raw = & winget list --accept-source-agreements 2>$null | Out-String
        return ($raw -split "`r?`n" | Where-Object { $_.Trim() -ne "" })
    } catch {
        return @("winget unavailable: $_")
    }
}

function Get-InstallerProcesses {
    $names = @("winget", "msiexec", "SetupPal", "installer_gui")
    Get-Process -ErrorAction SilentlyContinue |
        Where-Object {
            $n = $_.ProcessName
            ($names -contains $n) -or ($n -like "unins*")
        } |
        ForEach-Object {
            [PSCustomObject]@{
                Id          = $_.Id
                ProcessName = $_.ProcessName
                Path        = $_.Path
                StartTime   = $_.StartTime
            }
        }
}

$startMenuUser = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$startMenuMachine = Join-Path ${env:ProgramData} "Microsoft\Windows\Start Menu\Programs"
$localPrograms = Join-Path $env:LOCALAPPDATA "Programs"
$venvs = Join-Path $env:USERPROFILE ".venvs"
$setuppalWork = Join-Path $env:USERPROFILE ".setuppal"

$snapshot = [ordered]@{
    capturedAt        = (Get-Date).ToString("o")
    computerName      = $env:COMPUTERNAME
    userName          = $env:USERNAME
    wingetList        = @(Get-WingetList)
    userPath          = Get-PathValue "Path" "User"
    machinePath       = Get-PathValue "Path" "Machine"
    localPrograms     = @(Get-DirListing $localPrograms)
    programFiles      = @(Get-DirListing ${env:ProgramFiles})
    programFilesX86   = @(Get-DirListing ${env:ProgramFiles(x86)})
    startMenuUser     = @(Get-DirListing $startMenuUser)
    startMenuMachine  = @(Get-DirListing $startMenuMachine)
    venvs             = @(Get-DirListing $venvs)
    setuppalWork      = @(Get-DirListing $setuppalWork)
    installerProcesses = @(Get-InstallerProcesses)
}

$dir = Split-Path -Parent $OutFile
if ($dir -and -not (Test-Path -LiteralPath $dir)) {
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
}

$snapshot | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $OutFile -Encoding utf8
Write-Host "Wrote snapshot to $OutFile"
