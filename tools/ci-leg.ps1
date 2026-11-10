# The checks tools/ci.sh asks of Windows, run in a clone on that computer, at the commit being
# checked:
#
#     $env:FOGATA_CI_SHA = '<commit>'; powershell -ExecutionPolicy Bypass -File tools\ci-leg.ps1
#
# FOGATA_CI_WINDOWS, on the computer that runs tools/ci.sh, is the command that gets the commit
# there, checks it out and runs this, whose exit status and output come back (see
# tools/ci-leg.sh, the same for macOS). It refuses a clone not at that commit, or with any
# change to a tracked file or any new file git does not ignore. Then it runs what tools/check.sh
# runs on the other systems and can run here: formatting, clippy with warnings as errors and
# every test of the workspace (the command line program also with its lab feature, which the
# tests of the release commands need), the tests of the vendored crates, cargo deny (licences,
# bans, sources), THIRD-PARTY-NOTICES up to date, the tests of the release scripts, then
# `dart format` (the Dart code must be formatted already), `flutter analyze` and `flutter test`, and the smoke test (tools/smoke-windows.ps1: two copies
# through the main things a person does, the screen shared through Windows' own capture; for a
# machine with virtual sound devices only). Only when every step passed it prints the line
# tools/ci.sh looks for:
#
#     fogata-ci-leg: system=windows commit=<commit> status=pass steps=<steps> skipped=<steps>
#
# FOGATA_CI_LEG_SKIP names steps (fmt, clippy, test, vendor-tests, deny, notices,
# script-tests, analyze, flutter-test, smoke) to leave out, for what cannot run on that computer;
# they are named in the line, and tools/ci.sh then records the leg as incomplete.
#
# Every native command is checked by its exit status ($ErrorActionPreference does not cover
# them), and a command that cannot even start fails the step.
$ErrorActionPreference = 'Stop'
# The copies the checks start never read the releases page on GitHub.
$env:FOGATA_UPDATE_WEB = 'off'
$sha = $env:FOGATA_CI_SHA
if (-not $sha) {
  Write-Host 'FOGATA_CI_SHA names the commit to check'
  exit 2
}
Set-Location (Join-Path $PSScriptRoot '..')

# Runs the native command $exe with $arguments and stops at once when it cannot start or exits
# with anything but 0.
function Invoke-Native([string]$exe, [string[]]$arguments) {
  & $exe @arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$exe $($arguments -join ' ') exited with $LASTEXITCODE"
  }
}

# The output of the native command (one string, trimmed), stopping when it fails.
function Get-Native([string]$exe, [string[]]$arguments) {
  $text = & $exe @arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$exe $($arguments -join ' ') exited with $LASTEXITCODE"
  }
  if ($null -eq $text) { return '' }
  return (($text | Out-String).Trim())
}

try {
  $head = Get-Native 'git' @('rev-parse', '--verify', 'HEAD^{commit}')
  if ($head -ne $sha) {
    Write-Host "ci-leg: this clone is at $head, not at $sha"
    exit 1
  }
  # Every tracked file (deny.toml, THIRD-PARTY-NOTICES, docs, rust-toolchain.toml too) and any
  # new file that is not ignored: a failed `git status` is a failure, not an empty list.
  $changes = Get-Native 'git' @('status', '--porcelain', '--untracked-files=normal')
} catch {
  Write-Host "ci-leg: cannot tell what this clone holds: $_"
  exit 1
}
if ($changes) {
  Write-Host 'ci-leg: the clone has changes:'
  Write-Host $changes
  exit 1
}

