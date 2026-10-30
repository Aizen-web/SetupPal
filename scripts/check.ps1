# Runs every SetupPal check in order and stops at the first failure.
$ErrorActionPreference = "Stop"

$root = (Resolve-Path "$PSScriptRoot\..").Path
Push-Location $root
try {
    $checks = @(
        @{ Name = "Rust tests"; Command = "cargo"; Args = @("test", "--workspace") },
        @{ Name = "Frontend build"; Command = "npm.cmd"; Args = @("--prefix", "crates/gui", "run", "build") },
        @{ Name = "Animation registry"; Command = "npm.cmd"; Args = @("--prefix", "crates/gui", "run", "check:animations") },
        @{ Name = "Key migration"; Command = "npm.cmd"; Args = @("--prefix", "crates/gui", "run", "check:keys") },
        @{ Name = "Chat formatter"; Command = "npm.cmd"; Args = @("--prefix", "crates/gui", "run", "check:formatter") },
        @{ Name = "Run log"; Command = "npm.cmd"; Args = @("--prefix", "crates/gui", "run", "check:run-log") },
        @{ Name = "Offline assets and CSP"; Command = "npm.cmd"; Args = @("--prefix", "crates/gui", "run", "check:offline") }
    )

    foreach ($check in $checks) {
        Write-Host "==> $($check.Name): $($check.Command) $($check.Args -join ' ')" -ForegroundColor Cyan
        & $check.Command @($check.Args)
        if ($LASTEXITCODE -ne 0) {
            Write-Host "FAILED: $($check.Name) (exit code $LASTEXITCODE)" -ForegroundColor Red
            exit $LASTEXITCODE
        }
    }

    Write-Host "All checks passed." -ForegroundColor Green
    exit 0
} finally {
    Pop-Location
}
