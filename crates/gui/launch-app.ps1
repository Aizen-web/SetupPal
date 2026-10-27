# SetupPal: Native Desktop App Launcher
$ErrorActionPreference = "SilentlyContinue"

# Check if Zero-Trust Backend server is running on 3001
$backendActive = Test-NetConnection -ComputerName 127.0.0.1 -Port 3001 -InformationLevel Quiet
if (-not $backendActive) {
    Write-Host "Starting SetupPal Zero-Trust Backend (cargo run -p installer_gui)..." -ForegroundColor Magenta
    $rootPath = (Resolve-Path "$PSScriptRoot\..\..").Path
    Start-Process -FilePath "cmd.exe" -ArgumentList "/c cargo run -p installer_gui" -WorkingDirectory $rootPath -WindowStyle Hidden
    
    $attempts = 0
    while (-not $backendActive -and $attempts -lt 12) {
        Start-Sleep -Seconds 1
        $backendActive = Test-NetConnection -ComputerName 127.0.0.1 -Port 3001 -InformationLevel Quiet
        $attempts++
    }
}

# Check if Vite server is running on 5173
$portActive = Test-NetConnection -ComputerName 127.0.0.1 -Port 5173 -InformationLevel Quiet
if (-not $portActive) {
    Write-Host "Starting SetupPal GUI dev server in background..." -ForegroundColor Cyan
    Start-Process -FilePath "cmd.exe" -ArgumentList "/c npx vite --port 5173" -WorkingDirectory $PSScriptRoot -WindowStyle Hidden
    
    $attempts = 0
    while (-not $portActive -and $attempts -lt 10) {
        Start-Sleep -Seconds 1
        $portActive = Test-NetConnection -ComputerName 127.0.0.1 -Port 5173 -InformationLevel Quiet
        $attempts++
    }
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
