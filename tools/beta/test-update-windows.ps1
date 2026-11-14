# The update's install step on Windows, with the published files and nothing else: installs
# an earlier pre-release as a person does (its zip from the releases page, checked against
# the signed list as the README says, then its own install.ps1), makes a profile in it by
# typing a name, lets that copy find the newest release on the releases page by itself and
# fetch it, starts it again as a person opens it the next time, when it swaps itself for the
# update, and checks what is left: the newer build in %LOCALAPPDATA%\Programs\Fogata,
# running, with no Fogata.new or Fogata.old beside it.
#
#   powershell -File tools\beta\test-update-windows.ps1 -From v0.1.0-beta.9 -To v0.1.0-beta.11
#       [-Out <folder>] [-Minutes <n>] [-Repo <owner/name>]
#
# -To is the release the earlier copy must end up as (the newest on the page: an earlier one
# is never taken). Meant for a computer of its own (GitHub's, a virtual machine) with a
# desktop nobody is typing at: it installs Fogata for this user, starts it and types into its
# window. It refuses to run where Fogata is installed or has data already; at the end it
# stops the copy it started and removes nothing. The copy is ended, not closed, before its
# second start: as when the computer is turned off.
# Exits 0 when the update installed, 1 when it did not, 3 when it cannot run here.
param(
  [Parameter(Mandatory = $true)][string]$From,
  [Parameter(Mandatory = $true)][string]$To,
  [string]$Out = '',
  [int]$Minutes = 20,
  [string]$Repo = 'holypandadev/Fogata'
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
if (-not $Out) { $Out = Join-Path $env:TEMP ('fogata-update-' + (Get-Date -Format yyyyMMdd-HHmmss)) }
New-Item -ItemType Directory -Force $Out | Out-Null
$app = Join-Path $env:LOCALAPPDATA 'Programs\Fogata'
$data = Join-Path $env:LOCALAPPDATA 'Fogata'
$new = Join-Path $env:LOCALAPPDATA 'Programs\Fogata.new'
$old = Join-Path $env:LOCALAPPDATA 'Programs\Fogata.old'
foreach ($path in $app, $data, $new, $old) {
  if (Test-Path -LiteralPath $path) {
    Write-Output "test-update-windows: $path exists; this test is for a computer without Fogata"
    exit 3
  }
}

function Say([string]$text) { Write-Output ("[{0}] {1}" -f (Get-Date -Format HH:mm:ss), $text) }

# The build a folder's programs were made as: the marker fogata-update compiles in, which
# tools/beta/release.py looks for too.
function Get-Build([string]$folder) {
  if (-not (Test-Path -LiteralPath $folder)) { return $null }
  $latin = [Text.Encoding]::GetEncoding(28591)
  foreach ($file in Get-ChildItem -LiteralPath $folder -Recurse -File -Include *.dll, *.exe) {
    try { $text = $latin.GetString([IO.File]::ReadAllBytes($file.FullName)) } catch { continue }
    $found = [regex]::Match($text, 'fogata-build-marker:(\d+);')
    if ($found.Success) { return [int]$found.Groups[1].Value }
  }
  return $null
}

# The copies of Fogata running from the installed folder.
function Get-Running {
  @(Get-Process -Name fogata -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.ToLower().StartsWith($app.ToLower()) })
}

