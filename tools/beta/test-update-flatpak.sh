#!/usr/bin/env bash
# The update's install step on Linux, with the published files and nothing else: installs an
# earlier pre-release's Flatpak as a person does (the file from the releases page, checked
# against the signed list as the README says, then `flatpak install --user`), starts it,
# makes a profile in it by typing a name, lets that copy find the newest release on the
# releases page by itself and fetch it, starts it again as a person opens it the next time,
# when it serves the update to flatpak and asks the Flatpak portal to install it, and checks
# what is left: the newer build installed, and running (the old copy starts the new one
# through the portal).
#
#   tools/beta/test-update-flatpak.sh <from tag> <to tag> [--out <folder>] [--minutes <n>]
#
#   test-update-flatpak.sh v0.1.0-beta.9 v0.1.0-beta.11   beta.9 must update itself to beta.11
#   test-update-flatpak.sh v0.1.0-beta.11 v0.1.0-beta.11  the first install alone: it starts
#                                                         and stays up
#
# <to tag> is the newest release on the page (an earlier one is never taken). The portal asks
# the person once whether Fogata may update itself; the test answers that ahead, in flatpak's
# own permission store, so the system's question is the one thing of the step it does not go
# through. The copy is ended, not closed, before its second start: as when the computer is
# turned off.
# Meant for a computer of its own (GitHub's, a virtual machine), in a desktop session nobody
# is typing at, with a display of 1280x800 the window fills (a display, and a session bus
# whose services know it, as the Flatpak portal starts the new copy;
# `xvfb-run -a -s "-screen 0 1280x800x24" dbus-run-session --` make them): it installs Fogata
# for this user, starts it, and clicks and types into its window. It refuses to run where
# Fogata's Flatpak is installed or has data already. Needs flatpak, curl, python3, ssh-keygen,
# xdotool and the org.freedesktop.Platform 26.08 runtime.
# Exits 0 when the update installed, 1 when it did not, 3 when it cannot run here.
set -euo pipefail
APP_ID=dev.holypanda.fogata
REPO=${FOGATA_UPDATE_TEST_REPO:-holypandadev/Fogata}
from=${1:-} to=${2:-}
[[ -n $from && -n $to ]] || { sed -n '2,29p' "$0" >&2; exit 3; }
shift 2
out='' minutes=20
while [[ $# -gt 0 ]]; do
    case $1 in
        --out) out=$2; shift 2 ;;
        --minutes) minutes=$2; shift 2 ;;
        *) echo "test-update-flatpak: unknown option $1" >&2; exit 3 ;;
    esac
done
cannot() { echo "test-update-flatpak: $*" >&2; exit 3; }
[[ $(uname) == Linux ]] || cannot "Linux only"
for tool in flatpak curl python3 ssh-keygen xdotool; do
    command -v "$tool" > /dev/null || cannot "no $tool"
done
[[ -n ${DISPLAY:-}${WAYLAND_DISPLAY:-} ]] || cannot "no display: run it in a desktop session"
[[ -n ${DBUS_SESSION_BUS_ADDRESS:-} ]] || cannot "no session bus: run it in a desktop session"
flatpak info org.freedesktop.Platform//26.08 > /dev/null 2>&1 ||
    cannot "no org.freedesktop.Platform 26.08 runtime"
! flatpak info "$APP_ID" > /dev/null 2>&1 || cannot "$APP_ID is installed: this test is for a computer without Fogata"
home_data=$HOME/.var/app/$APP_ID
[[ ! -e $home_data ]] || cannot "$home_data exists: this test is for a computer without Fogata"
data=$home_data/data/fogata

[[ -n $out ]] || out=$(mktemp -d "${TMPDIR:-/tmp}/fogata-update.XXXXXX")
mkdir -p "$out"

repo=$(cd "$(dirname "$0")/../.." && pwd)

say() { echo "[$(date +%H:%M:%S)] $*"; }

fetch() { # <tag> <file name> <destination>
    curl --silent --show-error --fail --location --retry 3 \
        --output "$3" "https://github.com/$REPO/releases/download/$1/$2"
}

# The build the installed copy was made as: the marker fogata-update compiles in, which
# tools/beta/release.py looks for too.
build_of() {
    local files
    files=$(flatpak info --user --show-location "$APP_ID" 2> /dev/null)/files
    [[ -d $files ]] || { echo none; return; }
    grep --recursive --no-filename --text --only-matching 'fogata-build-marker:[0-9]*;' "$files" 2> /dev/null |
        head -n 1 | tr -dc 0-9 || true
}

# How many copies of the application run.
running() {
    flatpak ps --columns=application 2> /dev/null | grep -c "^$APP_ID\$" || true
}

shots=0
screen() {
    shots=$((shots + 1))
    if command -v import > /dev/null && [[ -n ${DISPLAY:-} ]]; then
        import -window root "$out/screen-$(printf %02d "$shots").png" 2> /dev/null || true
    fi
}

app=
finish() {
    local code=$?
    screen
    flatpak kill "$APP_ID" 2> /dev/null || true
    [[ -z $app ]] || { kill "$app" 2> /dev/null || true; wait "$app" 2> /dev/null || true; }
    # What the copies wrote for problem reports (nothing personal reaches these files).
    if [[ -d $data/diagnostics ]]; then
        cp -r "$data/diagnostics" "$out/diagnostics"
        for log in "$data"/diagnostics/*.log; do
            [[ -f $log ]] || continue
            echo "--- ${log##*/} (its last 80 lines)"
            tail -n 80 "$log"
        done
    fi
    if [[ -s $out/app.log ]]; then
        echo "--- what the application printed (its last 40 lines)"
        tail -n 40 "$out/app.log"
    fi
    exit "$code"
}
trap finish EXIT
fail() { say "FAIL: $*"; exit 1; }

