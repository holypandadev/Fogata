#!/usr/bin/env bash
# The camera through the Camera portal and PipeWire, in Flatpaks made by tools/beta/flatpak.sh
# (the application's own permissions: no --device=all), in a desktop session of its own
# (tools/smoke-wayland/session.sh: KWin on a virtual output, its own session bus, PipeWire,
# WirePlumber and desktop portal, no device of the computer) with a made-up camera
# (tools/smoke-wayland/camera_source.c), and a flatpak installation of its own:
#   1. libwebrtc's own test of the portal's cameras (camera_portal_test.cc in
#      app/third_party/flutter_webrtc/common/cpp/test), in a Flatpak: the portal asks the
#      person (the session's picker grants), the camera is listed with the id V4L2 gives it,
#      opens and gives pictures of the size asked, and a camera plugged in and out meanwhile
#      (another made-up one, started and ended outside the sandbox) comes and goes;
#   2. the person says no: nothing is listed or opens; asked again, the system says no
#      without asking;
#   3. the application (app/integration_test/camera_portal_test.dart), in a Flatpak: the
#      settings ask for the cameras (the picker grants), and the camera opens as a call
#      opens it, its pictures reaching Flutter at the size asked; started again, nothing is
#      asked;
#   4. the application with the camera refused: the settings and the call say the system
#      refused it, and nothing is asked;
#   5. outside a Flatpak, libwebrtc's test with no portal to answer: unavailable.
# It also runs libwebrtc's test outside a Flatpak through the portal, and says what the
# portal answered there (granted or not, asked or not), without judging it: a plain build
# opens cameras directly unless FOGATA_CAMERA=portal (camera_access.h).
#
# Usage: tools/beta/test-flatpak-camera.sh [<bundle>]
#   <bundle>  `flutter build linux --debug -t integration_test/camera_portal_test.dart`'s
#             (app/build/linux/x64/debug/bundle), built when not given
# Needs what session.sh needs, flatpak with the org.freedesktop.Platform 26.08 runtime, a C
# compiler, PipeWire's headers and libwebrtc unpacked by a Linux build of the app
# (app/third_party/flutter_webrtc/third_party/libwebrtc, or LIBWEBRTC=<folder>). Exits 0
# when all passed, 1 when one failed, 3 when it cannot run here. FOGATA_CAMERA_TEST_OUT
# keeps the logs there; else they go in a folder of their own under $TMPDIR, kept and named
# when something failed.
set -uo pipefail
APP_ID=dev.holypanda.fogata
CAMERA="Fogata test camera"
BUS=usb-0000:00:14.0-9
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)

