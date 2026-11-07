#!/bin/bash
# Builds the updater helper (app/macos/Updater) and puts it into a release build of the
# macOS app, at Contents/Library/Helpers/Fogata Updater.app, for the architectures of the
# app. Run it after `flutter build macos --release` and before tools/sign-macos.sh, which
# signs the helper with the app's identity (hardened runtime, no sandbox).
#
# Usage: tools/beta/macos-updater.sh [--team <id>] [--container <path>] [Fogata.app]
#   --team       the team id the new version's signature must carry, pinned into the
#                helper (`security find-identity -v -p codesigning` shows it in brackets at
#                the end of the identity's name); without it the helper requires its own
#                signature's team. Give it for a release.
#   --container  where the helper looks for the new version: Fogata's container, as a path
#                under the home of the user running (the default,
#                Library/Containers/dev.holypanda.fogata/Data), or an absolute path for a
#                test copy run with FOGATA_DATA_DIR inside it. Tests only.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
team= container=Library/Containers/dev.holypanda.fogata/Data
while [ $# -gt 0 ]; do
  case $1 in
    --team) team=$2; shift 2 ;;
    --container) container=$2; shift 2 ;;
    *) break ;;
  esac
done
app=${1:-$repo/app/build/macos/Build/Products/Release/Fogata.app}
app=${app%/}
src=$repo/app/macos/Updater
[ -x "$app/Contents/MacOS/Fogata" ] || { echo "no app at $app" >&2; exit 1; }
if [ -n "$team" ] && ! [[ $team =~ ^[A-Z0-9]{10}$ ]]; then
  echo "a team id is ten capitals and digits, not $team" >&2
  exit 1
fi
case $container in
  *'"'* | *'\\'*) echo "the container path cannot hold quotes or backslashes" >&2; exit 1 ;;
esac
helper="$app/Contents/Library/Helpers/Fogata Updater.app"
rm -rf "$helper"
mkdir -p "$helper/Contents/MacOS"
cp "$src/Info.plist" "$helper/Contents/Info.plist"
# What the helper is built to require (main.swift reads it as Pinned).
pinned_dir=$(mktemp -d -t fogata-updater)
cat > "$pinned_dir/Pinned.swift" <<PINNED
enum Pinned {
  // Kept as one string, so tools/sign-macos.sh can read the team off the built helper.
  static let teamMarker = "fogata-updater-team:$team;"
  static var teamID: String? {
    let id = teamMarker.dropFirst("fogata-updater-team:".count).dropLast()
    return id.isEmpty ? nil : String(id)
  }
  static let container = "$container"
}
PINNED
outputs=()
for arch in $(lipo -archs "$app/Contents/MacOS/Fogata"); do
  out=$(mktemp -t fogata-updater)
  swiftc -O -target "$arch-apple-macos12" "$src/main.swift" "$src/Checks.swift" \
    "$pinned_dir/Pinned.swift" -o "$out"
  outputs+=("$out")
done
lipo -create "${outputs[@]}" -output "$helper/Contents/MacOS/Fogata Updater"
rm -f "${outputs[@]}"
rm -rf "${pinned_dir:?}"
echo "$helper: built for $(lipo -archs "$helper/Contents/MacOS/Fogata Updater"), team ${team:-of its own signature}, container $container"
