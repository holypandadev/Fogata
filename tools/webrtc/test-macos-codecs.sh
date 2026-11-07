#!/usr/bin/env bash
# Tests of macos-codecs.sh, on a Mac: small libraries made here, one clean and others that
# export another codec's class or name another codec's file, then the WebRTC archive the
# Podfile installs (WebRTC-SDK.podspec), when this computer's store has it.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "${work:?}"' EXIT

# A library for both processors, from C `source`.
library() {
    printf '%s\n' "$2" > "$work/$1.c"
    cc -dynamiclib -arch arm64 -arch x86_64 -o "$work/$1" "$work/$1.c"
}
library clean 'int vp8_and_opus(void) { return 8; }'
library h264 'int OBJC_CLASS_$_RTCVideoEncoderH264 = 1;'
library g711 'const char *log_file(void) { return "../../modules/audio_coding/codecs/g711/g711.c"; }'
library cryptor 'int OBJC_CLASS_$_RTCFrameCryptor = 1;'

"$here/macos-codecs.sh" "$work/clean" > /dev/null
for bad in h264 g711 cryptor; do
    if "$here/macos-codecs.sh" "$work/$bad" > /dev/null 2>&1; then
        echo "test-macos-codecs: the library with $bad was taken" >&2
        exit 1
    fi
done

# The archive itself, as the podspec finds it.
version=$(sed -n "s/^  s.version = '\(.*\)'$/\1/p" "$repo/app/macos/WebRTC-SDK.podspec")
store=${FOGATA_LIBWEBRTC_DIR:-$HOME/fogata-libwebrtc}
archive=$store/fogata-libwebrtc.m$version/WebRTC.xcframework.zip
if [[ -f $archive ]]; then
    unzip -q "$archive" -d "$work/archive"
    "$here/macos-codecs.sh" \
        "$work/archive/WebRTC.xcframework/macos-arm64_x86_64/WebRTC.framework/Versions/A/WebRTC"
    echo "test-macos-codecs: passed (with $archive)"
else
    echo "test-macos-codecs: passed (no $archive here to check)"
fi
