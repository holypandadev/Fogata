#!/usr/bin/env bash
# Builds libwebrtc for Linux x64 in a work folder made by fetch.sh, tests the parts Fogata
# added, and packs the archive the plugin unpacks into <work>/dist (with its SHA-256).
#
#     tools/webrtc/build-linux.sh <work> [--no-prepare]
#
# JOBS (default 8) is how many compilers run at once. --no-prepare keeps the tree as it is
# (to build again after changing it by hand); otherwise prepare.sh resets it first.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
. "$here/pins.sh"
. "$here/layout.sh"

work=${1:?usage: build-linux.sh <work> [--no-prepare]}
work=$(cd "$work" && pwd)
[[ ${2:-} == --no-prepare ]] || "$here/prepare.sh" "$work"

export DEPOT_TOOLS_UPDATE=0 DEPOT_TOOLS_METRICS=0 PATH="$work/depot_tools:$PATH"
src=$work/webrtc/src
out=$work/out/linux-x64
mkdir -p "$out"
same_layout "$out"
cat "$here/args/common.gn" "$here/args/linux-x64.gn" > "$out/args.gn"
(cd "$src" && gn gen "$out")
(cd "$src" && nice -n 10 ninja -C "$out" -j "${JOBS:-8}" libwebrtc libwebrtc_cpp_api_unittests)
LD_LIBRARY_PATH="$out" "$out/libwebrtc_cpp_api_unittests" \
    --gtest_filter='Vp8SizeGate.*:CustomAudioTransport.*:VoiceEngineMuted.*'
python3 -I "$here/package.py" --os linux --arch x64 --src "$src" --out "$out" \
    --into "$work/dist"
store=${FOGATA_LIBWEBRTC_DIR:-$HOME/fogata-libwebrtc}/$LIBWEBRTC_VERSION
mkdir -p "$store"
cp "$work/dist/$LIBWEBRTC_VERSION"/libwebrtc-linux-x64-release.zip* "$store/"
echo "copied into $store, where the app's build takes it from"
