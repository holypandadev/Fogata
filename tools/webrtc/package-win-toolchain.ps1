# Packs the Visual Studio toolchain installed on this Windows computer into the archive the
# Linux cross build of libwebrtc takes (tools/webrtc/build-windows.sh), with depot_tools'
# own packager, at the commit tools/webrtc/pins.sh names. See WINDOWS.md for what must be
# installed first.
#
#     powershell -ep bypass -f package-win-toolchain.ps1 -Out <folder> [-Work C:\webrtc-tools]
#
# The archive, <hash>.zip (1.5 to 2.5 GB), is written into -Out. -Work holds depot_tools
# (with the Python it downloads for itself) and the packager's temporary files.
param(
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Work = "C:\webrtc-tools"
)
$ErrorActionPreference = "Stop"

$pins = Get-Content (Join-Path $PSScriptRoot "pins.sh")
function Pin($name) {
    $line = $pins | Where-Object { $_ -match "^$name=" } | Select-Object -First 1
    if (-not $line) { throw "pins.sh names no $name" }
    return $line.Substring($name.Length + 1)
}
$url = Pin "DEPOT_TOOLS_URL"
$commit = Pin "DEPOT_TOOLS_COMMIT"

# The Windows SDK the WebRTC branch is built with (build/vs_toolchain.py's SDK_VERSION).
$sdk = "10.0.28000.0"
$kits = "${env:ProgramFiles(x86)}\Windows Kits\10"
if (-not (Test-Path "$kits\Include\$sdk\um")) {
    throw "Windows SDK $sdk is not installed (see WINDOWS.md)"
}
if (-not (Test-Path "$kits\Debuggers\x64\cdb.exe")) {
    throw "the SDK's Debugging Tools for Windows are not installed (see WINDOWS.md)"
}

New-Item -ItemType Directory -Force $Work | Out-Null
$tools = Join-Path $Work "depot_tools"
if (-not (Test-Path "$tools\.git")) {
    git init -q $tools
}
git -C $tools fetch -q --depth=1 $url $commit
if ($LASTEXITCODE -ne 0) { throw "could not fetch depot_tools $commit" }
git -C $tools -c advice.detachedHead=false checkout -q -f --detach $commit
if ($LASTEXITCODE -ne 0) { throw "could not check out depot_tools $commit" }

$env:DEPOT_TOOLS_UPDATE = "0"
$env:DEPOT_TOOLS_METRICS = "0"
$env:DEPOT_TOOLS_WIN_TOOLCHAIN = "0"
$env:PATH = "$tools;$env:PATH"
# Fetches depot_tools' own Python and writes python3.bat, as its update would (which
# DEPOT_TOOLS_UPDATE=0 skips, keeping it at the pinned commit).
cmd /c "$tools\cipd_bin_setup.bat"
cmd /c "$tools\bootstrap\win_tools.bat"
if ($LASTEXITCODE -ne 0 -or -not (Test-Path "$tools\python3.bat")) {
    throw "depot_tools did not bootstrap its Python"
}

$temp = Join-Path $Work "package"
Remove-Item -Recurse -Force $temp -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $temp | Out-Null
Push-Location $temp
try {
    # The packager asks vswhere for full Visual Studio products only; a copy of it beside
    # the original also finds the Build Tools. Another Visual Studio (say 2022, for Rust)
    # may stay installed: the packager takes the 2026 one.
    $packager = "$tools\win_toolchain\package_from_build_tools.py"
    (Get-Content -Raw "$tools\win_toolchain\package_from_installed.py").Replace(
        'vswhere.exe -prerelease"', 'vswhere.exe -prerelease -products *"') |
        Set-Content -NoNewline -Encoding utf8 $packager
    cmd /c "$tools\python3.bat" $packager 2026 -w $sdk --allow_multiple_vs_installs
    if ($LASTEXITCODE -ne 0) { throw "the packager failed" }
    $zip = Get-ChildItem -Filter *.zip | Select-Object -First 1
    if (-not $zip) { throw "the packager made no archive" }
    New-Item -ItemType Directory -Force $Out | Out-Null
    Copy-Item $zip.FullName $Out
    Write-Output "wrote $(Join-Path $Out $zip.Name)"
} finally {
    Pop-Location
}
Remove-Item -Recurse -Force $temp
