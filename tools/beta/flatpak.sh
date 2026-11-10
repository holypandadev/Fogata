#!/usr/bin/env bash
# Builds the Linux Flatpak of Fogata from a release bundle of the Flutter app, with only the
# flatpak command and the org.freedesktop.Platform runtime (no flatpak-builder, no SDK): the
# bundle is copied into /app as built.
#
# Usage: tools/beta/flatpak.sh <bundle> <work> [--gpg-homedir <dir>] [--extra-libs <dir>]
#                               [--cli <fogata>] [--unsigned]
#   bundle      app/build/linux/x64/release/bundle
#   work        a folder to write, made new; it gets
#                 repo/           an OSTree repository holding this version alone, its
#                                 commit and summary signed by the Flatpak signing key:
#                                 packed (`fogata release pack`), it is the update's package
#                 Fogata.flatpak  the first install, which carries the signing key's public
#                                 half and names as its origin the loopback address the
#                                 application serves its updates on while they install
#   --gpg-homedir the GnuPG home holding the Flatpak signing key (tools/beta/flatpak-key.sh
#                 makes it); by default ~/.config/fogata-flatpak-key/gnupg. Never one
#                 inside the folder of a release or rescue key, which is refused.
#   --extra-libs  libraries to put in /app/lib, should the runtime lack one (none today:
#                 the tray serves its icon over D-Bus itself, with no indicator library)
#   --cli         a Flatpak of the command-line program instead of the application, with
#                 the same id: only for tests of the update path
#   --unsigned    no signing key: a Flatpak whose origin flatpak never verifies, which an
#                 installed copy must never get (tests of the lab only)
#   --test-bundle  <bundle> may be a debug build (an integration test built with
#                 `flutter build linux --debug -t ...`), for tests of the lab only, with
#                 --unsigned (tools/beta/test-flatpak-camera.sh)
#
# What the signature does, and what it does not (tools/beta/test-flatpak-origin.sh tries it):
# flatpak (the portal, `flatpak update`, a software centre) takes from the loopback origin
# only a summary and a commit this key signed, and no commit older than the one installed (it
# compares their times). So another program answering on the port while Fogata does not
# serve there (another account, a Flatpak with the network) can offer only a version this
# key signed and newer than the one installed. But flatpak knows nothing of the release
# manifest or its build rules: whatever this key signs installs, and a summary it signs can
# make flatpak trust more keys (xa.gpg-keys) or fetch from another address (xa.redirect-url)
# for good. On Linux this key can therefore make copies run any code, as the release key
# can, and no release takes that back. It is kept apart from the release keys
# (tools/beta/flatpak-key.sh). It must stay the same from one release to the next (a copy
# installed with one key does not take releases signed with another), as must the port
# (fogata-core's FLATPAK_REPO_PORT) and the branch.
set -euo pipefail
APP_ID=dev.holypanda.fogata
BRANCH=beta
RUNTIME=org.freedesktop.Platform
RUNTIME_VERSION=26.08
PORT=47811
repo_root=$(cd "$(dirname "$0")/../.." && pwd)
usage="usage: tools/beta/flatpak.sh <bundle> <work> [--gpg-homedir <dir>] [--extra-libs <dir>] [--cli <fogata>] [--unsigned] [--test-bundle]"

bundle=${1:?$usage}
work=${2:?$usage}
shift 2
extra_libs= cli= unsigned= test_bundle= gpg_homedir=${FOGATA_GPG_HOMEDIR:-$HOME/.config/fogata-flatpak-key/gnupg}
while [[ $# -gt 0 ]]; do
    case $1 in
        --gpg-homedir) gpg_homedir=$2; shift 2 ;;
        --extra-libs) extra_libs=$2; shift 2 ;;
        --cli) cli=$2; shift 2 ;;
        --unsigned) unsigned=1; shift ;;
        --test-bundle) test_bundle=1; shift ;;
        *) echo "unknown option $1" >&2; exit 2 ;;
    esac
done

