#!/bin/bash
# Signs a release build of the macOS app for handing out, with one signing identity and the
# hardened runtime: every framework and library inside the app first, then the app itself
# with its release entitlements (App Sandbox; no JIT, no debugger, library validation kept).
# The hardened runtime loads only libraries signed by the app's own team or by Apple, so an
# ad-hoc signature ("-"), which has no team, would stop the app at launch.
#
# Usage: tools/sign-macos.sh [--no-timestamp] "<identity>" [Fogata.app]
#   identity  a name or SHA-1 hash from `security find-identity -v -p codesigning`, such as
#             an "Apple Development" certificate (free with an Apple Account)
#   app       defaults to app/build/macos/Build/Products/Release/Fogata.app
# Every signature gets a trusted timestamp from Apple's server (the computer must be
# online), so it stays valid after the certificate expires (an Apple Development
# certificate lasts a year) and the updater inside the app keeps taking new versions. With
# --no-timestamp the signatures stop validating the day the certificate expires, and every
# tester must install again: only for a test.
# Run it after `flutter build macos --release` and after any change to the app's Info.plist.
# Every signature carries the identity's certificate, which anyone can read from the app
# (`codesign -dv --verbose=4 Fogata.app`): for an Apple Development certificate its subject
# names the Apple Account's e-mail and, for a personal team, the account holder's name.
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
timestamp=--timestamp
if [ "${1:-}" = --no-timestamp ]; then
  timestamp=--timestamp=none
  shift
fi
identity=${1:?usage: tools/sign-macos.sh [--no-timestamp] \"<identity>\" [Fogata.app]}
app=${2:-$repo/app/build/macos/Build/Products/Release/Fogata.app}
app=${app%/}
entitlements=$repo/app/macos/Runner/Release.entitlements

fail() {
  echo "sign-macos: $*" >&2
  exit 1
}

[ "$identity" != "-" ] || fail "an ad-hoc signature has no team, and the hardened runtime would refuse the app's own frameworks: name a signing identity"
[ -d "$app/Contents/MacOS" ] || fail "no app at $app"
case "/$app/" in
  */Debug/* | */Profile/*) fail "$app is not a release build: use flutter build macos --release" ;;
esac
# Flutter puts the Dart program as kernel code, run by the VM's JIT, only into debug builds.
[ ! -e "$app/Contents/Frameworks/App.framework/Resources/flutter_assets/kernel_blob.bin" ] ||
  fail "$app is a debug build: use flutter build macos --release"

sign() {
  codesign --force --sign "$identity" --options runtime "$timestamp" "$@"
}

# Deepest first, so each bundle seals code that is already signed.
while IFS= read -r -d '' code; do
  sign "$code"
done < <(find "$app/Contents" -depth \( -type d -name '*.framework' -o -type f -name '*.dylib' \) -print0)
# The updater helper (tools/beta/macos-updater.sh), a bundle of its own: signed without the
# sandbox, since it replaces the app's bundle once the app has quit.
helper="$app/Contents/Library/Helpers/Fogata Updater.app"
if [ -d "$helper" ]; then
  sign --entitlements "$repo/app/macos/Updater/Updater.entitlements" "$helper"
fi
sign --entitlements "$entitlements" "$app"

codesign --verify --deep --strict "$app" || fail "the signature does not verify"
details=$(codesign -dv --verbose=4 "$app" 2>&1)
grep -q '^CodeDirectory .*flags=0x[0-9a-f]*([a-z,-]*runtime' <<< "$details" ||
  fail "the app is not signed with the hardened runtime"
team=$(sed -n 's/^TeamIdentifier=//p' <<< "$details")
[ -n "$team" ] && [ "$team" != "not set" ] || fail "the signature names no team"
if [ "$timestamp" = --timestamp ]; then
  grep -q '^Timestamp=' <<< "$details" || fail "the signature carries no timestamp"
else
  echo "sign-macos: NO TIMESTAMP: these signatures stop validating when the certificate expires" >&2
fi
granted=$(codesign -d --entitlements - --xml "$app" 2>/dev/null)
for key in get-task-allow cs.allow-jit cs.allow-unsigned-executable-memory \
  cs.disable-library-validation cs.allow-dyld-environment-variables; do
  ! grep -q "com.apple.security.$key" <<< "$granted" || fail "the app has the entitlement $key"
done
# The updater installs only a sandboxed version (app/macos/Updater/Checks.swift).
grep -q '<key>com.apple.security.app-sandbox</key>[[:space:]]*<true/>' <<< "$granted" ||
  fail "the app is not sandboxed"
if [ -d "$helper" ]; then
  helper_granted=$(codesign -d --entitlements - --xml "$helper" 2>/dev/null)
  for key in get-task-allow cs.allow-jit cs.allow-unsigned-executable-memory \
    cs.disable-library-validation cs.allow-dyld-environment-variables; do
    ! grep -q "com.apple.security.$key" <<< "$helper_granted" || fail "the updater has the entitlement $key"
  done
fi

# Every piece of code the app can load must carry the same team.
others=0
while IFS= read -r -d '' file; do
  case $(file -b "$file") in
    *Mach-O*) ;;
    *) continue ;;
  esac
  theirs=$(codesign -dv "$file" 2>&1 | sed -n 's/^TeamIdentifier=//p')
  if [ "$theirs" != "$team" ]; then
    echo "sign-macos: ${file#"$app"/} is signed by team '${theirs:-none}', not $team" >&2
    others=$((others + 1))
  fi
done < <(find "$app" -type f -print0)
[ "$others" = 0 ] || fail "$others file(s) signed by another team"

# The helper requires the new version's signature to be of the team pinned into it, which
# must be this one.
if [ -d "$helper" ]; then
  pinned=$(strings -a "$helper/Contents/MacOS/Fogata Updater" | sed -n 's/^fogata-updater-team:\([A-Z0-9]*\);$/\1/p' | head -n 1)
  if [ -n "$pinned" ] && [ "$pinned" != "$team" ]; then
    fail "the updater was built for team $pinned (tools/beta/macos-updater.sh --team), the signature is team $team"
  fi
fi

echo "$app: signed by team $team with the hardened runtime"
