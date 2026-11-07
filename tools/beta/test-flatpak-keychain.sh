#!/usr/bin/env bash
# Where the Flatpak of Fogata keeps the data key, tried with fogata-core's own code in a
# Flatpak made with tools/beta/flatpak.sh (its permissions, no org.freedesktop.secrets), in a
# session of its own: a private D-Bus session bus, GNOME Keyring unlocked with a keyring of
# its own as the Secret Service and as the Secret portal's backend, xdg-desktop-portal, and a
# flatpak installation in a home folder of its own. Nothing of the person's session, keychain
# or flatpak installation is reached: the person's session bus is never named here.
#   1. A folder sealed on the computer, its key in the Secret Service (as an earlier Flatpak
#      kept it), does not open in the Flatpak, which says how to let it reach the key once.
#   2. With that permission given for a start, it opens (the same identity) and its key is
#      moved to the Flatpak's own keychain, sealed with the Secret portal's secret.
#   3. With the permission taken back, it opens all the same.
#   4. A new folder sealed in the Flatpak keeps its key there only, not in the Secret Service.
#   5. fogata-core's test of the system keychain (keep, give back, remove, find a key without
#      sealed.json) passes in the Flatpak.
#
# Usage: tools/beta/test-flatpak-keychain.sh [<fogata-core test binary>]
#   Without one, it is built (cargo test -p fogata-core --lib --no-run).
# Linux only; needs flatpak, the org.freedesktop.Platform 26.08 runtime, dbus-daemon,
# gnome-keyring-daemon and xdg-desktop-portal. Exits 3 when one is missing.
set -euo pipefail
[[ $(uname) == Linux ]] || { echo "Linux only" >&2; exit 0; }
repo=$(cd "$(dirname "$0")/../.." && pwd)
APP_ID=dev.holypanda.fogata
TEST=vault::tests::a_folder_with_the_system_keychain

missing=()
for tool in flatpak dbus-daemon dbus-send gdbus gnome-keyring-daemon strip; do
    command -v "$tool" > /dev/null || missing+=("$tool")
done
portal=
for file in /usr/libexec/xdg-desktop-portal /usr/lib/xdg-desktop-portal; do
    [[ -x $file ]] && portal=$file
done
[[ -n $portal ]] || missing+=(xdg-desktop-portal)
flatpak info --system org.freedesktop.Platform//26.08 > /dev/null 2>&1 ||
    flatpak info --user org.freedesktop.Platform//26.08 > /dev/null 2>&1 ||
    missing+=("the org.freedesktop.Platform 26.08 runtime")