fetch "$to" release.json "$out/release.json" || fail "cannot fetch release.json of $to"
read -r wanted wanted_version min_build < <(python3 -I -c '
import json, sys
m = json.load(open(sys.argv[1]))
print(int(m["build"]), m["version"], int(m["min_build"]))' "$out/release.json")
say "$to is build $wanted ($wanted_version), which asks for build $min_build at least"

# The earlier release, checked as its README says: the list of downloads signed by a
# pre-release key of tools/allowed_signers, and the file as that list says.
version=${from#v}
name=Fogata-$version-linux.flatpak
for file in "$name" SHA256SUMS SHA256SUMS.sig; do
    fetch "$from" "$file" "$out/$file" || fail "cannot fetch $file of $from"
done
ssh-keygen -Y verify -f "$repo/tools/allowed_signers" -I fogata-prerelease -n file \
    -s "$out/SHA256SUMS.sig" < "$out/SHA256SUMS" ||
    fail "SHA256SUMS of $from is not signed by a pre-release key"
(cd "$out" && grep -F "  $name" SHA256SUMS | sha256sum --check --status) ||
    fail "$name is not the file SHA256SUMS lists"
say "installing the Flatpak of $from"
flatpak install --user --noninteractive -y --bundle "$out/$name" \
    > "$out/install.log" 2>&1 || { cat "$out/install.log"; fail "flatpak could not install it"; }
first=$(build_of)
say "installed: build $first, commit $(flatpak info --user --show-commit "$APP_ID")"
if [[ $from == "$to" ]]; then
    [[ $first == "$wanted" ]] || fail "the Flatpak of $from holds build $first, not $wanted"
else
    [[ $first =~ ^[0-9]+$ && $first -lt $wanted ]] ||
        fail "the copy installed is build $first, not one before $wanted"
    # The portal's question to the person, answered ahead.
    flatpak permission-set flatpak updates "$APP_ID" yes ||
        fail "cannot answer the portal's question ahead (flatpak permission-set)"
fi

say "starting it"
flatpak run "$APP_ID" > "$out/app.log" 2>&1 &
app=$!

if [[ $from != "$to" ]]; then
    # A profile, as a person makes one: the name typed into the welcome page, then Enter. A
    # copy without one asks nobody for updates. Where no keychain runs (none does in a
    # session made for this test) the welcome page asks first whether to go on without one:
    # the click is on that dialog's "Continue unencrypted", where the page draws it on a
    # 1280x800 display, and then on the name's field.
    end=$((SECONDS + 300))
    # (friends.bin holds it: the device's key alone is made as soon as a keychain keeps it.)
    while [[ ! -f $data/friends.bin ]]; do
        [[ $SECONDS -lt $end ]] || fail "no profile was made within five minutes"
        sleep 5
        [[ $(running) -gt 0 ]] || fail "the copy ended by itself before a profile was made"
        screen
        xdotool mousemove 586 520 click 1
        sleep 2
        xdotool mousemove 900 467 click 1
        sleep 1
        xdotool key ctrl+a
        xdotool type Tester
        xdotool key Return
        sleep 3
    done
    say "the profile is made"
    screen

    # It finds the release by itself and fetches it.
    end=$((SECONDS + minutes * 60))
    last=''
    while [[ ! -f $data/updates/linux-flatpak.pack ]]; do
        [[ $SECONDS -lt $end ]] || fail "after $minutes minutes the update was not fetched"
        sleep 5
        [[ $(running) -gt 0 ]] || fail "the copy ended by itself before the update came"
        state="updates: $(cd "$data/updates" 2> /dev/null && find . -maxdepth 1 -type f -printf '%f %kK; ' || true)"
        state=${state//[0-9][0-9][0-9][0-9]K/...K}
        if [[ $state != "$last" ]]; then
            say "$state"
            last=$state
        fi
    done
    say "the update is fetched and checked"
    sleep 5
    screen

    # The next start installs it.
    flatpak kill "$APP_ID" 2> /dev/null || true
    wait "$app" 2> /dev/null || true
    for _ in $(seq 30); do
        [[ $(running) -gt 0 ]] || break
        sleep 1
    done
    say "starting it again"
    flatpak run "$APP_ID" >> "$out/app.log" 2>&1 &
    app=$!
    end=$((SECONDS + 600))
    last='' took=0
    while [[ $SECONDS -lt $end ]]; do
        sleep 5
        build=$(build_of)
        state="build $build, $(running) running"
        if [[ $state != "$last" ]]; then
            say "$state"
            last=$state
            screen
        fi
        if [[ $build == "$wanted" && $(running) -gt 0 ]]; then
            took=1
            break
        fi
    done
    [[ $took == 1 ]] ||
        fail "ten minutes after its second start the installed copy is build $(build_of), not $wanted"
    say "build $wanted is installed; watching the copy it started for a minute"
else
    say "watching it for a minute"
fi

sleep 60
screen
[[ $(running) -gt 0 ]] || fail "the copy of build $wanted ended by itself within a minute"
[[ $(build_of) == "$wanted" ]] || fail "the installed copy is build $(build_of), not $wanted"
if [[ $from != "$to" ]]; then
    [[ ! -f $data/updates/installed.json ]] || say "installed.json: $(cat "$data/updates/installed.json")"
    say "PASS: the Flatpak of $from updated itself to $to (build $first to build $wanted)"
else
    say "PASS: the Flatpak of $from installs, starts and stays up (build $wanted)"
fi
