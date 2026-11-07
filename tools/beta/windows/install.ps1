# Installs Fogata for this Windows user, with no administrator: copies the files of the
# Fogata folder beside this script that Fogata.sha256 lists, each checked against its
# SHA-256 there, to %LOCALAPPDATA%\Programs\Fogata, where it later updates itself, adds it to
# the Start menu and opens it. Nothing else in the folder is copied. The data folder is not
# touched.
#
# Fogata.sha256 comes in the same zip as this script, so it shows the files arrived whole,
# not who made them: what shows that is the zip's SHA-256, which Holy Panda sends apart from
# the download. This script travels inside that zip too, so whoever changed the zip could have
# changed this script and the checksum it shows: that checksum only means something when the
# person installing worked out the zip's SHA-256 on their own BEFORE unzipping it. Before
# installing, this shows its checksum, says so, and asks.
$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot 'Fogata'
$list = Join-Path $PSScriptRoot 'Fogata.sha256'
$programs = Join-Path $env:LOCALAPPDATA 'Programs'
$target = Join-Path $programs 'Fogata'
function Stop-Install([string]$why) {
  Write-Host $why
  Read-Host 'Press Enter to close'
  exit 1
}
if (-not (Test-Path -LiteralPath (Join-Path $source 'fogata.exe')) -or
    -not (Test-Path -LiteralPath $list)) {
  Stop-Install 'The Fogata folder or Fogata.sha256 is not next to this script. Unzip the whole download first.'
}
# The files to install: relative paths with / between plain names, nothing that climbs out,
# names a drive or a stream, each with its SHA-256.
$files = [ordered]@{}
foreach ($line in Get-Content -LiteralPath $list -Encoding UTF8) {
  if ($line -notmatch '^([0-9a-f]{64})  ([^\\:*?"<>|]+)$') {
    Stop-Install "Fogata.sha256 holds a line that is not a file: $line"
  }
  $hash = $Matches[1]
  $relative = $Matches[2]
  $parts = $relative -split '/'
  if ($relative.StartsWith('/') -or
      @($parts | Where-Object { $_ -eq '' -or $_ -eq '.' -or $_ -eq '..' -or $_ -match '[\x00-\x1f]' }).Count -gt 0) {
    Stop-Install "Fogata.sha256 names a file outside the Fogata folder: $relative"
  }
  if ($files.Contains($relative)) {
    Stop-Install "Fogata.sha256 names $relative twice"
  }
  $files[$relative] = $hash
}
if (-not $files.Contains('fogata.exe')) {
  Stop-Install 'Fogata.sha256 does not list fogata.exe'
}
# The zip this folder was unzipped from: Extract All names the folder after it, beside it.
$zip = "$PSScriptRoot.zip"
Write-Host ''
if (Test-Path -LiteralPath $zip -PathType Leaf) {
  $zipHash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
  Write-Host "SHA-256 of $(Split-Path -Leaf $zip):"
  Write-Host "  $zipHash"
} else {
  Write-Host "The zip this folder came from is not next to it, so its SHA-256 cannot be shown here."
  Write-Host 'In PowerShell, Get-FileHash <the zip you downloaded> shows it.'
}
Write-Host ''
Write-Host 'IMPORTANT: this script came inside the zip. Whoever changed the zip could have changed'
Write-Host 'this script too, and with it the checksum above. So do not trust it. Before you unzipped,'
Write-Host 'you should have worked out the zip''s SHA-256 yourself (in PowerShell: Get-FileHash <the zip'
Write-Host 'you downloaded>) and compared it with the checksum Holy Panda sent you apart from the'
Write-Host 'download. If you did not, type no now, and do that first.'
$answer = Read-Host 'Did you work it out yourself before unzipping, and does it match what Holy Panda sent? Type yes to install'
if ($answer.Trim() -ne 'yes') {
  Stop-Install 'Nothing was installed. If the checksum differs, do not open these files: tell Holy Panda.'
}
$running = Get-Process -Name 'fogata' -ErrorAction SilentlyContinue |
  Where-Object { $_.Path -and $_.Path.StartsWith($target, [StringComparison]::OrdinalIgnoreCase) }
