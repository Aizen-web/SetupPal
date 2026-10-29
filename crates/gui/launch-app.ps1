# SetupPal: High-Performance Native Desktop App Launcher
$ErrorActionPreference = "SilentlyContinue"

# Fast TCP port probe using .NET Sockets (replaces slow Test-NetConnection)
function Test-PortFast($port, $timeoutMs = 120) {
    $tcp = New-Object System.Net.Sockets.TcpClient
    try {
        $iar = $tcp.BeginConnect('127.0.0.1', $port, $null, $null)
        if ($iar.AsyncWaitHandle.WaitOne($timeoutMs)) {
            $tcp.EndConnect($iar)
            return $true
        }
        return $false
    } catch {
        return $false
    } finally {
        $tcp.Close()
    }
}

function Test-HttpHealthFast($url, $timeoutMs = 400) {
    try {
        $req = [System.Net.HttpWebRequest]::Create($url)
        $req.Timeout = $timeoutMs
        $req.Method = "GET"
        $resp = $req.GetResponse()
        $status = [int]$resp.StatusCode
        $resp.Close()
        return ($status -ge 200 -and $status -lt 300)
    } catch {
        return $false
    }
}

$rootPath = (Resolve-Path "$PSScriptRoot\..\..").Path
$releaseExe = "$rootPath\target\release\installer_gui.exe"
$debugExe = "$rootPath\target\debug\installer_gui.exe"
$viteBin = "$PSScriptRoot\node_modules\.bin\vite.cmd"

# Initial fast probe (sub-10ms)
$backendActive = Test-PortFast 3001 80
$frontendActive = Test-PortFast 5173 80

# 1. Start Zero-Trust Backend (port 3001) in parallel
if (-not $backendActive) {
    Write-Host "Starting SetupPal Zero-Trust Backend (port 3001)..." -ForegroundColor Magenta
    if (Test-Path $releaseExe) {
        Start-Process -FilePath $releaseExe -WorkingDirectory $rootPath -WindowStyle Hidden
    } elseif (Test-Path $debugExe) {
        Start-Process -FilePath $debugExe -WorkingDirectory $rootPath -WindowStyle Hidden
    } else {
        Start-Process -FilePath "cmd.exe" -ArgumentList "/c cargo run -p installer_gui" -WorkingDirectory $rootPath -WindowStyle Hidden
    }
} else {
    Write-Host "Backend already active on port 3001" -ForegroundColor Green
}

# 2. Start Vite dev server (port 5173) in parallel
if (-not $frontendActive) {
    Write-Host "Starting SetupPal GUI dev server (port 5173)..." -ForegroundColor Cyan
    if (Test-Path $viteBin) {
        Start-Process -FilePath $viteBin -ArgumentList "--port 5173" -WorkingDirectory $PSScriptRoot -WindowStyle Hidden
    } else {
        Start-Process -FilePath "cmd.exe" -ArgumentList "/c npx vite --port 5173" -WorkingDirectory $PSScriptRoot -WindowStyle Hidden
    }
} else {
    Write-Host "GUI dev server already active on port 5173" -ForegroundColor Green
}

# 3. Concurrent rapid polling loop (100ms intervals instead of 1s sleeps)
if (-not $backendActive -or -not $frontendActive) {
    Write-Host "Synchronizing services..." -ForegroundColor DarkGray
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $maxWaitMs = 15000

    while ((-not $backendActive -or -not $frontendActive) -and ($sw.ElapsedMilliseconds -lt $maxWaitMs)) {
        Start-Sleep -Milliseconds 100
        if (-not $backendActive) {
            $backendActive = Test-PortFast 3001 50
        }
        if (-not $frontendActive) {
            $frontendActive = Test-PortFast 5173 50
        }
    }
}

# 4. Guarantee backend HTTP is ready before launching the window
if ($backendActive) {
    $healthy = Test-HttpHealthFast "http://127.0.0.1:3001/api/health" 350
    $hAttempts = 0
    while (-not $healthy -and $hAttempts -lt 10) {
        Start-Sleep -Milliseconds 100
        $healthy = Test-HttpHealthFast "http://127.0.0.1:3001/api/health" 350
        $hAttempts++
    }
}

# 5. Find native browser/webview binary and launch
$edgePath = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
if (-not (Test-Path $edgePath)) {
    $edgePath = "C:\Program Files\Microsoft\Edge\Application\msedge.exe"
}
if (-not (Test-Path $edgePath)) {
    $edgePath = "$env:LOCALAPPDATA\Microsoft\Edge\Application\msedge.exe"
}

if (Test-Path $edgePath) {
    Write-Host "Opening SetupPal Native Desktop Window (1100x720)..." -ForegroundColor Green
    $userDataDir = "$env:TEMP\setuppal_native_app_profile"
    Start-Process -FilePath $edgePath -ArgumentList "--app=http://localhost:5173", "--window-size=1100,720", "--user-data-dir=`"$userDataDir`""
} else {
    Start-Process "http://localhost:5173"
}
