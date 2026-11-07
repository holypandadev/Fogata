#!/usr/bin/env bash
# Builds and runs the native tests of the Linux player's code that needs no GStreamer: the
# HDR tone mapping (tone_map.h) and the packing of padded rows (rows.h). Any C++17
# compiler will do, on any system.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$(mktemp -d)
trap 'rm -rf "${out:?}"' EXIT
"${CXX:-c++}" -std=c++17 -O1 -Wall -Wextra -Werror -pthread "$here/tone_map_test.cc" \
  -o "$out/tone_map_test"
"$out/tone_map_test"
"${CXX:-c++}" -std=c++17 -O1 -Wall -Wextra -Werror "$here/rows_test.cc" -o "$out/rows_test"
"$out/rows_test"
