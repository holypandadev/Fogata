#!/usr/bin/env bash
# Builds and runs the native tests of the Linux player's code that needs no GStreamer: the
# HDR tone mapping (tone_map.h) and the packing of padded rows (rows.h). Any C++17
# compiler will do, on any system.
#
#     run_tests.sh [--sanitize]
#
# --sanitize builds them with AddressSanitizer and UndefinedBehaviorSanitizer (GCC or Clang),
# any finding fatal: tools/check.sh runs them so on Linux too.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
flags=(-O1)
case "${1:-}" in
  "") ;;
  --sanitize) flags=(-O1 -g -fno-omit-frame-pointer -fsanitize=address,undefined -fno-sanitize-recover=all) ;;
  *) echo "usage: run_tests.sh [--sanitize]" >&2; exit 2 ;;
esac
out=$(mktemp -d)
trap 'rm -rf "${out:?}"' EXIT
"${CXX:-c++}" -std=c++17 "${flags[@]}" -Wall -Wextra -Werror -pthread "$here/tone_map_test.cc" \
  -o "$out/tone_map_test"
"$out/tone_map_test"
"${CXX:-c++}" -std=c++17 "${flags[@]}" -Wall -Wextra -Werror "$here/rows_test.cc" -o "$out/rows_test"
"$out/rows_test"
