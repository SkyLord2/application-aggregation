$ErrorActionPreference = "Stop"

Write-Host "== XiaoHai Assistant Smoke Test ==" -ForegroundColor Cyan

$programData = $env:ProgramData
$base = Join-Path $programData "XiaoHaiAssistant"
$plugins = Join-Path $base "plugins"
$state = Join-Path $base "install-state.json"

Write-Host "[1/5] Check ProgramData layout..."
if (-not (Test-Path $base)) { throw "Missing: $base" }
if (-not (Test-Path $plugins)) { throw "Missing: $plugins" }
if (-not (Test-Path $state)) { throw "Missing: $state" }

Write-Host "[2/5] Parse install-state.json..."
$st = Get-Content $state -Raw | ConvertFrom-Json
if (-not $st.product_code) { throw "Missing product_code in state" }

Write-Host "[3/5] List plugins..."
$pluginFiles = Get-ChildItem $plugins -Filter "*.json" -ErrorAction SilentlyContinue
Write-Host ("Plugins: " + $pluginFiles.Count)

Write-Host "[4/5] Check autorun registry..."
$runKey = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"
$run = Get-ItemProperty -Path $runKey -ErrorAction SilentlyContinue
if (-not $run.XiaoHaiAssistant) { Write-Warning "Autorun value not found: XiaoHaiAssistant" }

Write-Host "[5/5] Done." -ForegroundColor Green
