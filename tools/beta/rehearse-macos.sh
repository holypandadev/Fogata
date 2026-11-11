#!/bin/bash
# The rehearsal of installing and updating Fogata on macOS (docs/releasing-beta.md,
# "Rehearsing on macOS"): what to build, and what to check after each step. It signs only
# in `build`, with the identity it is given; every other step only looks.
#
#   tools/beta/rehearse-macos.sh prereqs
#       What the rehearsal needs on this Mac. Uses no key: it only lists the signing
#       identities' names.
#   tools/beta/rehearse-macos.sh build <build> <version> <identity> <team> <out folder>
#       A release build of this commit, as docs/releasing-beta.md makes one: flutter build,
#       the updater for <team>, signed with <identity>, zipped as release.py takes it, with
#       its SHA-256 beside it. Refuses a checkout with changes.
#   tools/beta/rehearse-macos.sh installed <build> <team> [Fogata.app]
#       Checks an installed copy: signed by <team> with the hardened runtime and the
#       sandbox, its updater pinned to <team> and outside the sandbox, of build <build>,
#       readable by every user. Fogata.app is ~/Applications/Fogata.app by default.
#   tools/beta/rehearse-macos.sh updated <build> <team> [Fogata.app]
#       As `installed`, after an update to <build>; and nothing of the update is left: no
#       .Fogata.app.old beside it, no new version waiting in the container, no quarantine on
#       the bundle the updater put in place.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
container=$HOME/Library/Containers/dev.holypanda.fogata/Data
staged="$container/Library/Application Support/Fogata/updates/Fogata.app"
failed=0
ok() { echo "  ok    $*"; }
bad() { echo "  FAIL  $*"; failed=1; }
note() { echo "  note  $*"; }

prereqs() {
  echo "macOS $(sw_vers -productVersion) on $(uname -m), user $(id -un)"
  if [[ -d $container ]]; then
    note "this user already has a Fogata container: a clean install wants a user that never ran Fogata"
  else
    ok "no Fogata container for this user: a clean install starts here"
  fi
  for tool in swiftc lipo codesign ditto shasum cmake flutter cargo rustup; do
    if command -v "$tool" > /dev/null; then ok "$tool"; else bad "$tool is missing"; fi
  done
  if command -v rustup > /dev/null; then
    local toolchain
    toolchain=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$repo/rust-toolchain.toml")
    local targets
    targets=$(rustup target list --installed --toolchain "$toolchain" 2>/dev/null || true)
    for target in aarch64-apple-darwin x86_64-apple-darwin; do
      if grep -qx "$target" <<< "$targets"; then ok "Rust $toolchain $target"
      else bad "Rust $toolchain lacks $target: rustup target add $target --toolchain $toolchain"; fi
    done
  fi
  local identities
  identities=$(security find-identity -v -p codesigning | grep -c '"' || true)
  if [[ $identities -gt 0 ]]; then
    ok "$identities signing identities (security find-identity -v -p codesigning names them)"
  else
    bad "no signing identity: make an Apple Development certificate in Xcode first"
  fi
  local free
  free=$(df -g "$HOME" | awk 'NR == 2 { print $4 }')
  if [[ $free -ge 20 ]]; then ok "$free GB free"; else bad "$free GB free: a release build wants 20"; fi
}

build() {
  local build=${1:?build} version=${2:?version} identity=${3:?identity} team=${4:?team}
  local out=${5:?out folder}
  [[ $build =~ ^[0-9]+$ ]] || { echo "the build is a number" >&2; exit 2; }
  [[ $team =~ ^[A-Z0-9]{10}$ ]] || { echo "a team id is ten capitals and digits" >&2; exit 2; }
  cd "$repo"
  if [[ -n $(git status --porcelain --untracked-files=normal -- crates vendor app tools Cargo.toml Cargo.lock) ]]; then
    echo "the checkout has changes: every build of a release comes from a commit as it is" >&2
    exit 1
  fi
  grep -q "THIS_BUILD: u64 = $build;" crates/fogata-update/src/lib.rs ||
    { echo "THIS_BUILD in crates/fogata-update/src/lib.rs is not $build" >&2; exit 1; }
  grep -q "^version: .*+$build\$" app/pubspec.yaml ||
    { echo "app/pubspec.yaml's version does not end in +$build" >&2; exit 1; }
  mkdir -p "$out"
  (cd app && flutter build macos --release)
  local app=app/build/macos/Build/Products/Release/Fogata.app
  tools/beta/macos-updater.sh --team "$team" "$app"
  tools/sign-macos.sh "$identity" "$app"
  local zip="$out/Fogata-$version-macos-universal.zip"
  rm -f "$zip"
  ditto -c -k --keepParent "$app" "$zip"
  (cd "$out" && shasum -a 256 "$(basename "$zip")" > "$(basename "$zip").sha256")
  echo "$zip, commit $(git rev-parse HEAD):"
  cat "$zip.sha256"
}

