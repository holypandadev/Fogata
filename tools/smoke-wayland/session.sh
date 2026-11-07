#!/usr/bin/env bash
# Runs a command in a Wayland desktop of its own, built the way a KDE Plasma session is but
# where nothing reaches the computer's own screen, sound, devices or desktop session. An
# unprivileged user, mount and PID namespace (as tools/call-lab/session.sh):
# - replaces /dev with a folder holding only null, zero, full, random, urandom and tty: no
#   sound, video or GPU device at all (rendering is Mesa's, in software);
# - hides the user's runtime folder (their Wayland, PipeWire, PulseAudio and session bus
#   sockets), the system bus, the system's PulseAudio folders and the X sockets' folder, and
#   gives the session a home folder of its own (its KDE settings and portal permissions);
# - starts there a session bus of its own (dbus-daemon, with only the system's service
#   files, so what it starts on demand also runs here), PipeWire, WirePlumber and
#   pipewire-pulse, and KWin with a virtual output (`kwin_wayland --virtual`) and Xwayland;
#   the desktop portal (xdg-desktop-portal with its KDE backend) starts on demand, as in a
#   Plasma session;
# - answers the portal's screen-sharing dialogs as a person would, through the
#   accessibility bus (tools/smoke-wayland/picker.py), with what $SMOKE_PICKER/answer says
#   ("share", the default, or "cancel");
# - ends every process started inside when it ends (its first process is the namespace's
#   init). The network is the computer's own.
#
# Usage: tools/smoke-wayland/session.sh <folder> <command> [args...]
#   <folder> keeps the session's home folder and the daemons' logs. The command runs with
#   WAYLAND_DISPLAY, DISPLAY (Xwayland), XDG_SESSION_TYPE=wayland, XDG_CURRENT_DESKTOP=KDE
#   and DBUS_SESSION_BUS_ADDRESS of this session, as an application started from Plasma.
# Exits 3, naming it, when the session cannot be built on this computer (a program missing,
# no unprivileged namespaces, the portal offering no screen sharing), else the command's own
# status.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
if [ "${SMOKE_SESSION_INSIDE:-}" != 1 ]; then
  [ $# -ge 2 ] || { echo "usage: $0 <folder> <command> [args...]" >&2; exit 2; }
  missing=()
  for tool in unshare dbus-daemon pipewire wireplumber pipewire-pulse pactl kwin_wayland \
    Xwayland python3; do
    command -v "$tool" > /dev/null || missing+=("$tool")
  done
  for file in /usr/libexec/xdg-desktop-portal /usr/libexec/at-spi-bus-launcher; do
    [ -x "$file" ] || missing+=("$file")
  done
  kde=
  for file in /usr/lib/*/libexec/xdg-desktop-portal-kde /usr/libexec/xdg-desktop-portal-kde; do
    [ -x "$file" ] && kde=$file
  done
  [ -n "$kde" ] || missing+=(xdg-desktop-portal-kde)
  python3 -c 'import dbus, gi; gi.require_version("Atspi", "2.0"); from gi.repository import Atspi' \
    > /dev/null 2>&1 || missing+=("python3 dbus and Atspi")
  if [ ${#missing[@]} -gt 0 ]; then
    echo "smoke session: missing ${missing[*]}" >&2
    exit 3
  fi
  unshare --user --map-root-user --mount true 2> /dev/null ||
    { echo "smoke session: unprivileged user namespaces are not allowed here" >&2; exit 3; }
  mkdir -p "$1"
  exec env SMOKE_SESSION_INSIDE=1 SMOKE_SESSION_RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" \
    unshare --user --map-root-user --mount --pid --fork --mount-proc "$0" "$@"
fi

DIR=$(cd "$1" && pwd); shift
RUNTIME=$SMOKE_SESSION_RUNTIME
fail() { echo "smoke session: $*" >&2; exit 3; }

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
# KWin shares a screen only when it composites with OpenGL, and on a virtual output it does
# that only on a GPU's render node. So the GPU's render nodes come in (and, for NVIDIA's
# driver, the nodes it renders through), never a card node: a render node draws into
# memory only, it cannot show anything on a screen. SMOKE_GPU=0 leaves the GPU out (KWin
# then cannot share the screen, and the session says so).
gpu=0
if [ "${SMOKE_GPU:-1}" != 0 ]; then
  mkdir -p /dev/dri
  for real in "$HOSTDEV"/dri/renderD*; do
    [ -c "$real" ] || continue
    name=/dev/dri/${real##*/}
    touch "$name" && mount --bind "$real" "$name" || fail "cannot give $name"
    gpu=1
  done
  if [ $gpu = 1 ]; then
    for node in nvidiactl nvidia0 nvidia-uvm nvidia-modeset; do
      [ -c "$HOSTDEV/$node" ] || continue
      touch "/dev/$node" && mount --bind "$HOSTDEV/$node" "/dev/$node" || fail "cannot give /dev/$node"
    done
  fi