fail() {
    echo "flatpak.sh: $*" >&2
    exit 1
}
[[ -z $test_bundle || -n $unsigned ]] || fail "--test-bundle goes with --unsigned only"

# The signing key: the one secret key in the GnuPG home.
# The Flatpak signing key is kept apart from the release and rescue keys (fogata release
# keygen), so that whoever takes one does not have the other: a GnuPG home inside a folder
# that holds one of them is refused.
beside_release_key() {
    local dir=$1
    [[ -d $dir ]] && dir=$(cd "$dir" && pwd -P)
    while :; do
        for file in release-key release-key.pub rescue-key rescue-key.pub; do
            [[ -e $dir/$file ]] && return 0
        done
        [[ $dir == / || $dir == . || -z $dir ]] && return 1
        dir=$(dirname "$dir")
    done
}
sign=()
if [[ -z $unsigned ]]; then
    beside_release_key "$gpg_homedir" &&
        fail "$gpg_homedir is inside a folder that holds a release or rescue key: move the Flatpak signing key to a folder of its own (docs/releasing-beta.md)"
    [[ -d $gpg_homedir ]] || fail "no GnuPG home at $gpg_homedir: make the Flatpak signing key with tools/beta/flatpak-key.sh"
    fingerprints=$(gpg --homedir "$gpg_homedir" --batch --list-secret-keys --with-colons 2>/dev/null |
        awk -F: '$1 == "fpr" {print $10; exit}')
    [[ -n $fingerprints ]] || fail "no secret key in $gpg_homedir: make it with tools/beta/flatpak-key.sh"
    sign=(--gpg-sign="$fingerprints" --gpg-homedir="$gpg_homedir")
fi

[[ ! -e $work ]] || fail "$work is there already"
if [[ -z $cli ]]; then
    [[ -x $bundle/fogata && -d $bundle/lib && -d $bundle/data ]] || fail "no release bundle in $bundle"
    [[ -n $test_bundle || ! -e $bundle/data/flutter_assets/kernel_blob.bin ]] ||
        fail "$bundle is a debug build"
fi
mkdir -p "$work"
build=$work/build

flatpak build-init "$build" "$APP_ID" "$RUNTIME" "$RUNTIME" "$RUNTIME_VERSION" > /dev/null
files=$build/files
mkdir -p "$files/bin" "$files/share/applications" "$files/share/icons/hicolor/256x256/apps" \
    "$files/share/icons/hicolor/512x512/apps"
if [[ -n $cli ]]; then
    install -m 755 "$cli" "$files/bin/fogata"
else
    cp -a "$bundle" "$files/fogata"
    printf '#!/bin/sh\nexec /app/fogata/fogata "$@"\n' > "$files/bin/fogata"
    chmod 755 "$files/bin/fogata"
fi
if [[ -n $extra_libs ]]; then
    mkdir -p "$files/lib"
    cp -a "$extra_libs"/. "$files/lib/"
fi
install -m 644 "$repo_root/app/linux/dev.holypanda.fogata.desktop" \
    "$files/share/applications/$APP_ID.desktop"
install -m 644 "$repo_root/app/linux/runner/resources/app_icon.png" \
    "$files/share/icons/hicolor/256x256/apps/$APP_ID.png"
install -m 644 "$repo_root/app/assets/images/app-icon.png" \
    "$files/share/icons/hicolor/512x512/apps/$APP_ID.png"

# Every library the application loads must be in the runtime or in /app.
if [[ -z $cli ]]; then
    missing=$(flatpak build "$build" sh -c \
        'ldd /app/fogata/fogata /app/fogata/lib/*.so 2>/dev/null | grep "not found" | sort -u' || true)
    [[ -z $missing ]] || fail "libraries the runtime lacks (give them with --extra-libs):
$missing"
fi

