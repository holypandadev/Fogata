#!/usr/bin/env bash
# What a Flatpak of Fogata takes from its loopback origin (http://127.0.0.1:47811/) when
# something other than Fogata answers there, as `flatpak update` and software centres fetch
# it: small Flatpaks made with tools/beta/flatpak.sh, installed into a flatpak installation
# of a home folder of its own (nothing of the computer's own flatpak installation, keychain
# or session is touched), each update served by a plain HTTP server on the port:
#   - nothing answering: nothing changes;
#   - a newer commit unsigned, or signed by another key: refused;
#   - a newer commit signed by the Flatpak signing key: installed, with no release manifest
#     at all (so that key alone decides, as docs/architecture.md, 7, says);
#   - an older commit signed by the key, offered to a newer one: refused.
# Linux only; needs flatpak, gpg, python3 and the org.freedesktop.Platform runtime that
# tools/beta/flatpak.sh builds on, installed in the system's or the user's installation, and
# the port free. Exits 3 when one of these is missing.
set -euo pipefail
[[ $(uname) == Linux ]] || { echo "Linux only" >&2; exit 0; }
repo=$(cd "$(dirname "$0")/../.." && pwd)
PORT=47811
APP_ID=dev.holypanda.fogata
for tool in flatpak gpg python3; do
    command -v "$tool" > /dev/null || { echo "test-flatpak-origin: no $tool" >&2; exit 3; }
done
flatpak info --system org.freedesktop.Platform//26.08 > /dev/null 2>&1 ||
    flatpak info --user org.freedesktop.Platform//26.08 > /dev/null 2>&1 ||
    { echo "test-flatpak-origin: no org.freedesktop.Platform 26.08 runtime" >&2; exit 3; }
if python3 -c "import socket,sys; s=socket.socket(); s.connect(('127.0.0.1', $PORT))" 2> /dev/null; then
    echo "test-flatpak-origin: port $PORT is in use" >&2
    exit 3
fi

tmp=$(mktemp -d "${TMPDIR:-/tmp}/fogata-flatpak-origin.XXXXXX")
server=
cleanup() {
    [[ -n $server ]] && kill "$server" 2> /dev/null && wait "$server" 2> /dev/null
    for home in "$tmp"/keys/*; do
        [[ -d $home ]] && gpgconf --homedir "$home" --kill gpg-agent 2> /dev/null
    done
    chmod -R u+w "$tmp" 2> /dev/null
    rm -rf "${tmp:?}"
}
trap cleanup EXIT

# The test's own home: its flatpak installation, settings and runtime folder. No session
# bus: nothing here may reach the person's.
export HOME=$tmp/home XDG_DATA_HOME=$tmp/home/.local/share XDG_CONFIG_HOME=$tmp/home/.config \
    XDG_CACHE_HOME=$tmp/home/.cache XDG_RUNTIME_DIR=$tmp/run DBUS_SESSION_BUS_ADDRESS=disabled:
mkdir -p "$HOME" "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

failed=0
fail() { echo "FAIL: $*" >&2; failed=1; }

mkdir -p "$tmp/keys"
for key in good other; do
    FOGATA_GPG_HOMEDIR=$tmp/keys/$key "$repo/tools/beta/flatpak-key.sh" --no-passphrase > /dev/null
done

# version <name> <number> [flatpak.sh options]: a Flatpak whose program prints <number>.
version() {
    local name=$1 number=$2
    shift 2
    printf '#!/bin/sh\necho %s\n' "$number" > "$tmp/program-$name"
    chmod 755 "$tmp/program-$name"
    "$repo/tools/beta/flatpak.sh" "$tmp/none" "$tmp/$name" --cli "$tmp/program-$name" "$@" > /dev/null
    # Commits are timed to the second: the next one must come later.
    sleep 1.1
}
version v1 1 --gpg-homedir "$tmp/keys/good"
version v2-unsigned 2 --unsigned
version v2-other 2 --gpg-homedir "$tmp/keys/other"
version v2 2 --gpg-homedir "$tmp/keys/good"
version v3 3 --gpg-homedir "$tmp/keys/good"

# The version installed: the one of the repositories made above whose commit is deployed.
installed() {
    local commit name
    commit=$(flatpak info --user --show-commit "$APP_ID" 2> /dev/null) || { echo none; return; }
    for name in v1 v2 v3 v2-unsigned v2-other; do
        if [[ $(cat "$tmp/$name/repo/refs/heads/app/$APP_ID/"*/beta) == "$commit" ]]; then
            echo "${name#v}"
            return
        fi
    done
    echo "unknown commit $commit"
}

serve() {
    stop
    python3 -m http.server --bind 127.0.0.1 --directory "$tmp/$1/repo" "$PORT" \
        > "$tmp/server.log" 2>&1 &
    server=$!
    for _ in $(seq 50); do
        python3 -c "import socket; socket.create_connection(('127.0.0.1', $PORT))" 2> /dev/null &&
            return
        sleep 0.1
    done
    fail "the server of $1 did not start: $(cat "$tmp/server.log")"
}

stop() {
    if [[ -n $server ]]; then
        kill "$server" 2> /dev/null || true
        wait "$server" 2> /dev/null || true
        server=
    fi
}

update() {
    flatpak update --user --noninteractive -y "$APP_ID" > "$tmp/update.log" 2>&1 || true
}

flatpak install --user --noninteractive -y --bundle "$tmp/v1/Fogata.flatpak" > "$tmp/install.log" 2>&1 ||
    { cat "$tmp/install.log" >&2; exit 1; }
[[ $(installed) == 1 ]] || fail "the first install is not version 1: $(installed)"

stop
update
[[ $(installed) == 1 ]] || fail "with nothing on the port, version $(installed) came: $(cat "$tmp/update.log")"

serve v2-unsigned
update
[[ $(installed) == 1 ]] || fail "an unsigned commit was installed: $(cat "$tmp/update.log")"

serve v2-other
update
[[ $(installed) == 1 ]] || fail "a commit signed by another key was installed: $(cat "$tmp/update.log")"

serve v3
update
[[ $(installed) == 3 ]] ||
    fail "a newer commit signed by the key was not installed: $(installed), $(cat "$tmp/update.log")"

serve v2
update
[[ $(installed) == 3 ]] ||
    fail "an older commit signed by the key replaced a newer one: $(installed), $(cat "$tmp/update.log")"
stop

if [[ $failed == 0 ]]; then
    echo "test-flatpak-origin: all passed"
fi
exit $failed