fi
umount -l "$HOSTDEV" || fail "cannot let the real /dev go"
for dev in /dev/video* /dev/media* /dev/dri/card* /dev/snd; do
  [ -e "$dev" ] && fail "$dev still there"
done

# Nothing of the user's own session: their runtime folder (Wayland, PipeWire, PulseAudio and
# session bus sockets), the system bus and PulseAudio, and the X sockets (Xwayland makes its
# own here).
mkdir -p "$RUNTIME"
mount -t tmpfs -o mode=700 none "$RUNTIME" || fail "cannot hide $RUNTIME"
for system in /run/pulse /var/run/pulse /run/dbus /tmp/.X11-unix /tmp/.ICE-unix; do
  [ -d "$system" ] && { mount -t tmpfs -o mode=1777 none "$system" || fail "cannot hide $system"; }
done
export HOME=$DIR/home
export XDG_RUNTIME_DIR=$RUNTIME XDG_CONFIG_HOME=$HOME/.config XDG_DATA_HOME=$HOME/.local/share \
  XDG_STATE_HOME=$HOME/.local/state XDG_CACHE_HOME=$HOME/.cache
mkdir -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_STATE_HOME" "$XDG_CACHE_HOME"
export DBUS_SYSTEM_BUS_ADDRESS=disabled:
unset PIPEWIRE_REMOTE WAYLAND_DISPLAY DISPLAY XAUTHORITY SESSION_MANAGER DESKTOP_STARTUP_ID \
  XDG_ACTIVATION_TOKEN GDK_BACKEND QT_QPA_PLATFORM XDG_SESSION_ID
# Without a GPU, only Mesa's software EGL (a GPU vendor's, such as NVIDIA's, ends when it
# finds no device).
if [ $gpu = 0 ]; then
  mesa=/usr/share/glvnd/egl_vendor.d/50_mesa.json
  [ -e "$mesa" ] && export __EGL_VENDOR_LIBRARY_FILENAMES=$mesa
  export LIBGL_ALWAYS_SOFTWARE=1
fi
: > "$DIR/client.conf"
export PULSE_SERVER=unix:$RUNTIME/pulse/native PULSE_CLIENTCONFIG=$DIR/client.conf
# As a Plasma session sets them for what it starts.
export XDG_SESSION_TYPE=wayland XDG_CURRENT_DESKTOP=KDE XDG_SESSION_DESKTOP=KDE \
  KDE_FULL_SESSION=true KDE_SESSION_VERSION=6 DESKTOP_SESSION=plasma XDG_MENU_PREFIX=plasma- \
  XDG_DATA_DIRS=/usr/share/plasma:/usr/local/share:/usr/share \
  XDG_CONFIG_DIRS=/etc/xdg/xdg-plasma:/etc/xdg
# Qt applications (the portal's dialogs) tell the accessibility bus what they show.
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 NO_AT_BRIDGE=0
# KDE's own settings, so that no first-run wizard or notification server is asked for.
cat > "$XDG_CONFIG_HOME/kwinrc" << 'EOF'
[Wayland]
InputMethod=
[Plugins]
screencastEnabled=true
EOF

