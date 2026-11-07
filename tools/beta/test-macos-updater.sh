#!/bin/bash
# Tests of the macOS updater helper (app/macos/Updater) that need no signing identity: the
# helper is built as tools/beta/macos-updater.sh builds it, inside a stand-in Fogata.app, and
# run on its own with a test container. Without a team signature it never gets as far as
# swapping, so these check what it refuses before that.
set -euo pipefail
[[ $(uname) == Darwin ]] || { echo "macOS only" >&2; exit 0; }
repo=$(cd "$(dirname "$0")/../.." && pwd)
# Not under /tmp or /var: the helper standardizes the path it is given, which turns
# /private/var into /var, and then it is not the resolved path.
mkdir -p "$repo/app/build"
tmp=$(mktemp -d "$repo/app/build/updater-test.XXXXXX")
trap 'rm -rf "${tmp:?}"' EXIT
app=$tmp/Applications/Fogata.app
container=$tmp/container
mkdir -p "$app/Contents/MacOS" "$container/updates"
# A stand-in for Fogata: waits while it has an argument, ends at once without (as when the
# helper opens it again).
printf '#include <unistd.h>\nint main(int c, char **v) { if (c > 1) pause(); return 0; }\n' |
    cc -x c - -o "$app/Contents/MacOS/Fogata"
"$repo/tools/beta/macos-updater.sh" --container "$container" "$app" > /dev/null
helper="$app/Contents/Library/Helpers/Fogata Updater.app/Contents/MacOS/Fogata Updater"
failed=0
fail() { echo "FAIL: $*" >&2; failed=1; }

new_version() {
    rm -rf "${container:?}/updates/Fogata.app" "$tmp/elsewhere"
    mkdir -p "$container/updates/Fogata.app/Contents"
}

# A new version named through a link is refused at once.
new_version
mkdir -p "$tmp/elsewhere"
ln -s "$container/updates" "$tmp/elsewhere/updates"
"$app/Contents/MacOS/Fogata" wait & fogata=$!
status=0
"$helper" "$fogata" "$tmp/elsewhere/updates/Fogata.app" > "$tmp/log" 2>&1 || status=$?
kill "$fogata"; wait "$fogata" 2>/dev/null || true
[[ $status == 2 ]] && grep -q "not at Fogata's place" "$tmp/log" ||
    fail "a new version named through a link: status $status, $(cat "$tmp/log")"

# The new version turns into a link to another bundle while the helper waits for Fogata to
# quit: the helper must not take it.
new_version
"$app/Contents/MacOS/Fogata" wait & fogata=$!
"$helper" "$fogata" "$container/updates/Fogata.app" > "$tmp/log" 2>&1 & helping=$!
sleep 1
kill -0 "$helping" 2>/dev/null || fail "the helper did not wait for Fogata: $(cat "$tmp/log")"
mv "$container/updates/Fogata.app" "$tmp/moved"
ln -s /System/Applications/Calculator.app "$container/updates/Fogata.app"
kill "$fogata"; wait "$fogata" 2>/dev/null || true
status=0
wait "$helping" || status=$?
grep -q "not at Fogata's place" "$tmp/log" ||
    fail "a new version that became a link while the helper waited went on: status $status, $(cat "$tmp/log")"
[[ -d /System/Applications/Calculator.app && -x $app/Contents/MacOS/Fogata ]] ||
    fail "the helper touched what it should not have"

# What the helper requires of the new version itself (app/macos/Updater/Checks.swift), tried
# on bundles signed ad hoc for the test: a sandboxed Fogata, newer than the installed build 5,
# and at most 2000 above it.
bundles=$tmp/bundles
mkdir -p "$bundles"
printf 'int main(void) { return 0; }\n' | cc -x c - -o "$tmp/program"
entitlements() {
    printf '<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0"><dict><key>com.apple.security.app-sandbox</key><%s/></dict></plist>\n' "$1" > "$tmp/$1.entitlements"
    echo "$tmp/$1.entitlements"
}
sandboxed=$(entitlements true)
not_sandboxed=$(entitlements false)
# bundle <name> <build> <identifier> <entitlements or "">
bundle() {
    local app=$bundles/$1
    mkdir -p "$app/Contents/MacOS"
    cp "$tmp/program" "$app/Contents/MacOS/Fogata"
    cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>$3</string>
<key>CFBundleExecutable</key><string>Fogata</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>$2</string>
</dict></plist>
PLIST
    if [[ -n $4 ]]; then
        codesign -s - -f --entitlements "$4" "$app" 2> /dev/null
    else
        codesign -s - -f "$app" 2> /dev/null
    fi
}
id=dev.holypanda.fogata
bundle installed.app 5 "$id" "$sandboxed"
bundle newer-yes.app 6 "$id" "$sandboxed"
bundle rescue-step-yes.app 2005 "$id" "$sandboxed"
bundle too-far-no.app 2006 "$id" "$sandboxed"
bundle same-build-no.app 5 "$id" "$sandboxed"
bundle older-no.app 4 "$id" "$sandboxed"
bundle not-sandboxed-no.app 6 "$id" ""
bundle sandbox-off-no.app 6 "$id" "$not_sandboxed"
bundle other-identifier-no.app 6 dev.holypanda.other "$sandboxed"
bundle not-a-build-no.app 6b "$id" "$sandboxed"
bundle changed-after-signing-no.app 6 "$id" "$sandboxed"
sed -i '' 's|<string>6</string>|<string>7</string>|' "$bundles/changed-after-signing-no.app/Contents/Info.plist"
bundle unsigned-no.app 6 "$id" ""
codesign --remove-signature "$bundles/unsigned-no.app"
# Top-level code compiles only from a file named main.swift.
mkdir -p "$tmp/checks"
cp "$repo/tools/beta/macos-updater-checks-test.swift" "$tmp/checks/main.swift"
swiftc -O "$repo/app/macos/Updater/Checks.swift" "$tmp/checks/main.swift" -o "$tmp/checks-test"
"$tmp/checks-test" "$bundles" 5 > "$tmp/log" 2>&1 ||
    fail "the checks of a new version: $(grep -v '^ok:' "$tmp/log")"

if [[ $failed == 0 ]]; then
    echo "test-macos-updater: all passed"
fi
exit $failed