if [[ ${CAMERA_TEST_INSIDE:-} == 1 ]]; then
    out=$CAMERA_TEST_OUT
    failed=0
    fail() { echo "FAIL: $*" | tee -a "$out/summary"; failed=1; }
    pass() { echo "ok: $*" | tee -a "$out/summary"; }
    dialogs() { grep -c "^$1 " "$SMOKE_PICKER/dialogs" 2> /dev/null || true; }
    answer() { echo "$1" > "$SMOKE_PICKER/answer"; }

    "$out/camera_source" "$CAMERA" "$BUS" > "$out/camera.log" 2>&1 &
    camera=$!
    for _ in $(seq 50); do
        pw-cli ls Node 2> /dev/null | grep -q "node.description = \"$CAMERA\"" && break
        sleep 0.2
    done
    pw-cli ls Node 2> /dev/null | grep -q "node.description = \"$CAMERA\"" ||
        { echo "the made-up camera did not start (see $out/camera.log)" >&2; exit 3; }

    # The portal offers its Camera interface only when it could watch PipeWire's cameras as
    # it started; started on demand before PipeWire was ready, it has none. Then the
    # session's own portal is started again (it is this session's, by its PID), as logging
    # in again would.
    has_camera() {
        gdbus introspect --session --dest org.freedesktop.portal.Desktop \
            --object-path /org/freedesktop/portal/desktop 2> /dev/null |
            grep -q 'interface org.freedesktop.portal.Camera '
    }
    if ! has_camera; then
        for pid in $(pgrep -f '(^|/)xdg-desktop-portal$'); do
            kill "$pid"
        done
        for _ in $(seq 50); do has_camera && break; sleep 0.2; done
    fi
    has_camera || { echo "the desktop portal offers no Camera interface here" >&2; exit 3; }

    install() {
        flatpak install --user --noninteractive -y --bundle "$1" > "$out/install.log" 2>&1 ||
            { cat "$out/install.log" >&2; exit 1; }
    }
    # The second camera libwebrtc's test asks for: plugged in when it writes plug/plug,
    # out when it writes plug/unplug (from here, outside the sandbox).
    plugger() {
        rm -rf "$out/plug"
        mkdir -p "$out/plug"
        (
            while [[ ! -e $out/plug/plug ]]; do sleep 0.2; done
            "$out/camera_source" "Second test camera" > "$out/second-camera.log" 2>&1 &
            second=$!
            while [[ ! -e $out/plug/unplug ]]; do sleep 0.2; done
            kill "$second"
            wait "$second"
        ) &
        plugging=$!
    }
    # native <expected> <log>: libwebrtc's test in the Flatpak.
    native() {
        plugger
        flatpak run --user --filesystem="$out" --env=FOGATA_TEST_CAMERA_PORTAL="$1" \
            --env=FOGATA_TEST_CAMERA_NAME="$CAMERA" --env=FOGATA_TEST_CAMERA_ID="$BUS" \
            --env=FOGATA_TEST_CAMERA_PLUG="$out/plug" "$APP_ID" > "$out/$2" 2>&1
        local status=$?
        kill "$plugging" 2> /dev/null
        return $status
    }
    # app <expected> <log>: the application's test in the Flatpak, until it tells.
    app() {
        rm -f "$out/camera-result.txt"
        flatpak run --user --filesystem="$out" --env=FOGATA_TEST_CAMERA_PORTAL="$1" \
            --env=FOGATA_TEST_CAMERA_NAME="$CAMERA" --env=FOGATA_TEST_CAMERA_ID="$BUS" \
            --env=FOGATA_TEST_OUT="$out" --env=FOGATA_DATA_DIR="$out/data" \
            --env=LP_NUM_THREADS=2 "$APP_ID" > "$out/$2" 2>&1 &
        local pid=$!
        for _ in $(seq 300); do
            [[ -s $out/camera-result.txt ]] && break
            kill -0 "$pid" 2> /dev/null || break
            sleep 0.5
        done
        sleep 1
        kill "$pid" 2> /dev/null
        wait "$pid" 2> /dev/null
        flatpak kill "$APP_ID" 2> /dev/null
        [[ $(head -n 1 "$out/camera-result.txt" 2> /dev/null) == passed ]]
    }
    reset() { flatpak permission-remove devices camera "$APP_ID" > /dev/null 2>&1 || true; }

    # 1. Granted, in libwebrtc's test.
    install "$out/native/Fogata.flatpak"
    reset
    answer share
    native granted native-granted.log && pass "libwebrtc's test in the Flatpak, granted" ||
        fail "libwebrtc's test in the Flatpak, granted (see $out/native-granted.log)"
    [[ $(dialogs granted) == 1 ]] && pass "the portal asked once, and was granted" ||
        fail "the portal's question was not seen and granted once ($SMOKE_PICKER/dialogs)"

    # 2. Refused.
    reset
    answer cancel
    native denied native-denied.log && pass "libwebrtc's test in the Flatpak, denied" ||
        fail "libwebrtc's test in the Flatpak, denied (see $out/native-denied.log)"
    [[ $(dialogs denied) == 1 ]] && pass "the portal asked, and was denied" ||
        fail "the portal's question was not seen and denied ($SMOKE_PICKER/dialogs)"
    native denied native-denied-again.log && [[ $(dialogs shown) == 2 ]] &&
        pass "asked again, the system says no without asking" ||
        fail "refused before, asked again (see $out/native-denied-again.log, $SMOKE_PICKER/dialogs)"

    # 3. The application, granted.
    install "$out/app/Fogata.flatpak"
    reset
    answer share
    app granted app-granted.log && pass "the application's camera in the Flatpak" ||
        fail "the application's camera in the Flatpak (see $out/app-granted.log, $out/camera-result.txt)"
    [[ $(dialogs granted) == 2 ]] && pass "the application's question was asked and granted" ||
        fail "the application's question was not seen and granted ($SMOKE_PICKER/dialogs)"
    app granted app-granted-again.log && [[ $(dialogs shown) == 3 ]] &&
        pass "started again, the application is not asked again" ||
        fail "started again (see $out/app-granted-again.log, $SMOKE_PICKER/dialogs)"

    # 4. The application, refused in the system's settings.
    flatpak permission-set devices camera "$APP_ID" no > /dev/null 2>&1 ||
        fail "cannot refuse the camera with flatpak permission-set"
    app denied app-denied.log && [[ $(dialogs shown) == 3 ]] &&
        pass "refused, the application says so, and nothing is asked" ||
        fail "the application with the camera refused (see $out/app-denied.log, $out/camera-result.txt)"

    # 5. Outside a Flatpak: no portal at all, then the portal.
    DBUS_SESSION_BUS_ADDRESS=unix:path=$out/no-bus FOGATA_TEST_CAMERA_PORTAL=unavailable \
        "$out/camera_portal_test" > "$out/plain-unavailable.log" 2>&1 &&
        pass "without a portal, unavailable" ||
        fail "without a portal (see $out/plain-unavailable.log)"
    before=$(dialogs shown)
    answer share
    plugger
    FOGATA_TEST_CAMERA_PORTAL=granted FOGATA_TEST_CAMERA_NAME="$CAMERA" \
        FOGATA_TEST_CAMERA_ID="$BUS" FOGATA_TEST_CAMERA_PLUG="$out/plug" \
        "$out/camera_portal_test" > "$out/plain-portal.log" 2>&1
    echo "outside a Flatpak, through the portal: status $?, $(($(dialogs shown) - before)) question(s) asked; $(grep -m1 "portal's answer" "$out/plain-portal.log")" |
        tee -a "$out/summary"

    kill "$camera" 2> /dev/null
    wait "$camera" 2> /dev/null
    exit $failed
