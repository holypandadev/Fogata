#!/usr/bin/env bash
# Builds WebRTC.framework for macOS (arm64 and x86_64 in one) on a Mac, from a work folder
# made by `fetch.sh <work> mac`, with patches/webrtc and patches/apple, and packs
# WebRTC.xcframework (with WebRTC's licence and NOTICE.md) into
# <work>/dist/<version>/WebRTC.xcframework.zip, printing its SHA-256. See MACOS.md.
#
#     tools/webrtc/build-macos.sh <work> [--no-prepare]
#
# JOBS (default 8) is how many compilers run at once.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
. "$here/pins.sh"
. "$here/layout.sh"

[[ $(uname -s) == Darwin ]] || { echo "build-macos.sh runs on a Mac" >&2; exit 1; }
work=${1:?usage: build-macos.sh <work> [--no-prepare]}
work=$(cd "$work" && pwd)
[[ ${2:-} == --no-prepare ]] || "$here/prepare.sh" "$work" "$here/patches/apple"

export DEPOT_TOOLS_UPDATE=0 DEPOT_TOOLS_METRICS=0 PATH="$work/depot_tools:$PATH"
src=$work/webrtc/src
for cpu in arm64 x64; do
    out=$work/out/mac-$cpu
    mkdir -p "$out"
    same_layout "$out"
    { cat "$here/args/common.gn" "$here/args/mac.gn"; echo "target_cpu = \"$cpu\""; } \
        > "$out/args.gn"
    (cd "$src" && gn gen "$out")
    # The bundle is put together by copying into it: files an earlier build of this folder
    # put there and this one no longer makes (the headers of codecs since left out) would
    # stay and go into the archive. ninja removes what it made before and no longer makes.
    (cd "$src" && ninja -C "$out" -t cleandead)
    (cd "$src" && nice -n 10 ninja -C "$out" -j "${JOBS:-8}" mac_framework_bundle)
done

joined=$work/out/mac-universal
rm -rf "${joined:?}"
mkdir -p "$joined"
cp -R "$work/out/mac-arm64/WebRTC.framework" "$joined/WebRTC.framework"
# The library itself, not the link to it a versioned framework has at its top.
binary=WebRTC
[[ -L $joined/WebRTC.framework/WebRTC ]] && binary=$(readlink "$joined/WebRTC.framework/WebRTC")
lipo -create -output "$joined/WebRTC.framework/$binary" \
    "$work/out/mac-arm64/WebRTC.framework/$binary" \
    "$work/out/mac-x64/WebRTC.framework/$binary"
lipo -info "$joined/WebRTC.framework/$binary"
# Opus and VP8 are the only codecs (patches/apple/0004), and there is no frame cryptor.
"$here/macos-codecs.sh" "$joined/WebRTC.framework/$binary"
xcodebuild -create-xcframework -framework "$joined/WebRTC.framework" \
    -output "$joined/WebRTC.xcframework"

# WebRTC's licence and the licences of every library built into the framework.
cp "$src/LICENSE" "$joined/WebRTC.xcframework/LICENSE"
notice=$(mktemp -d)
python3 -B "$src/tools_webrtc/libs/generate_licenses.py" --target //sdk:mac_framework_bundle \
    "$notice" "$work/out/mac-arm64"
python3 -I -c 'import html, sys; print(html.unescape(open(sys.argv[1]).read()), end="")' \
    "$notice/LICENSE.md" > "$joined/WebRTC.xcframework/NOTICE.md"
rm -rf "${notice:?}"

dist=$work/dist/$LIBWEBRTC_VERSION
mkdir -p "$dist"
rm -f "$dist/WebRTC.xcframework.zip"
# -y keeps the framework's links as links.
(cd "$joined" && find WebRTC.xcframework -exec touch -h -t 202601010000 {} + &&
    zip -q -X -r -y "$dist/WebRTC.xcframework.zip" WebRTC.xcframework)
(cd "$dist" && shasum -a 256 WebRTC.xcframework.zip | tee WebRTC.xcframework.zip.sha256)
store=${FOGATA_LIBWEBRTC_DIR:-$HOME/fogata-libwebrtc}/$LIBWEBRTC_VERSION
mkdir -p "$store"
cp "$dist"/WebRTC.xcframework.zip* "$store/"
echo "copied into $store, where the app's pod install takes it from"
