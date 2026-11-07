# Builds and runs probe_test.cpp with Visual Studio's build tools (x64), against
# video_probe.cpp, with the integration tests' clip:
#   run_tests.ps1 [-Clip <clip.mp4>] [-Only silent|stalls|six|all]
param([string]$Clip, [string]$Only)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$plugin = Split-Path -Parent $here
if (-not $Clip) {
  $Clip = Join-Path $plugin '..\..\..\integration_test\clip.mp4'
}
$out = Join-Path $env:TEMP 'fogata-probe-test'
Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $out | Out-Null
$vcvars = Get-ChildItem "${env:ProgramFiles(x86)}\Microsoft Visual Studio\*\BuildTools\VC\Auxiliary\Build\vcvars64.bat" |
  Sort-Object FullName -Descending | Select-Object -First 1
if (-not $vcvars) { 'no Visual Studio build tools'; exit 1 }
$sources = "`"$here\probe_test.cpp`" `"$plugin\video_probe.cpp`""
$flags = '/nologo /std:c++17 /EHsc /W4 /WX /wd4100 /O2 /Zi /DNOMINMAX'
$libs = 'mfplat.lib mfreadwrite.lib mfuuid.lib windowscodecs.lib ole32.lib oleaut32.lib ws2_32.lib'
cmd /c "`"$($vcvars.FullName)`" >nul && cl $flags $sources /Fo`"$out\\`" /Fe`"$out\probe_test.exe`" /link /DEBUG $libs" | Select-Object -Last 5
if ($LASTEXITCODE -ne 0) { 'build failed'; exit 1 }
& "$out\probe_test.exe" (Resolve-Path $Clip).Path $out $Only
"probe_test exit $LASTEXITCODE"
exit $LASTEXITCODE