fi

# Outside: everything built, then the session.
missing=()
for tool in flatpak cc pkg-config; do
    command -v "$tool" > /dev/null || missing+=("$tool")
done
pkg-config --exists libpipewire-0.3 glib-2.0 || missing+=("PipeWire's or GLib's headers")
flatpak info --system org.freedesktop.Platform//26.08 > /dev/null 2>&1 ||
    flatpak info --user org.freedesktop.Platform//26.08 > /dev/null 2>&1 ||
    missing+=("the org.freedesktop.Platform 26.08 runtime")
webrtc=${LIBWEBRTC:-$repo/app/third_party/flutter_webrtc/third_party/libwebrtc}
[[ -e $webrtc/include/libwebrtc.h ]] || missing+=("libwebrtc in $webrtc (build the app for Linux once)")
if [[ ${#missing[@]} -gt 0 ]]; then
    echo "test-flatpak-camera: missing ${missing[*]}" >&2
    exit 3
fi
if [[ -n ${FOGATA_CAMERA_TEST_OUT:-} ]]; then
    out=$FOGATA_CAMERA_TEST_OUT
    mkdir -p "$out"
else
    out=$(mktemp -d "${TMPDIR:-/tmp}/fogata-camera-test.XXXXXX")
fi
out=$(cd "$out" && pwd)

bundle=${1:-}
if [[ -z $bundle ]]; then
    (cd "$repo/app" && flutter build linux --debug -t integration_test/camera_portal_test.dart) \
        > "$out/flutter-build.log" 2>&1 ||
        { echo "the application's test did not build (see $out/flutter-build.log)" >&2; exit 1; }
    bundle=$repo/app/build/linux/x64/debug/bundle
fi
lib=$(dirname "$(find "$webrtc/lib" -name libwebrtc.so | head -n 1)")
cc -O2 -o "$out/camera_source" "$repo/tools/smoke-wayland/camera_source.c" \
    $(pkg-config --cflags --libs libpipewire-0.3) || exit 1
test=$repo/app/third_party/flutter_webrtc
"${CXX:-clang++}" -std=c++17 -O1 -DRTC_DESKTOP_DEVICE -I"$test/linux" -I"$webrtc/include" \
    $(pkg-config --cflags glib-2.0) "$test/common/cpp/test/camera_portal_test.cc" \
    "$test/linux/camera_access.cc" -L"$lib" -Wl,-rpath,"$lib" -lwebrtc -lpthread \
    $(pkg-config --libs glib-2.0) -o "$out/camera_portal_test" || exit 1
# In the Flatpak libwebrtc is in /app/lib, where the runtime's loader looks.
mkdir -p "$out/libs"
cp "$lib/libwebrtc.so" "$out/libs/"
"$here/flatpak.sh" none "$out/native" --cli "$out/camera_portal_test" --extra-libs "$out/libs" \
    --unsigned > "$out/flatpak-native.log" 2>&1 ||
    { cat "$out/flatpak-native.log" >&2; exit 1; }
"$here/flatpak.sh" "$bundle" "$out/app" --unsigned --test-bundle > "$out/flatpak-app.log" 2>&1 ||
    { cat "$out/flatpak-app.log" >&2; exit 1; }

CAMERA_TEST_INSIDE=1 CAMERA_TEST_OUT=$out SMOKE_SESSION_CHECK=0 \
    "$repo/tools/smoke-wayland/session.sh" "$out/session" "$0"
status=$?
echo
cat "$out/summary" 2> /dev/null
if [[ $status == 0 ]]; then
    echo "test-flatpak-camera: all passed"
    [[ -n ${FOGATA_CAMERA_TEST_OUT:-} ]] || rm -rf "${out:?}"
else
    echo "test-flatpak-camera: failed (status $status); logs in $out"
fi
exit $status
