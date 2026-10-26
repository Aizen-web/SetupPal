# SetupPal: Native Desktop App Launcher
$ErrorActionPreference = "SilentlyContinue"

# Check if Vite server is running on 5173
$portActive = Test-NetConnection -ComputerName 127.0.0.1 -Port 5173 -InformationLevel Quiet
if (-not $portActive) {
    Write-Host "Starting SetupPal GUI dev server in background..." -ForegroundColor Cyan
    Start-Process -FilePath "cmd.exe" -ArgumentList "/c npx vite --port 5173" -WorkingDirectory $PSScriptRoot -WindowStyle Hidden
    Start-Sleep -Seconds 2
}

# Find native browser/webview binary
$edgePath = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
if (-not (Test-Path $edgePath)) {
    $edgePath = "C:\Program Files\Microsoft\Edge\Application\msedge.exe"
}

if (Test-Path $edgePath) {
    Write-Host "Opening SetupPal Native Desktop Window (1100x720)..." -ForegroundColor Green
    $userDataDir = "$env:TEMP\setuppal_native_app_profile"
    Start-Process -FilePath $edgePath -ArgumentList "--app=http://localhost:5173", "--window-size=1100,720", "--user-data-dir=`"$userDataDir`""
} else {
    Start-Process "http://localhost:5173"
}