check_bundle() {
  local build=$1 team=$2 app=$3
  local helper="$app/Contents/Library/Helpers/Fogata Updater.app"
  [[ -x $app/Contents/MacOS/Fogata ]] || { bad "no Fogata at $app"; return; }
  if codesign --verify --deep --strict "$app" 2> /dev/null; then ok "the signature verifies"
  else bad "the signature does not verify"; fi
  local details
  details=$(codesign -dv --verbose=4 "$app" 2>&1)
  [[ $(sed -n 's/^TeamIdentifier=//p' <<< "$details") == "$team" ]] &&
    ok "signed by team $team" || bad "not signed by team $team"
  grep -q '^CodeDirectory .*flags=0x[0-9a-f]*([a-z,-]*runtime' <<< "$details" &&
    ok "hardened runtime" || bad "no hardened runtime"
  grep -q '^Timestamp=' <<< "$details" && ok "timestamped" || bad "no timestamp"
  codesign -d --entitlements - --xml "$app" 2> /dev/null | grep -q '<key>com.apple.security.app-sandbox</key>[[:space:]]*<true/>' &&
    ok "sandboxed" || bad "not sandboxed"
  local found
  found=$(defaults read "$app/Contents/Info.plist" CFBundleVersion 2> /dev/null || true)
  [[ $found == "$build" ]] && ok "build $build" || bad "build ${found:-unknown}, not $build"
  [[ $(stat -f %Lp "$app") == 755 ]] && ok "readable by every user (755)" ||
    bad "mode $(stat -f %Lp "$app"), not 755"
  if [[ -d $helper ]]; then
    codesign --verify --strict "$helper" 2> /dev/null && ok "the updater's signature verifies" ||
      bad "the updater's signature does not verify"
    ! codesign -d --entitlements - --xml "$helper" 2> /dev/null | grep -q 'app-sandbox' &&
      ok "the updater is outside the sandbox" || bad "the updater is sandboxed: it could not swap"
    local pinned
    pinned=$(strings -a "$helper/Contents/MacOS/Fogata Updater" |
      sed -n 's/^fogata-updater-team:\([A-Z0-9]*\);$/\1/p' | head -n 1)
    [[ $pinned == "$team" ]] && ok "the updater takes team $team only" ||
      bad "the updater is pinned to '${pinned:-its own signature}', not $team"
  else
    bad "no updater inside: this copy cannot update itself"
  fi
  # Not notarized (Apple Development): Gatekeeper refuses it until "Open Anyway".
  note "Gatekeeper: $(spctl --assess --type execute -vv "$app" 2>&1 | head -n 1)"
  local quarantine
  quarantine=$(xattr -p com.apple.quarantine "$app" 2> /dev/null || true)
  note "quarantine: ${quarantine:-none}"
}

installed() {
  local app=${3:-$HOME/Applications/Fogata.app}
  echo "Installed copy at $app"
  check_bundle "${1:?build}" "${2:?team}" "${app%/}"
}

updated() {
  local app=${3:-$HOME/Applications/Fogata.app}
  app=${app%/}
  echo "Updated copy at $app"
  check_bundle "${1:?build}" "${2:?team}" "$app"
  [[ ! -e $(dirname "$app")/.Fogata.app.old ]] && ok "no old version left beside it" ||
    bad "the old version is still at $(dirname "$app")/.Fogata.app.old"
  [[ ! -e $staged ]] && ok "no new version left waiting in the container" ||
    bad "a new version is still at $staged"
  [[ -z $(xattr -p com.apple.quarantine "$app" 2> /dev/null || true) ]] &&
    ok "no quarantine on the bundle put in place" || bad "the bundle put in place is quarantined"
}

command=${1:-}
shift || true
case $command in
  prereqs) prereqs ;;
  build) build "$@" ;;
  installed) installed "$@" ;;
  updated) updated "$@" ;;
  *) sed -n '2,24p' "$0" >&2; exit 2 ;;
esac
exit $failed