$shots = 0
function Save-Screen {
  try {
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $bounds = [Windows.Forms.SystemInformation]::VirtualScreen
    $bitmap = New-Object Drawing.Bitmap $bounds.Width, $bounds.Height
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($bounds.Location, [Drawing.Point]::Empty, $bounds.Size)
    $script:shots++
    $bitmap.Save((Join-Path $Out ('screen-{0:d2}.png' -f $script:shots)), [Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose(); $bitmap.Dispose()
  } catch { Say "no picture of the screen: $_" }
}

function Get-File([string]$tag, [string]$name, [string]$dest) {
  $url = "https://github.com/$Repo/releases/download/$tag/$name"
  & curl.exe --silent --show-error --fail --location --retry 3 --output $dest $url
  if ($LASTEXITCODE -ne 0) { throw "cannot fetch $url" }
}

$passed = $false
try {
  # What the newest release says it is.
  $manifest = Join-Path $Out 'release.json'
  Get-File $To 'release.json' $manifest
  $release = Get-Content -Raw -LiteralPath $manifest | ConvertFrom-Json
  $wanted = [int]$release.build
  Say "$To is build $wanted ($($release.version)), which asks for build $($release.min_build) at least"

  # The earlier release, checked as its README says: the list of downloads signed by a
  # pre-release key of tools/allowed_signers, and the zip as that list says.
  $version = $From -replace '^v', ''
  $name = "Fogata-$version-windows-x64"
  $zip = Join-Path $Out "$name.zip"
  $sums = Join-Path $Out 'SHA256SUMS'
  Get-File $From "$name.zip" $zip
  Get-File $From 'SHA256SUMS' $sums
  Get-File $From 'SHA256SUMS.sig' "$sums.sig"
  $signers = Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'tools\allowed_signers'
  & cmd.exe /c "ssh-keygen -Y verify -f `"$signers`" -I fogata-prerelease -n file -s `"$sums.sig`" < `"$sums`""
  if ($LASTEXITCODE -ne 0) { throw "SHA256SUMS of $From is not signed by a pre-release key" }
  $hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
  if (-not (Select-String -LiteralPath $sums -SimpleMatch "$hash  $name.zip" -Quiet)) {
    throw "$name.zip is not the file SHA256SUMS lists"
  }
  # Unzipped beside the zip under its name, as Extract All does.
  Expand-Archive -LiteralPath $zip -DestinationPath (Join-Path $Out $name)
  $install = Get-ChildItem -LiteralPath (Join-Path $Out $name) -Recurse -File -Filter install.ps1 | Select-Object -First 1
  if (-not $install) { throw "no install.ps1 in $zip" }
  Say "installing $From with its own $($install.Name)"
  # It asks whether the checksum was compared: it was, above.
  'yes' | & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $install.FullName
  if ($LASTEXITCODE -ne 0) { throw "install.ps1 ended with $LASTEXITCODE" }
  $first = Get-Build $app
  Say "installed: build $first in $app"
  if ($null -eq $first -or $first -ge $wanted) { throw "the copy installed is build $first, not one before $wanted" }

  # A profile, as a person makes one: the name typed into the welcome page, then Enter.
  # A copy without one asks nobody for updates.
  $shell = New-Object -ComObject WScript.Shell
  Add-Type @'
using System;
using System.Runtime.InteropServices;
namespace FogataTest {
  public struct Rect { public int Left, Top, Right, Bottom; }
  public static class Win {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
  }
}
'@
  # (friends.bin holds it: the device's key alone is made as soon as the page shows.)
  $made = Join-Path $data 'friends.bin'
  $end = (Get-Date).AddMinutes(5)
  while (-not (Test-Path -LiteralPath $made)) {
    if ((Get-Date) -gt $end) { throw 'no profile was made within five minutes' }
    Start-Sleep -Seconds 5
    $window = Get-Running | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
    if (-not $window) { Say 'waiting for the window'; continue }
    Save-Screen
    if ($shell.AppActivate($window.Id)) {
      Start-Sleep -Milliseconds 500
      # A click on the name's field, where the welcome page draws it in its window: keys
      # reach the page only once something in the window was clicked.
      $rect = New-Object FogataTest.Rect
      [void][FogataTest.Win]::GetWindowRect($window.MainWindowHandle, [ref]$rect)
      $x = [int]($rect.Left + ($rect.Right - $rect.Left) * 0.62)
      $y = [int]($rect.Top + ($rect.Bottom - $rect.Top) * 0.59)
      [void][FogataTest.Win]::SetForegroundWindow($window.MainWindowHandle)
      [void][FogataTest.Win]::SetCursorPos($x, $y)
      [FogataTest.Win]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
      [FogataTest.Win]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
      Start-Sleep -Milliseconds 500
      $shell.SendKeys('^a')
      $shell.SendKeys('Tester')
      $shell.SendKeys('{ENTER}')
      Start-Sleep -Seconds 3
    } else { Say 'the window did not come to the front' }
  }
  Say 'the profile is made'
  Save-Screen

  # It finds the release by itself and fetches it.
  $pack = Join-Path $data 'updates\windows-x64.pack'
  $end = (Get-Date).AddMinutes($Minutes)
  $last = ''
  while (-not (Test-Path -LiteralPath $pack)) {
    if ((Get-Date) -gt $end) { throw "after $Minutes minutes the update was not fetched" }
    Start-Sleep -Seconds 5
    if ((Get-Running).Count -eq 0) { throw 'the copy ended by itself before the update came' }
    $part = Get-ChildItem -LiteralPath (Join-Path $data 'updates') -File -ErrorAction SilentlyContinue |
      ForEach-Object { "$($_.Name) $([math]::Floor($_.Length / 10MB) * 10) MB" }
    $state = "updates: $($part -join '; ')"
    if ($state -ne $last) { Say $state; $last = $state }
  }
  Say 'the update is fetched and checked'
  Start-Sleep -Seconds 5
  Save-Screen

  # The next start installs it.
  foreach ($process in Get-Running) { Stop-Process -Id $process.Id -Force }
  for ($i = 0; $i -lt 30 -and (Get-Running).Count -gt 0; $i++) { Start-Sleep -Seconds 1 }
  Say 'starting it again'
  Start-Process -FilePath (Join-Path $app 'fogata.exe')
  $end = (Get-Date).AddMinutes(10)
  $last = ''
  $took = $false
  while ((Get-Date) -lt $end) {
    Start-Sleep -Seconds 5
    $build = Get-Build $app
    $running = Get-Running
    $state = "build $build, $($running.Count) running, Fogata.new $(Test-Path -LiteralPath $new), Fogata.old $(Test-Path -LiteralPath $old)"
    if ($state -ne $last) { Say $state; $last = $state; Save-Screen }
    if ($build -eq $wanted -and $running.Count -gt 0 -and -not (Test-Path -LiteralPath $new)) { $took = $true; break }
  }
  if (-not $took) { throw "ten minutes after its second start the installed copy is build $(Get-Build $app), not $wanted" }

  # The new copy stays up, and the swap left nothing behind.
  Say "build $wanted is installed and running; watching it for a minute"
  Start-Sleep -Seconds 60
  Save-Screen
  $running = Get-Running
  if ($running.Count -eq 0) { throw "the new copy ended by itself within a minute" }
  for ($i = 0; $i -lt 30 -and (Test-Path -LiteralPath $old); $i++) { Start-Sleep -Seconds 1 }
  foreach ($path in $new, $old) {
    if (Test-Path -LiteralPath $path) { throw "$path was left behind" }
  }
  $note = Join-Path $data 'updates\installed.json'
  if (Test-Path -LiteralPath $note) { Say "installed.json: $((Get-Content -Raw -LiteralPath $note).Trim())" }
  $passed = $true
  Say "PASS: $From updated itself to $To (build $first to build $wanted)"
} catch {
  Say "FAIL: $_"
} finally {
  Save-Screen
  foreach ($process in Get-Running) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
  Start-Sleep -Seconds 2
  # What the copies wrote for problem reports (nothing personal reaches these files).
  $logs = Join-Path $data 'diagnostics'
  if (Test-Path -LiteralPath $logs) {
    Copy-Item -LiteralPath $logs -Destination (Join-Path $Out 'diagnostics') -Recurse -Force
    foreach ($log in Get-ChildItem -LiteralPath $logs -File) {
      Write-Output "--- $($log.Name) (its last 80 lines)"
      Get-Content -LiteralPath $log.FullName -Tail 80
    }
  }
  Get-ChildItem -LiteralPath (Join-Path $env:LOCALAPPDATA 'Programs') -ErrorAction SilentlyContinue |
    ForEach-Object { Write-Output "Programs\$($_.Name)" }
}
if ($passed) { exit 0 } else { exit 1 }
