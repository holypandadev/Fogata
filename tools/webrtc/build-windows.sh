#!/usr/bin/env bash
# Builds libwebrtc for Windows x64 on Linux (cross-compiled with clang-cl), in a work folder
# made by fetch.sh, and packs the archive the plugin unpacks into <work>/dist (with its
# SHA-256). Needs the Visual Studio toolchain packed once on Windows by
# package-win-toolchain.ps1: see WINDOWS.md.
#
#     tools/webrtc/build-windows.sh <work> <folder with the toolchain's <hash>.zip> [--no-prepare]
#
# JOBS (default 8) is how many compilers run at once.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
. "$here/pins.sh"
. "$here/layout.sh"

work=${1:?usage: build-windows.sh <work> <toolchain folder> [--no-prepare]}
toolchain=${2:?usage: build-windows.sh <work> <toolchain folder> [--no-prepare]}
work=$(cd "$work" && pwd)
toolchain=$(cd "$toolchain" && pwd)
zips=("$toolchain"/*.zip)
[[ ${#zips[@]} -eq 1 && -f ${zips[0]} ]] ||
    { echo "$toolchain must hold exactly one toolchain archive (<hash>.zip)" >&2; exit 1; }
ours=$(basename "${zips[0]}" .zip)

# gclient's hook unpacks a toolchain from this folder, named by its hash, in place of
# Google's own (which only Google can download), and mounts it case-insensitively.
export DEPOT_TOOLS_WIN_TOOLCHAIN=1 DEPOT_TOOLS_WIN_TOOLCHAIN_BASE_URL=$toolchain
src=$work/webrtc/src
wanted=$(sed -n "s/^TOOLCHAIN_HASH = '\(.*\)'$/\1/p" "$src/build/vs_toolchain.py")
[[ -n $wanted ]] || { echo "no TOOLCHAIN_HASH in build/vs_toolchain.py" >&2; exit 1; }
export "GYP_MSVS_HASH_$wanted=$ours"

# The Windows dependencies and the toolchain (gclient sync with target_os win).
"$here/fetch.sh" "$work" linux win
[[ ${3:-} == --no-prepare ]] || "$here/prepare.sh" "$work"

export DEPOT_TOOLS_UPDATE=0 DEPOT_TOOLS_METRICS=0 PATH="$work/depot_tools:$PATH"
out=$work/out/win-x64
mkdir -p "$out"
same_layout "$out"
cat "$here/args/common.gn" "$here/args/win-x64.gn" > "$out/args.gn"
(cd "$src" && gn gen "$out")
(cd "$src" && nice -n 10 ninja -C "$out" -j "${JOBS:-8}" libwebrtc libwebrtc_cpp_api_unittests)
python3 -I "$here/package.py" --os win --arch x64 --src "$src" --out "$out" \
    --into "$work/dist"
store=${FOGATA_LIBWEBRTC_DIR:-$HOME/fogata-libwebrtc}/$LIBWEBRTC_VERSION
mkdir -p "$store"
cp "$work/dist/$LIBWEBRTC_VERSION"/libwebrtc-win-x64-release.zip* "$store/"
echo "copied into $store, where the app's build takes it from"
echo "Run the size check's tests on Windows: copy $out/libwebrtc_cpp_api_unittests.exe" \
    "and $out/libwebrtc.dll to one folder there and run" \
    "libwebrtc_cpp_api_unittests.exe --gtest_filter=Vp8SizeGate.*"