pids=()
stop() {
  local i
  for ((i = ${#pids[@]} - 1; i >= 0; i--)); do
    kill "${pids[i]}" 2> /dev/null
  done
  sleep 0.5
}
trap 'stop; exit 1' INT TERM HUP
trap stop EXIT

# The session bus, with the system's service files only (and none of the user's).
cat > "$DIR/bus.conf" << EOF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <keep_umask/>
  <listen>unix:path=$RUNTIME/bus</listen>
  <auth>EXTERNAL</auth>
  <servicedir>/usr/share/dbus-1/services</servicedir>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
  <limit name="reply_timeout">300000</limit>
</busconfig>
EOF
export DBUS_SESSION_BUS_ADDRESS=unix:path=$RUNTIME/bus
dbus-daemon --config-file="$DIR/bus.conf" --nofork --nosyslog > "$DIR/dbus.log" 2>&1 &
pids+=($!)
for _ in $(seq 50); do [ -S "$RUNTIME/bus" ] && break; sleep 0.1; done
[ -S "$RUNTIME/bus" ] || fail "the session bus did not start (see $DIR/dbus.log)"

pipewire > "$DIR/pipewire.log" 2>&1 &
pids+=($!)
for _ in $(seq 50); do [ -S "$RUNTIME/pipewire-0" ] && break; sleep 0.1; done
wireplumber > "$DIR/wireplumber.log" 2>&1 &
pids+=($!)
pipewire-pulse > "$DIR/pipewire-pulse.log" 2>&1 &
pids+=($!)
for _ in $(seq 100); do pactl info > /dev/null 2>&1 && break; sleep 0.1; done
server=$(pactl info 2> /dev/null | sed -n 's/^Server String: //p')
[ "$server" = "$PULSE_SERVER" ] || fail "pactl reaches \"$server\", not the session's own server"

# KDE's index of the installed applications, which KWin reads to tell which programs may
# use its restricted Wayland interfaces (the portal's screen casting among them), as a
# Plasma session builds it at login.
kbuildsycoca6 --noincremental > "$DIR/sycoca.log" 2>&1 ||
  fail "cannot index the applications (see $DIR/sycoca.log)"

# KWin, on a virtual output, with Xwayland, as the desktop.
KWIN_WAYLAND_NO_PERMISSION_CHECKS=${SMOKE_NO_PERMISSION_CHECKS:-0} kwin_wayland --virtual --xwayland --socket wayland-smoke --width "${SMOKE_WIDTH:-1600}" \
  --height "${SMOKE_HEIGHT:-900}" --no-lockscreen --no-global-shortcuts \
  > "$DIR/kwin.log" 2>&1 &
pids+=($!)
for _ in $(seq 200); do [ -S "$RUNTIME/wayland-smoke" ] && break; sleep 0.1; done
[ -S "$RUNTIME/wayland-smoke" ] || fail "KWin did not start (see $DIR/kwin.log)"
export WAYLAND_DISPLAY=wayland-smoke
# Xwayland's display, which KWin announces on the session bus once it is up.
display=
for _ in $(seq 200); do
  for socket in /tmp/.X11-unix/X*; do
    [ -S "$socket" ] && display=:${socket##*/X}
  done
  [ -n "$display" ] && break
  sleep 0.1
done
[ -n "$display" ] || fail "Xwayland did not start (see $DIR/kwin.log)"
export DISPLAY=$display
if [ $gpu = 0 ]; then
  dbus-update-activation-environment __EGL_VENDOR_LIBRARY_FILENAMES LIBGL_ALWAYS_SOFTWARE \
    2> /dev/null
fi
# What the bus starts on demand (the portal and its KDE backend) sees this session.
dbus-update-activation-environment DBUS_SESSION_BUS_ADDRESS WAYLAND_DISPLAY DISPLAY XDG_SESSION_TYPE \
  XDG_CURRENT_DESKTOP XDG_SESSION_DESKTOP KDE_FULL_SESSION KDE_SESSION_VERSION \
  DESKTOP_SESSION XDG_MENU_PREFIX XDG_DATA_DIRS XDG_CONFIG_DIRS QT_LINUX_ACCESSIBILITY_ALWAYS_ON QT_ACCESSIBILITY HOME XDG_CONFIG_HOME \
  XDG_DATA_HOME XDG_STATE_HOME XDG_CACHE_HOME XDG_RUNTIME_DIR PULSE_SERVER \
  PULSE_CLIENTCONFIG \
  DBUS_SYSTEM_BUS_ADDRESS 2> "$DIR/activation.log" ||
  fail "cannot pass the session to the bus (see $DIR/activation.log)"

# The person in front of the portal's dialogs.
export SMOKE_PICKER=${SMOKE_PICKER:-$DIR/picker}
mkdir -p "$SMOKE_PICKER"
python3 -I "$HERE/picker.py" "$SMOKE_PICKER" > "$DIR/picker.log" 2>&1 &
pids+=($!)

# The portal offers screen sharing here, through KDE's dialog, before anything runs.
if [ "${SMOKE_SESSION_CHECK:-1}" = 1 ] && ! python3 -I "$HERE/portal_check.py" > "$DIR/portal-check.log" 2>&1; then
  cat "$DIR/portal-check.log" >&2
  fail "the desktop portal shares no screen here (see $DIR/portal-check.log)"
fi

"$@"
