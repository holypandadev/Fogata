# libwebrtc for Windows (x64)

Windows' libwebrtc is cross-compiled on Linux, as Chromium's own Windows builds can be: the
Linux computer runs clang-cl with Microsoft's headers and libraries, taken from a Visual
Studio installed on Windows and packed once into an archive. A build in Windows itself would
need about 30 GB free and, on 4 cores, several hours; on Linux it takes about as long as the
Linux build.

## 1. Pack the toolchain on Windows (once per Visual Studio or SDK version)

The packager of the pinned depot_tools takes only **Visual Studio 2026** and the Windows SDK
the WebRTC branch names (`SDK_VERSION` in `build/vs_toolchain.py` of the WebRTC tree:
**10.0.28000.0** for branch-head 8078). On the Windows computer:

1. Free at least **20 GB** on C: (Visual Studio 2026 Build Tools with C++ about 8 GB, the SDK
   about 3 GB, the archive and its temporary copy about 5 GB). On a VM with less room on
   C:, stop it, grow its disk by 30 GB, start it, and extend C: in Disk Management (Windows
   grows the partition into the new space).
2. Install Visual Studio 2026 (Build Tools are enough) with: Desktop development with C++,
   C++ ATL for the latest build tools (x86 & x64), and MSVC build tools for ARM64 with its
   ATL (the packager looks for the ARM64 libraries too). From the installer's command line:

       vs_BuildTools.exe --add Microsoft.VisualStudio.Workload.VCTools ^
         --add Microsoft.VisualStudio.Component.VC.ATL ^
         --add Microsoft.VisualStudio.Component.VC.Tools.ARM64 ^
         --add Microsoft.VisualStudio.Component.VC.ATL.ARM64 --includeRecommended --passive

3. Install the Windows SDK 10.0.28000.0 with **Debugging Tools for Windows** (in Apps,
   modify the SDK and tick them, or install the SDK on its own with that feature).
4. With git on the PATH, in PowerShell:

       powershell -ep bypass -f <repository>\tools\webrtc\package-win-toolchain.ps1 `
         -Out <folder>\webrtc-win-toolchain

   (the repository's `tools\webrtc` folder can be anywhere, a VM's share of the Linux
   computer too; `-Out` is where the archive goes, for instance that share). It fetches the pinned
   depot_tools into `C:\webrtc-tools`, lets it download its own Python, and runs
   depot_tools' `win_toolchain\package_from_installed.py 2026 -w 10.0.28000.0` (a copy that
   also finds the Build Tools, and lets an older Visual Studio stay installed), which writes
   `<hash>.zip`. `C:\webrtc-tools` can be deleted afterwards.

The archive holds Microsoft's compiler, headers and libraries: keep it on the build
computer, never publish it.

## 2. Build on Linux

    tools/webrtc/build-windows.sh ~/webrtc-work ~/webrtc-win-toolchain

It runs `fetch.sh <work> linux win` (gclient then fetches the Windows dependencies, unpacks
the toolchain from that folder into `depot_tools/win_toolchain/vs_files/<hash>` and mounts
it case-insensitively with ciopfs, which needs FUSE and libfuse 2: on Ubuntu the package
`libfuse2t64`, or its two libraries unpacked anywhere and named with `LD_LIBRARY_PATH`), applies the patches, builds with
`args/common.gn` and `args/win-x64.gn`, and writes

    <work>/dist/fogata-libwebrtc.m156.8078.6/libwebrtc-win-x64-release.zip   (+ .sha256)

with `win-x64-release/{lib/libwebrtc.dll, lib/libwebrtc.dll.lib, include, LICENSE,
NOTICE.md}`, as webrtc-sdk's Windows archives are laid out. Put its SHA-256 into
`app/third_party/flutter_webrtc/third_party/libwebrtc_version.ini` as
`sha256_win-x64 = ...`.

## 3. Check it on Windows

1. The wrapper's tests: copy `<work>/out/win-x64/libwebrtc_cpp_api_unittests.exe` and
   `libwebrtc.dll` into one folder on Windows and run
   `libwebrtc_cpp_api_unittests.exe --gtest_filter=Vp8SizeGate.*:CustomAudioTransport.*:VoiceEngineMuted.*:EchoRestart.*:PortableVector.*:SharedEncoder*:SharedEncodeSlots.*`
   (all of them pass).
2. Build the app with that archive: copy it from the Linux store
   (`~/fogata-libwebrtc/<version>/`) into the same folder under `%USERPROFILE%` on Windows,
   then `flutter build windows`.
3. `windows/test/run_tests.ps1` of the plugin, and a call with the rigs (`win-call.sh`):
   sound both ways, camera and screen, the microphone kept when devices change (libwebrtc's
   own device change event, `OnDeviceChange`, comes from patch 0003 on Windows).