if [[ ${#missing[@]} -gt 0 ]]; then
    echo "test-flatpak-keychain: missing ${missing[*]}" >&2
    exit 3
fi

binary=${1:-}
if [[ -z $binary ]]; then
    binary=$(cd "$repo" && cargo test -p fogata-core --lib --no-run --message-format=json 2> /dev/null |
        python3 -c 'import json, sys
for line in sys.stdin:
    m = json.loads(line)
    if m.get("reason") == "compiler-artifact" and m["target"]["name"] == "fogata_core" and m.get("executable"):
        print(m["executable"])')
fi
[[ -x $binary ]] || { echo "test-flatpak-keychain: no test binary at '$binary'" >&2; exit 1; }

tmp=$(mktemp -d "${TMPDIR:-/tmp}/fogata-flatpak-keychain.XXXXXX")
pids=()
bus=unix:path=$tmp/run/bus
cleanup() {
    # Everything started on the private bus: by the address it was given.
    local pid
    for pid in /proc/[0-9]*; do
        pid=${pid#/proc/}
        { tr '\0' '\n' < "/proc/$pid/environ"; } 2> /dev/null |
            grep -qxF "DBUS_SESSION_BUS_ADDRESS=$bus" && pids+=("$pid")
    done
    for pid in "${pids[@]}"; do
        kill "$pid" 2> /dev/null || true
    done
    fusermount3 -u "$tmp/run/doc" 2> /dev/null || fusermount -u "$tmp/run/doc" 2> /dev/null || true
    chmod -R u+w "$tmp" 2> /dev/null
    rm -rf "${tmp:?}" 2> /dev/null || true
}
trap cleanup EXIT

export HOME=$tmp/home XDG_DATA_HOME=$tmp/home/.local/share XDG_CONFIG_HOME=$tmp/home/.config \
    XDG_CACHE_HOME=$tmp/home/.cache XDG_RUNTIME_DIR=$tmp/run XDG_CURRENT_DESKTOP=GNOME
unset DISPLAY WAYLAND_DISPLAY
mkdir -p "$HOME" "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
export DBUS_SESSION_BUS_ADDRESS=$bus
# The bus starts the system's session services on demand, all but flatpak's session helper:
# flatpak runs without it, while one started here would hand the sandbox a socket of the
# computer's certificate store it never made.
mkdir -p "$tmp/services"
for service in /usr/share/dbus-1/services/*.service; do
    [[ $(basename "$service") == org.freedesktop.Flatpak.service ]] ||
        ln -s "$service" "$tmp/services/"
done
cat > "$tmp/bus.conf" <<CONF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>$bus</listen>
  <servicedir>$tmp/services</servicedir>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
CONF
dbus-daemon --config-file="$tmp/bus.conf" --nofork > "$tmp/dbus.log" 2>&1 &
pids+=($!)
for _ in $(seq 50); do [[ -S $XDG_RUNTIME_DIR/bus ]] && break; sleep 0.1; done
[[ -S $XDG_RUNTIME_DIR/bus ]] || { echo "the private bus did not start" >&2; exit 1; }
printf 'test' | gnome-keyring-daemon --foreground --unlock --components=secrets \
    > "$tmp/keyring.log" 2>&1 &
pids+=($!)
for _ in $(seq 50); do
    dbus-send --session --print-reply --dest=org.freedesktop.DBus / \
        org.freedesktop.DBus.NameHasOwner string:org.freedesktop.secrets 2> /dev/null |
        grep -q 'boolean true' && break
    sleep 0.1
done

# The portal, started now and given the time to find its backends (those of a desktop that
# is not here fail first), so that its Secret portal is there when the Flatpak asks.
for _ in $(seq 100); do
    gdbus introspect --session --dest org.freedesktop.portal.Desktop \
        --object-path /org/freedesktop/portal/desktop 2> /dev/null |
        grep -q 'interface org.freedesktop.portal.Secret' && break
    sleep 0.2
done

failed=0
fail() { echo "FAIL: $*" >&2; failed=1; }

# Without its debugging information: a debug test binary is hundreds of megabytes, which
# the repository, the bundle and the installation would each hold.
strip --strip-debug -o "$tmp/fogata-core-tests" "$binary"
"$repo/tools/beta/flatpak.sh" "$tmp/none" "$tmp/flatpak" --cli "$tmp/fogata-core-tests" --unsigned > /dev/null
flatpak install --user --noninteractive -y --bundle "$tmp/flatpak/Fogata.flatpak" \
    > "$tmp/install.log" 2>&1 || { cat "$tmp/install.log" >&2; exit 1; }

data=$HOME/.var/app/$APP_ID/data
keys=$data/fogata-keychain
mkdir -p "$data"

# on_computer|in_flatpak <seal-folder|open-folder> <dir>: the identity printed, or the error.
on_computer() {
    FOGATA_TEST_KEYCHAIN=$1 FOGATA_TEST_KEYCHAIN_DIR=$2 \
        "$binary" --exact "$TEST" --ignored --nocapture > "$tmp/out" 2>&1 || true
    sed -n 's/^identity: \([0-9a-f]*\) sealed: \(.*\)$/\1 \2/p' "$tmp/out"
}
in_flatpak() {
    flatpak run --user --command=fogata --env=FOGATA_TEST_KEYCHAIN="$1" \
        --env=FOGATA_TEST_KEYCHAIN_DIR="$2" "$APP_ID" --exact "$TEST" --ignored --nocapture \
        > "$tmp/out" 2>&1 || true
    sed -n 's/^identity: \([0-9a-f]*\) sealed: \(.*\)$/\1 \2/p' "$tmp/out"
}

# 1. A folder sealed on the computer, its key in the Secret Service.
before=$(on_computer seal-folder "$data/old")
[[ $before == *" true" ]] || fail "the folder was not sealed on the computer: $(cat "$tmp/out")"
got=$(in_flatpak open-folder "$data/old")
[[ -z $got ]] || fail "the Flatpak opened a folder whose key it should not reach: $got"
grep -q 'flatpak override --user --talk-name=org.freedesktop.secrets' "$tmp/out" ||
    fail "the Flatpak did not say how to reach the key: $(cat "$tmp/out")"
[[ ! -d $keys ]] || [[ -z $(ls -A "$keys") ]] || fail "a key file was written before the move"

# 2. Allowed once: the key moves.
flatpak override --user --talk-name=org.freedesktop.secrets "$APP_ID"
got=$(in_flatpak open-folder "$data/old")
[[ $got == "$before" ]] || fail "with the permission, the folder did not open as before: $got, $(cat "$tmp/out")"
name=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["key"])' "$data/old/sealed.json")
[[ -f $keys/$name.key ]] || fail "the key $name was not moved to $keys"
[[ $(stat -c %a "$keys/$name.key") == 600 && $(stat -c %a "$keys") == 700 ]] ||
    fail "the key file is readable by others"

# 3. The permission taken back: it opens from the Flatpak's own keychain.
flatpak override --user --no-talk-name=org.freedesktop.secrets "$APP_ID"
got=$(in_flatpak open-folder "$data/old")
[[ $got == "$before" ]] || fail "without the permission, the moved key did not open the folder: $got, $(cat "$tmp/out")"

# 4. A new folder sealed in the Flatpak: its key is not in the Secret Service.
new=$(in_flatpak seal-folder "$data/new")
[[ $new == *" true" ]] || fail "the Flatpak did not seal a new folder: $(cat "$tmp/out")"
[[ $(in_flatpak open-folder "$data/new") == "$new" ]] || fail "the new folder did not open again"
name=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["key"])' "$data/new/sealed.json")
[[ -f $keys/$name.key ]] || fail "the new folder's key is not in $keys"
got=$(on_computer open-folder "$data/new")
[[ -z $got ]] && grep -q "does not hold" "$tmp/out" ||
    fail "the new folder's key went to the Secret Service: $got, $(cat "$tmp/out")"

# 5. fogata-core's test of the system keychain, in the Flatpak.
flatpak run --user --command=fogata --env=FOGATA_TEST_KEYCHAIN=private "$APP_ID" \
    --exact vault::tests::the_system_keychain --ignored > "$tmp/out" 2>&1 ||
    fail "the system keychain's test in the Flatpak: $(cat "$tmp/out")"
grep -q '1 passed' "$tmp/out" || fail "the system keychain's test did not run: $(cat "$tmp/out")"

if [[ $failed == 0 ]]; then
    echo "test-flatpak-keychain: all passed"
fi
exit $failed