# The Python that runs here: the launcher, then the usual names (the Store's stub of
# python3.exe exits with an error, so each one must run).
function Find-Python {
  foreach ($candidate in @(@('py', '-3'), @('python'), @('python3'))) {
    if (-not (Get-Command $candidate[0] -ErrorAction SilentlyContinue)) { continue }
    $extra = @($candidate | Select-Object -Skip 1)
    try {
      & $candidate[0] @extra -c 'import sys; sys.exit(0 if sys.version_info >= (3, 9) else 1)' 2>$null
      # The comma keeps the array whole: a one-element array would come out as a bare string.
      if ($LASTEXITCODE -eq 0) { return , $candidate }
    } catch { continue }
  }
  throw 'no Python 3.9 or newer (py -3, python, python3) on this computer'
}
function Invoke-Python([string[]]$arguments) {
  $found = Find-Python
  $extra = @($found | Select-Object -Skip 1)
  Invoke-Native $found[0] ($extra + @('-I', '-B') + $arguments)
}

$skip = @(($env:FOGATA_CI_LEG_SKIP -split ',') | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$steps = New-Object System.Collections.Generic.List[string]
$skipped = New-Object System.Collections.Generic.List[string]
# Runs step $name (a script block of Invoke-Native calls), unless FOGATA_CI_LEG_SKIP names it.
function Step([string]$name, [scriptblock]$run) {
  if ($skip -contains $name) {
    Write-Host "ci-leg: $name SKIPPED"
    $skipped.Add($name)
    return
  }
  Write-Host "ci-leg: $name"
  try {
    & $run
  } catch {
    Write-Host "ci-leg: $name FAILED ($_)"
    exit 1
  }
  $steps.Add($name)
}
function Join-Names($names) { if ($names.Count -eq 0) { 'none' } else { $names -join ',' } }
Step 'fmt' { Invoke-Native 'cargo' @('fmt', '--all', '--check') }
Step 'clippy' {
  Invoke-Native 'cargo' @('clippy', '--workspace', '--all-targets', '--', '-D', 'warnings')
  Invoke-Native 'cargo' @('clippy', '-p', 'fogata', '--all-targets', '--features', 'lab', '--', '-D', 'warnings')
}
Step 'test' {
  Invoke-Native 'cargo' @('test', '--workspace', '--quiet')
  Invoke-Native 'cargo' @('test', '-p', 'fogata', '--features', 'lab', '--quiet')
}
Step 'vendor-tests' {
  foreach ($crate in @('libp2p-mdns', 'libp2p-quic', 'turn', 'spake2')) {
    Invoke-Native 'cargo' @('test', '-p', $crate, '--lib', '--quiet')
  }
}
Step 'deny' { Invoke-Native 'cargo' @('deny', '--offline', 'check', '--hide-inclusion-graph', 'licenses', 'bans', 'sources') }
Step 'notices' { Invoke-Python @('tools/third_party_notices.py', '--check') }
# The tests of tools/ci.sh, check.sh and the hooks (tools/test_ci.py) run bash scripts and are
# for Linux and macOS; the release scripts' own tests and the commit marker's run everywhere.
Step 'script-tests' {
  Invoke-Python @('tools/beta/test_release.py')
  Invoke-Python @('tools/test_native_build.py')
}
Step 'analyze' {
  Push-Location app
  try {
    Invoke-Native 'flutter' @('pub', 'get', '--offline')
    Invoke-Native 'dart' @('format', '--output=none', '--set-exit-if-changed', 'lib', 'test',
      'integration_test', 'test_driver', 'packages/fogata_video')
    Invoke-Native 'flutter' @('analyze')
  } finally { Pop-Location }
}
Step 'flutter-test' {
  Push-Location app
  try { Invoke-Native 'flutter' @('test') } finally { Pop-Location }
}
Step 'smoke' {
  $out = Join-Path $env:TEMP ('fogata-ci-smoke-' + $sha.Substring(0, 12))
  Invoke-Native 'powershell' @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'tools\smoke-windows.ps1', '-Out', $out)
}
Write-Host "fogata-ci-leg: system=windows commit=$sha status=pass steps=$(Join-Names $steps) skipped=$(Join-Names $skipped)"
