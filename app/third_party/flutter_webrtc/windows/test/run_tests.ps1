# Builds and runs loopback_test.cc with Visual Studio's build tools (x64), against the
# libwebrtc a Windows build of the app unpacks into third_party\libwebrtc. Whoever runs it
# starts the programs that play (into a virtual output) and the window it shares:
#   run_tests.ps1 -Title <window title> -Flag <file its program waits for> [-Root <plugin>]
#                 [-Other <process id of the other program, stopped at the end>]
param([string]$Title, [string]$Flag, [string]$Root, [string]$Other = '')
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = if ($Root) { $Root } else { Split-Path -Parent (Split-Path -Parent $here) }
$webrtc = "$root\third_party\libwebrtc"
$out = Join-Path $env:TEMP "fogata-loopback-test"
New-Item -ItemType Directory -Force $out | Out-Null
$vcvars = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat'
$sources = "`"$here\loopback_test.cc`" `"$root\windows\application_loopback_capturer.cc`" `"$root\windows\loopback_capturer_factory.cc`""
$flags = "/nologo /std:c++17 /EHsc /O2 /Zi /DRTC_DESKTOP_DEVICE /DLIB_WEBRTC_API_DLL /I`"$root\windows`" /I`"$root\common\cpp\include`" /I`"$webrtc\include`""
$libs = "`"$webrtc\lib\libwebrtc.dll.lib`" avrt.lib ksuser.lib mmdevapi.lib ole32.lib runtimeobject.lib uuid.lib winmm.lib user32.lib"
cmd /c "`"$vcvars`" >nul && cl $flags $sources /Fo`"$out\\`" /Fe`"$out\loopback_test.exe`" /link /DEBUG $libs" | Select-Object -Last 5
if ($LASTEXITCODE -ne 0) { "build failed"; exit 1 }
Copy-Item -Force "$webrtc\lib\libwebrtc.dll" $out
& "$out\loopback_test.exe" $Title $Flag $Other
exit $LASTEXITCODE
