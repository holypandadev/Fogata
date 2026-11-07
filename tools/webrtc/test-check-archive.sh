#!/usr/bin/env bash
# Tests of check-archive.sh's arguments, with fetch.sh and the build scripts stood in for:
# what each system takes, what is fetched, what the build is given, and that a Windows
# toolchain folder is checked before anything is fetched.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "${tmp:?}"' EXIT
mkdir -p "$tmp/tools/webrtc" "$tmp/app/third_party/flutter_webrtc/third_party" "$tmp/app/macos"
cp "$here/check-archive.sh" "$here/pins.sh" "$tmp/tools/webrtc/"
. "$here/pins.sh"
# What a stand-in build makes, and its SHA-256, named by the stand-in commit.
printf 'stand-in archive' > "$tmp/archive"
if command -v sha256sum > /dev/null; then
    sum=$(sha256sum "$tmp/archive" | cut -c1-64)
else
    sum=$(shasum -a 256 "$tmp/archive" | cut -c1-64)
fi
printf 'sha256_linux-x64 = %s\nsha256_win-x64 = %s\n' "$sum" "$sum" \
    > "$tmp/app/third_party/flutter_webrtc/third_party/libwebrtc_version.ini"
cat > "$tmp/tools/webrtc/fetch.sh" <<STUB
#!/usr/bin/env bash
echo "fetch \$*" >> "$tmp/calls"
STUB
for system in linux windows; do
    archive=libwebrtc-linux-x64-release.zip
    [[ $system == windows ]] && archive=libwebrtc-win-x64-release.zip
    cat > "$tmp/tools/webrtc/build-$system.sh" <<STUB
#!/usr/bin/env bash
echo "build-$system \$*" >> "$tmp/calls"
mkdir -p "\$1/dist/$LIBWEBRTC_VERSION"
cp "$tmp/archive" "\$1/dist/$LIBWEBRTC_VERSION/$archive"
STUB
done
chmod +x "$tmp"/tools/webrtc/*.sh
check="$tmp/tools/webrtc/check-archive.sh"
failed=0
fail() { echo "FAIL: $*" >&2; failed=1; }
run() { rm -f "$tmp/calls"; touch "$tmp/calls"; status=0; "$check" "$@" > "$tmp/out" 2>&1 || status=$?; }

run linux "$tmp/w1"
[[ $status == 0 ]] || fail "linux: status $status, $(cat "$tmp/out")"
grep -qx "fetch $tmp/w1 linux" "$tmp/calls" || fail "linux fetched: $(cat "$tmp/calls")"
grep -qx "build-linux $tmp/w1" "$tmp/calls" || fail "linux built: $(cat "$tmp/calls")"

run windows "$tmp/w2"
[[ $status == 2 && ! -s $tmp/calls ]] || fail "windows without a toolchain: status $status, $(cat "$tmp/calls")"

mkdir -p "$tmp/empty"
run windows "$tmp/w3" "$tmp/empty"
[[ $status == 1 && ! -s $tmp/calls ]] || fail "windows with no toolchain archive: status $status, $(cat "$tmp/calls")"

mkdir -p "$tmp/toolchain"
touch "$tmp/toolchain/0123abcd.zip"
run windows "$tmp/w4" "$tmp/toolchain"
[[ $status == 0 ]] || fail "windows: status $status, $(cat "$tmp/out")"
# Linux's dependencies only: build-windows.sh fetches Windows' with the toolchain given.
grep -qx "fetch $tmp/w4 linux" "$tmp/calls" || fail "windows fetched: $(cat "$tmp/calls")"
grep -qx "build-windows $tmp/w4 $tmp/toolchain" "$tmp/calls" || fail "windows built: $(cat "$tmp/calls")"

touch "$tmp/toolchain/4567cdef.zip"
run windows "$tmp/w5" "$tmp/toolchain"
[[ $status == 1 && ! -s $tmp/calls ]] || fail "windows with two toolchain archives: status $status"

[[ $failed == 0 ]] && echo "test-check-archive: all passed"
exit $failed
