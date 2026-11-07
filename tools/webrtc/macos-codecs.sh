#!/usr/bin/env bash
# Checks that the macOS WebRTC library (the framework's binary, for arm64 and x86_64) has
# Opus and VP8 as its only codecs, and no frame cryptor (patches/apple/0004): no class of
# another codec is exported, and no file of one is named in the library (the file names its
# log and its checks keep). build-macos.sh runs it on what it built, and
# test-macos-codecs.sh on the archive the Podfile installs.
#
#     tools/webrtc/macos-codecs.sh <WebRTC.framework/Versions/A/WebRTC>
set -euo pipefail
binary=${1:?usage: macos-codecs.sh <framework binary>}
thin=$(mktemp -d)
trap 'rm -rf "${thin:?}"' EXIT
for cpu in arm64 x86_64; do
    lipo -thin "$cpu" "$binary" -output "$thin/$cpu"
    if nm -gU "$thin/$cpu" |
            grep -E 'OBJC_CLASS_\$_RTC(Video(De|En)coder(Factory)?(H264|H265|VP9|AV1)|FrameCryptor|DataPacketCryptor)' ||
        strings "$thin/$cpu" |
            grep -E 'modules/audio_coding/codecs/(g711|g722|ilbc|pcm16b)|modules/video_coding/codecs/h26[45]|components/video_codec/(RTCVideo(De|En)coderH26|nalu_rewriter)|api/crypto/frame_crypto|common_video/h265'; then
        echo "the $cpu framework holds a codec other than Opus and VP8, or the frame cryptor" >&2
        exit 1
    fi
done