if ($running) {
  Write-Host 'Fogata is running. Quit it from its tray icon, then run this again.'
  Read-Host 'Press Enter to close'
  exit 1
}
New-Item -ItemType Directory -Force -Path $programs | Out-Null
# Fogata puts its own updates in place with the same folders, under this lock (30 minutes
# old, it is one an update left behind).
$lock = Join-Path $programs 'Fogata.update.lock'
$lockFile = Get-Item -LiteralPath $lock -ErrorAction SilentlyContinue
if ($lockFile -and $lockFile.LastWriteTime -gt (Get-Date).AddMinutes(-30)) {
  Stop-Install 'Fogata is putting an update in place. Wait a minute, then run this again.'
}
$staging = Join-Path $programs 'Fogata.new'
$old = Join-Path $programs 'Fogata.old'
# A copy an earlier install could not put back is put back first; then what an earlier
# install or update left behind goes, before anything is copied.
if ((Test-Path -LiteralPath $old) -and -not (Test-Path -LiteralPath $target)) {
  try {
    Rename-Item -LiteralPath $old -NewName 'Fogata'
  } catch {
    Stop-Install "The copy installed before is in $old and cannot be put back ($($_.Exception.Message)). Close what may hold its files and run this again."
  }
}
foreach ($leftover in @($staging, $old)) {
  if (Test-Path -LiteralPath $leftover) {
    try {
      Remove-Item -LiteralPath $leftover -Recurse -Force
    } catch {
      Stop-Install "$leftover is left from an earlier install and cannot be removed ($($_.Exception.Message)). Close what may hold its files, or delete that folder, and run this again."
    }
  }
}
New-Item -ItemType Directory -Path $staging | Out-Null
foreach ($relative in $files.Keys) {
  $windowsPath = $relative.Replace('/', '\')
  $from = Join-Path $source $windowsPath
  $to = Join-Path $staging $windowsPath
  if (-not (Test-Path -LiteralPath $from -PathType Leaf)) {
    Remove-Item -LiteralPath $staging -Recurse -Force
    Stop-Install "$relative is missing from the Fogata folder. Unzip the whole download again."
  }
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $to) | Out-Null
  Copy-Item -LiteralPath $from -Destination $to
  # The copy is what is checked, and what is installed.
  $hash = (Get-FileHash -LiteralPath $to -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($hash -ne $files[$relative]) {
    Remove-Item -LiteralPath $staging -Recurse -Force
    Stop-Install "$relative is not the file of this release. Download Fogata again."
  }
  # The download carried the mark of the web; this file is the release's, checked above,
  # and Fogata's updates arrive signed over Fogata itself.
  Unblock-File -LiteralPath $to
}
# The installed copy goes aside first and is removed only once the new one is in its place;
# if the new one cannot take its place (a file held open, an antivirus), it comes back.
$had = Test-Path -LiteralPath $target
if ($had) {
  try {
    Rename-Item -LiteralPath $target -NewName 'Fogata.old'
  } catch {
    Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
    Stop-Install "The copy installed before cannot be moved ($($_.Exception.Message)); it is kept. Close what may hold its files and run this again."
  }
}
try {
  Rename-Item -LiteralPath $staging -NewName 'Fogata'
} catch {
  $why = $_.Exception.Message
  Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
  if (-not $had) {
    Stop-Install "Fogata could not be put in place ($why). Close what may hold its files and run this again."
  }
  try {
    Rename-Item -LiteralPath $old -NewName 'Fogata'
  } catch {
    Stop-Install "Fogata could not be put in place ($why), and the copy installed before could not be put back ($($_.Exception.Message)): it is in $old. Rename that folder to Fogata, or run this again."
  }
  Stop-Install "Fogata could not be put in place ($why); the copy installed before is kept. Close what may hold its files and run this again."
}
if ($had) {
  try {
    Remove-Item -LiteralPath $old -Recurse -Force
  } catch {
    Write-Host "The copy installed before is in $old; it can be deleted."
  }
}
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Fogata.lnk'
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($shortcut)
$link.TargetPath = Join-Path $target 'fogata.exe'
$link.WorkingDirectory = $target
$link.Save()
Start-Process -FilePath (Join-Path $target 'fogata.exe')
Write-Host 'Fogata is installed and in the Start menu.'
