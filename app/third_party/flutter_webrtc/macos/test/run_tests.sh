#!/usr/bin/env bash
# Builds and runs, against the WebRTC framework the app is built with
# (app/macos/WebRTC-SDK.podspec, from this computer's store):
# - frame_convert_test.m with FogataFrameConvert.m, as a release build compiles the plugin
#   (-Os); with --measure it also times both conversions;
# - video_renderer_test.m with FlutterRTCVideoRenderer.m and the FlutterMacOS framework of
#   the Flutter on PATH (no engine is started, no window opened), with ThreadSanitizer.
#   run_tests.sh [--measure]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../../../.." && pwd)
sources=$here/../flutter_webrtc/Sources/flutter_webrtc
version=$(sed -n "s/^  s.version = '\(.*\)'$/\1/p" "$repo/app/macos/WebRTC-SDK.podspec")
store=${FOGATA_LIBWEBRTC_DIR:-$HOME/fogata-libwebrtc}
archive=${FOGATA_WEBRTC_XCFRAMEWORK:-$store/fogata-libwebrtc.m$version/WebRTC.xcframework.zip}
if [[ ! -f $archive ]]; then
    echo "frame_convert_test: skipped (no $archive here)"
    exit 0
fi
work=$(mktemp -d)
trap 'rm -rf "${work:?}"' EXIT
unzip -q "$archive" -d "$work"
frameworks=$work/WebRTC.xcframework/macos-arm64_x86_64
clang -fobjc-arc -Os -Wall -Werror -mmacosx-version-min=12.0 -F "$frameworks" \
    -I "$sources" "$sources/FogataFrameConvert.m" "$here/frame_convert_test.m" \
    -framework WebRTC -framework CoreVideo -framework Foundation \
    -Wl,-rpath,"$frameworks" -o "$work/frame_convert_test"
"$work/frame_convert_test" "$@"

flutter=$(command -v flutter || true)
if [[ -z $flutter ]]; then
    echo "video_renderer_test: skipped (no flutter on PATH)"
    exit 0
fi
engine=$(cd "$(dirname "$flutter")" && pwd -P)/cache/artifacts/engine/darwin-x64
flutter_frameworks=$engine/FlutterMacOS.xcframework/macos-arm64_x86_64
clang -fobjc-arc -O1 -g -fsanitize=thread -Wall -Werror -mmacosx-version-min=12.0 \
    -F "$frameworks" -F "$flutter_frameworks" -I "$sources" -I "$sources/include/flutter_webrtc" \
    "$sources/FogataFrameConvert.m" "$sources/FlutterRTCVideoRenderer.m" \
    "$here/video_renderer_test.m" \
    -framework WebRTC -framework FlutterMacOS -framework CoreVideo -framework CoreGraphics \
    -framework Foundation \
    -Wl,-rpath,"$frameworks" -Wl,-rpath,"$flutter_frameworks" -o "$work/video_renderer_test"
TSAN_OPTIONS=halt_on_error=1 "$work/video_renderer_test"