# What the application uses: the network (and the loopback port of its updates), its
# window (Wayland, or X11 where there is no Wayland), sound, the downloads folder, the tray,
# and how long the computer has gone unused (the
# compositor over the Wayland socket it already has, the X server's MIT-SCREEN-SAVER over the
# X11 one, GNOME's idle monitor). Screens are shared through the ScreenCast portal, autostart
# through the Background portal, updates through the Flatpak portal, and the data key is
# sealed with the secret the Secret portal gives this application alone (fogata-core's
# vault), so no --talk-name=org.freedesktop.secrets: that name gives every secret of the
# person's keychain. A key an earlier Flatpak kept there moves once that permission is
# granted for a start (the application says how, docs/releasing-beta.md).
#   no --device=all: cameras come through the Camera portal and PipeWire (libwebrtc's
#                 own path, which the plugin takes in a Flatpak: linux/camera_access.h in
#                 app/third_party/flutter_webrtc, tools/webrtc/patches/wrapper/0009). The
#                 portal asks the person once and opens a PipeWire connection that sees the
#                 cameras alone, so no /dev/video* is needed, nor the rest all gave (USB,
#                 hidraw, input, kvm...).
#   --device=dri  the GPU, which Flutter draws with (OpenGL) and the video player decodes
#                 with (VA-API); flatpak gives NVIDIA's own nodes with it too. Without it
#                 everything is drawn in software.
#   no --share=ipc: the host's System V shared memory is only for X11's MIT-SHM, which the
#                 window does without (Flutter draws with OpenGL).
#   no org.freedesktop.ScreenSaver: the name also offers Lock, SetActive and Inhibit, which
#                 flatpak cannot filter out, and the idle time comes without it (above). The
#                 Flatpak does not know when the screen is locked (GetActive), so a key
#                 pressed at the lock screen counts as someone there.
#   org.gnome.Mutter.IdleMonitor: GetIdletime on the Core monitor only (idle time on GNOME,
#                 which has no Wayland idle protocol).
#   xdg-run/pipewire-0: PipeWire's own socket, which --socket=pulseaudio does not give. The
#                 sound of a shared screen links itself there to the other applications'
#                 outputs, leaving out Fogata's own (else it takes all the default output
#                 plays, the call's voices included, and the others hear themselves), and
#                 the silent input WebRTC records from while the microphone is off and that
#                 sound goes out is a node made there (else the microphone opens). PipeWire
#                 knows the application for a Flatpak and lets it see and link the session's
#                 streams and devices, what the PulseAudio socket already lets it do.
# tools/beta/test_release.py (FlatpakPermissions) pins this list and the runtime: change it
# there too, on purpose.
flatpak build-finish "$build" \
    --command=fogata \
    --share=network \
    --socket=wayland \
    --socket=fallback-x11 \
    --device=dri \
    --socket=pulseaudio \
    --filesystem=xdg-run/pipewire-0 \
    --filesystem=xdg-download \
    --talk-name=org.kde.StatusNotifierWatcher \
    --talk-name=org.gnome.Mutter.IdleMonitor > /dev/null

flatpak build-export --no-update-summary "${sign[@]}" "$work/repo" "$build" "$BRANCH" > /dev/null
flatpak build-update-repo "${sign[@]}" "$work/repo" > /dev/null
keys=()
if [[ -z $unsigned ]]; then
    gpg --homedir "$gpg_homedir" --batch --export "$fingerprints" > "$work/signing-key.gpg"
    [[ -s $work/signing-key.gpg ]] || fail "cannot export the signing key's public half"
    [[ -f $work/repo/summary.sig ]] || fail "the repository's summary was not signed"
    keys=(--gpg-keys="$work/signing-key.gpg")
fi
flatpak build-bundle --repo-url="http://127.0.0.1:$PORT/" "${keys[@]}" "$work/repo" \
    "$work/Fogata.flatpak" "$APP_ID" "$BRANCH"
rm -rf "${build:?}"
if [[ -z $unsigned ]]; then
    echo "$work/repo: the update's repository, signed by $fingerprints; $work/Fogata.flatpak: the first install"
else
    echo "$work/repo: the update's repository, UNSIGNED; $work/Fogata.flatpak: a first install that verifies nothing (lab only)"
fi
