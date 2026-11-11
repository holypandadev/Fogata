#!/usr/bin/env bash
# What a build of the application was made with, for the record of the job that built it
# (.github/workflows/ci.yml and release-build.yml write it to the run's summary, and the
# release builds keep it beside each zip, which tools/beta/ci_builds.py fetches with them):
#
#     tools/toolchain.sh <linux|macos|windows> [<build folder>]
#
# The runner's image (GitHub's ImageOS and ImageVersion), Flutter with its engine revision,
# Rust, CMake, and each system's own: clang and the versions of the development packages the
# build links (Linux); Xcode, its SDK, Swift and CocoaPods (macOS); Visual Studio, the MSVC
# toolset and the Windows SDK (Windows), the last two as the build in <build folder>
# (app/build/windows/x64 by default) used them. A tool that cannot be asked is named as
# missing: this is a note of what was there, it stops nothing.
set -uo pipefail
system=${1:?usage: tools/toolchain.sh <linux|macos|windows> [<build folder>]}
build=${2:-app/build/windows/x64}

# The first line of what the command says, or "missing".
first() { local out; out=$("$@" 2> /dev/null | head -n 1) && [[ -n $out ]] && echo "$out" || echo missing; }

echo "image: ${ImageOS:-unknown} ${ImageVersion:-unknown}"
machine=$(flutter --version --machine 2> /dev/null)
field() { printf '%s' "$machine" | sed -n "s/.*\"$1\": *\"\([^\"]*\)\".*/\1/p" | head -n 1; }
echo "flutter: $(field frameworkVersion) (framework $(field frameworkRevision), engine $(field engineRevision), dart $(field dartSdkVersion))"
echo "rustc: $(first rustc -V)"
echo "cmake: $(first cmake --version)"
case $system in
    linux)
        echo "clang: $(first clang --version)"
        for package in libgtk-3-dev libpipewire-0.3-dev libpulse-dev; do
            echo "$package: $(first dpkg-query -W -f '${Version}\n' "$package")"
        done
        ;;
    macos)
        echo "xcode: $(xcodebuild -version 2> /dev/null | tr '\n' ' ' | sed 's/ *$//' || true) ($(first xcode-select -p))"
        echo "macos sdk: $(first xcrun --sdk macosx --show-sdk-version)"
        echo "swift: $(first swiftc --version)"
        echo "cocoapods: $(first pod --version)"
        ;;
    windows)
        # Not a name bash can expand: read from the environment.
        programs=$(printenv 'ProgramFiles(x86)' || echo "C:/Program Files (x86)")
        vswhere="$programs/Microsoft Visual Studio/Installer/vswhere.exe"
        vs=$("$vswhere" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 \
            -property installationPath 2> /dev/null | tr -d '\r')
        echo "visual studio: $(first "$vswhere" -latest -products '*' -property catalog_productDisplayVersion | tr -d '\r') at ${vs:-missing}"
        echo "msvc default toolset: $(first cat "$vs/VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt" | tr -d '\r')"
        # What CMake found and wrote into the projects of this build.
        compiler=$(sed -n 's/^set(CMAKE_CXX_COMPILER_VERSION "\(.*\)")$/\1/p' "$build"/CMakeFiles/*/CMakeCXXCompiler.cmake 2> /dev/null | head -n 1)
        echo "msvc of the build: ${compiler:-missing}"
        project=$(ls "$build"/runner/*.vcxproj 2> /dev/null | head -n 1)
        # The value of an element of the runner's project, or "missing".
        tag() {
            local value=
            [[ -n $project ]] && value=$(sed -n "s/.*<$1>\(.*\)<\/$1>.*/\1/p" "$project" | head -n 1 | tr -d '\r')
            echo "${value:-missing}"
        }
        echo "platform toolset of the build: $(tag PlatformToolset)"
        echo "windows sdk of the build: $(tag WindowsTargetPlatformVersion)"
        ;;
    *)
        echo "tools/toolchain.sh: no system $system" >&2
        exit 2
        ;;
esac
