# The smoke test on Windows: two copies of the app on this computer do the main things a
# person does (app/integration_test/smoke_test.dart: profiles, friends, a direct chat both
# ways, a group, its voice room, the screen shared with the room's button through Windows'
# own capture and watched by the other copy, a file, leaving). The test's screen list is the
# app's own dialog, filled by the system as a person's click fills it.
#
#   powershell -File tools\smoke-windows.ps1 [-Out <folder>] [-NoBuild] [-Minutes <n>]
#
# Builds the test (`flutter build windows --debug -t integration_test/smoke_test.dart`)
# unless -NoBuild, then starts copy a and copy b with folders of their own under -Out
# (default: a new folder in %TEMP%), waits for both results (up to -Minutes, default 15),
# and stops them. Meant for a computer with no real microphone or speakers in use (a test
# machine with virtual devices only, such as VB-Cable): no microphone is opened, the camera
# stays off, and the app's sounds play to the default output.
# Exits 0 when both passed, 1 when one failed or crashed (a copy that ended by itself with
# an exception code, such as 0xC0000005, is named as a crash), and 3 when it cannot run here.
param(
  [string]$Out = '',
  [switch]$NoBuild,
  [int]$Minutes = 15
)
$ErrorActionPreference = 'Continue'
$repo = Split-Path -Parent $PSScriptRoot
if (-not $Out) { $Out = Join-Path $env:TEMP ('fogata-smoke-' + (Get-Date -Format yyyyMMdd-HHmmss)) }
New-Item -ItemType Directory -Force $Out | Out-Null
$meet = Join-Path $Out 'meet'
Remove-Item -Recurse -Force $meet -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $meet | Out-Null

if (-not $NoBuild) {
  if (-not (Get-Command flutter -ErrorAction SilentlyContinue)) {
    Write-Output 'smoke: flutter missing'
    exit 3
  }
  Write-Output 'smoke: building integration_test/smoke_test.dart'
  Push-Location (Join-Path $repo 'app')
  flutter build windows --debug -t integration_test/smoke_test.dart *> (Join-Path $Out 'build.log')
  $built = $LASTEXITCODE
  Pop-Location
  if ($built -ne 0) {
    Get-Content (Join-Path $Out 'build.log') -Tail 20
    exit 1
  }
}
$exe = Join-Path $repo 'app\build\windows\x64\runner\Debug\fogata.exe'
if (-not (Test-Path $exe)) {
  Write-Output "smoke: no build at $exe"
  exit 1
}

$copies = @{}
foreach ($role in 'a', 'b') {
  $data = Join-Path $Out $role
  New-Item -ItemType Directory -Force $data | Out-Null
  $env:FOGATA_ROLE = $role
  $env:FOGATA_SMOKE = $meet
  $env:FOGATA_DATA_DIR = $data
  $env:FOGATA_DOWNLOADS = $data
  $env:RUST_LOG = 'warn'
  $copies[$role] = Start-Process $exe -PassThru `
    -RedirectStandardOutput (Join-Path $Out "$role.out") `
    -RedirectStandardError (Join-Path $Out "$role.err")
  Write-Output "smoke: copy $role is $($copies[$role].Id)"
  Start-Sleep 2
}

# Until both wrote their result, or one ended without one (and the other waited a minute).
$end = (Get-Date).AddMinutes($Minutes)
$goneAt = $null
while ((Get-Date) -lt $end) {
  $results = @('a', 'b' | Where-Object { Test-Path (Join-Path $meet "$_-result.txt") })
  if ($results.Count -eq 2) { break }
  $gone = @('a', 'b' | Where-Object { $copies[$_].HasExited -and -not (Test-Path (Join-Path $meet "$_-result.txt")) })
  $failed = @('a', 'b' | Where-Object { (Get-Content (Join-Path $meet "$_-result.txt") -TotalCount 1 -ErrorAction SilentlyContinue) -like 'failed*' })
  if (($gone.Count -gt 0 -or $failed.Count -gt 0) -and -not $goneAt) { $goneAt = Get-Date }
  if ($goneAt -and ((Get-Date) - $goneAt).TotalSeconds -gt 60) { break }
  Start-Sleep 1
}

$ok = $true
foreach ($role in 'a', 'b') {
  $p = $copies[$role]
  $result = Join-Path $meet "$role-result.txt"
  $what = if (Test-Path $result) { (Get-Content $result -TotalCount 1) } else { '' }
  $last = Get-Content (Join-Path $meet "$role.log") -Tail 1 -ErrorAction SilentlyContinue
  if ($p.HasExited -and $what -ne 'passed' -and $p.ExitCode -ne 0) {
    Write-Output ("  copy $role CRASHED: exit code 0x{0:X8} after: $last" -f $p.ExitCode)
    $ok = $false
  } elseif ($what -ne 'passed') {
    Write-Output "  copy ${role}: $(if ($what) { $what } else { 'no result' }) after: $last"
    $ok = $false
  } else {
    Write-Output "  copy $role passed"
  }
}
foreach ($p in $copies.Values) {
  if (-not $p.HasExited) {
    Stop-Process -Force -Id $p.Id -ErrorAction SilentlyContinue
    $p.WaitForExit(10000) | Out-Null
  }
}
Write-Output "smoke: logs in $Out"
if ($ok) { Write-Output 'smoke: passed'; exit 0 }
Write-Output 'smoke: FAILED'
exit 1
