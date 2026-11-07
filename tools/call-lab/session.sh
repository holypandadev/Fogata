#!/usr/bin/env bash
# Runs a command in a sound session of its own, where no real microphone, camera or speaker
# can be reached. An unprivileged user, mount and PID namespace:
# - replaces /dev with a folder holding only null, zero, full, random, urandom and tty (taken
#   from the real one) and a fresh shm: no sound, video or GPU device at all;
# - hides the user's runtime folder (their PipeWire and PulseAudio sockets) and the system's
#   PulseAudio folders, and starts a PipeWire, WirePlumber and pipewire-pulse of its own there,
#   which only ever hold the virtual devices the command makes;
# - names that server's socket in PULSE_SERVER and gives libpulse an empty client.conf, so
#   neither a client.conf, nor the X display's root window, nor a TCP default can lead
#   anywhere else; it stops unless `pactl info` names exactly that socket;
# - ends every process started inside when it ends (its first process is the namespace's init).
# The network is the computer's own.
#
# Usage: tools/call-lab/session.sh <folder> <command> [args...]
#   <folder> keeps the session's configuration, state and the daemons' logs.
#   CALL_LAB_ALSA=1 puts stand-ins of the computer's ALSA devices in /dev/snd (each one
#   /dev/null under the real name), to check what the app does when they are listed but no
#   sound server answers; nothing can be recorded or played through them.
set -uo pipefail
if [ "${CALL_LAB_INSIDE:-}" != 1 ]; then
  [ $# -ge 2 ] || { echo "usage: $0 <folder> <command> [args...]" >&2; exit 2; }
  mkdir -p "$1"
  exec env CALL_LAB_INSIDE=1 CALL_LAB_RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" \
    unshare --user --map-root-user --mount --pid --fork --mount-proc "$0" "$@"
fi

DIR=$(cd "$1" && pwd); shift
RUNTIME=$CALL_LAB_RUNTIME
fail() { echo "session: $*" >&2; exit 1; }

# A /dev of its own. The real one stays reachable a moment at $DIR/.hostdev for the nodes
# taken from it, then goes.
HOSTDEV=$DIR/.hostdev
mkdir -p "$HOSTDEV"
mount --rbind /dev "$HOSTDEV" || fail "cannot reach /dev"
mount -t tmpfs -o mode=755 none /dev || fail "cannot replace /dev"
for node in null zero full random urandom tty; do
  touch "/dev/$node" && mount --bind "$HOSTDEV/$node" "/dev/$node" || fail "cannot give /dev/$node"
done
mkdir -p /dev/shm /dev/pts
mount -t tmpfs -o mode=1777 none /dev/shm || fail "cannot make /dev/shm"
ln -s /proc/self/fd /dev/fd
ln -s /proc/self/fd/0 /dev/stdin
ln -s /proc/self/fd/1 /dev/stdout
ln -s /proc/self/fd/2 /dev/stderr
if [ "${CALL_LAB_ALSA:-}" = 1 ] && [ -d "$HOSTDEV/snd" ]; then
  mkdir -p /dev/snd
  for real in "$HOSTDEV"/snd/*; do
    [ -c "$real" ] || continue
    name=/dev/snd/$(basename "$real")
    touch "$name" && mount --bind "$HOSTDEV/null" "$name" || fail "cannot stand in for $name"
  done
fi
umount -l "$HOSTDEV" || fail "cannot let the real /dev go"
for dev in /dev/video* /dev/media* /dev/dri; do
  [ -e "$dev" ] && fail "$dev still there"
done
for dev in /dev/snd/*; do
  [ -e "$dev" ] || continue
  [ "$(stat -c %t:%T "$dev")" = "$(stat -c %t:%T /dev/null)" ] || fail "$dev is a real sound device"
done

# No sound server but this one: the user's sockets and the system-wide PulseAudio's hidden.
mkdir -p "$RUNTIME"
mount -t tmpfs -o mode=700 none "$RUNTIME" || fail "cannot hide $RUNTIME"
for system in /run/pulse /var/run/pulse; do
  [ -d "$system" ] && { mount -t tmpfs -o mode=755 none "$system" || fail "cannot hide $system"; }
done
export XDG_RUNTIME_DIR=$RUNTIME XDG_CONFIG_HOME=$DIR/config XDG_STATE_HOME=$DIR/state
export DBUS_SESSION_BUS_ADDRESS=disabled: DBUS_SYSTEM_BUS_ADDRESS=disabled:
# No GPU device here: only Mesa's software EGL (a GPU vendor's, such as NVIDIA's, ends Xvfb
# when it finds no device).
mesa=/usr/share/glvnd/egl_vendor.d/50_mesa.json
[ -e "$mesa" ] && export __EGL_VENDOR_LIBRARY_FILENAMES=$mesa
export LIBGL_ALWAYS_SOFTWARE=1
unset PIPEWIRE_REMOTE WAYLAND_DISPLAY DISPLAY
mkdir -p "$XDG_CONFIG_HOME" "$XDG_STATE_HOME"
: > "$DIR/client.conf"
export PULSE_SERVER=unix:$RUNTIME/pulse/native PULSE_CLIENTCONFIG=$DIR/client.conf

stop() {
  [ -n "${PP:-}" ] && kill "$PP" 2>/dev/null
  [ -n "${WP:-}" ] && kill "$WP" 2>/dev/null
  sleep 0.3
  [ -n "${PW:-}" ] && kill "$PW" 2>/dev/null
}
trap 'stop; exit 1' INT TERM HUP
trap stop EXIT

pipewire > "$DIR/pipewire.log" 2>&1 &
PW=$!
for _ in $(seq 50); do [ -S "$RUNTIME/pipewire-0" ] && break; sleep 0.1; done
wireplumber > "$DIR/wireplumber.log" 2>&1 &
WP=$!
pipewire-pulse > "$DIR/pipewire-pulse.log" 2>&1 &
PP=$!
for _ in $(seq 100); do pactl info > /dev/null 2>&1 && break; sleep 0.1; done
server=$(pactl info 2>/dev/null | sed -n 's/^Server String: //p')
[ "$server" = "$PULSE_SERVER" ] || fail "pactl reaches \"$server\", not the session's own server"

"$@"
