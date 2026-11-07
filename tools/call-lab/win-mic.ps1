# Windows: what a call does when the microphone cannot be used. With -Mode denied (the
# default) the microphone privacy switch is turned off for this user and for desktop
# applications (Settings › Privacy & security › Microphone); with -Mode noinput every
# capture endpoint is disabled, so the computer has no input at all (needs an
# administrator; checked first, nothing is changed without one). Starts two copies of the
# call test built in debug in this checkout (flutter build windows --debug -t
# integration_test/call_test.dart): copy a enters with the system's microphone on, copy b
# muted (FOGATA_TEST_MIC_CASE=denied: a must say it cannot use the microphone and show it
# off, and keep running). Waits for both and puts everything back as it was, whatever
# happened, then checks it is back. Prints each copy's result and what a logged.
# Usage (PowerShell, in the checkout): powershell -ep bypass -f tools\call-lab\win-mic.ps1 [-Mode noinput]
param([ValidateSet('denied', 'noinput')] [string] $Mode = 'denied')
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $root 'app\build\windows\x64\runner\Debug\fogata.exe'
if (-not (Test-Path $exe)) { "no build at $exe"; exit 1 }
$store = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone'
$keys = @($store, "$store\NonPackaged")
$before = @{}
$disabled = @()
if ($Mode -eq 'noinput') {
  $admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  "administrator: $admin"
  if (-not $admin) { 'disabling a device needs an administrator: nothing changed'; exit 2 }
  # Capture endpoints: the inputs the system lists ({0.0.1...} ids are capture devices).
  $inputs = @(Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue | Where-Object { $_.InstanceId -like 'SWD\MMDEVAPI\{0.0.1.*' -and $_.Status -eq 'OK' })
  "inputs before: $(($inputs | ForEach-Object { $_.FriendlyName }) -join ' | ')"
  if ($inputs.Count -eq 0) { 'no input to disable'; exit 3 }
} else {
  foreach ($k in $keys) {
    if (-not (Test-Path $k)) { New-Item -Force $k | Out-Null }
    $before[$k] = (Get-ItemProperty $k -ErrorAction SilentlyContinue).Value
  }
}
$run = "C:\fogata\mic-$Mode-" + (Get-Date -Format HHmmss)
New-Item -ItemType Directory -Force "$run\meet", "$run\a", "$run\b" | Out-Null
"run $run"
$copies = @()
try {
  if ($Mode -eq 'noinput') {
    foreach ($d in $inputs) {
      Disable-PnpDevice -InstanceId $d.InstanceId -Confirm:$false -ErrorAction Stop
      $disabled += $d
    }
    $left = @(Get-PnpDevice -Class AudioEndpoint | Where-Object { $_.InstanceId -like 'SWD\MMDEVAPI\{0.0.1.*' -and $_.Status -eq 'OK' })
    "inputs disabled: $($disabled.Count), still on: $($left.Count)"
    if ($left.Count -gt 0) { throw 'an input is still on' }
  } else {
    foreach ($k in $keys) { Set-ItemProperty $k -Name Value -Value 'Deny' }
    "microphone switch: Deny (was $($before[$store]), desktop apps $($before["$store\NonPackaged"]))"
  }
  foreach ($role in 'a', 'b') {
    $env:FOGATA_ROLE = $role
    $env:FOGATA_PEERS = '2'
    $env:FOGATA_CALL = "$run\meet"
    $env:FOGATA_DATA_DIR = "$run\$role"
    $env:FOGATA_DOWNLOADS = "$run\$role"
    $env:FOGATA_CALL_LOG = '1'
    $env:FOGATA_TEST_MIC_CASE = 'denied'
    $env:RUST_LOG = 'warn'
    if ($role -eq 'a') { $env:FOGATA_TEST_MIC = 'system' } else { Remove-Item Env:FOGATA_TEST_MIC -ErrorAction SilentlyContinue }
    $copies += Start-Process $exe -PassThru -RedirectStandardError "$run\$role.err" -RedirectStandardOutput "$run\$role.out"
  }
  $end = (Get-Date).AddMinutes(15)
  while ((Get-Date) -lt $end) {
    $done = @('a', 'b' | Where-Object { Test-Path "$run\meet\$_-result.txt" }).Count
    $alive = @($copies | Where-Object { -not $_.HasExited }).Count
    if ($done -eq 2 -or $alive -eq 0) { break }
    Start-Sleep 2
  }
  foreach ($c in $copies) { "copy $($c.Id) running: $(-not $c.HasExited)$(if ($c.HasExited) { ", exit code $($c.ExitCode)" })" }
} finally {
  foreach ($c in $copies) { if (-not $c.HasExited) { Stop-Process -Force -Id $c.Id; $c.WaitForExit(10000) | Out-Null } }
  if ($Mode -eq 'noinput') {
    foreach ($d in $inputs) { Enable-PnpDevice -InstanceId $d.InstanceId -Confirm:$false -ErrorAction Continue }
    $back = @(Get-PnpDevice -Class AudioEndpoint | Where-Object { $_.InstanceId -like 'SWD\MMDEVAPI\{0.0.1.*' -and $_.Status -eq 'OK' })
    "inputs back: $(($back | ForEach-Object { $_.FriendlyName }) -join ' | ') ($($back.Count) of $($inputs.Count))"
  } else {
    foreach ($k in $keys) {
      if ($null -eq $before[$k]) { Remove-ItemProperty $k -Name Value -ErrorAction SilentlyContinue }
      else { Set-ItemProperty $k -Name Value -Value $before[$k] }
    }
    "microphone switch put back: $((Get-ItemProperty $store -ErrorAction SilentlyContinue).Value)"
  }
}
foreach ($role in 'a', 'b') {
  "== $role result: $(Get-Content "$run\meet\$role-result.txt" -TotalCount 1 -ErrorAction SilentlyContinue)"
}
"== a's log"
Get-Content "$run\meet\a.log" -ErrorAction SilentlyContinue | Select-String 'denied microphone|connected to everyone|devices:' | ForEach-Object { $_.Line.Substring(0, [Math]::Min(400, $_.Line.Length)) }
"== a's errors"
Get-Content "$run\a.err", "$run\a.out" -ErrorAction SilentlyContinue | Select-String 'Fatal|Check failed|Exception|microphone|ADM|audio' | Select-Object -Last 15 | ForEach-Object { $_.Line }
