#!/bin/sh
# Builds and runs opened_video/main.swift against OpenedVideo.swift, with the integration
# tests' clip:  run_tests.sh [clip.mp4]
set -eu
here=$(cd "$(dirname "$0")" && pwd)
clip=${1:-"$here/../../../../integration_test/clip.mp4"}
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
swiftc -O -target "$(uname -m)-apple-macos12" -o "$out/opened_video_test" "$here/../Classes/OpenedVideo.swift" \
  "$here/opened_video/main.swift"
"$out/opened_video_test" "$clip"
