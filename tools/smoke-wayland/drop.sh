#!/usr/bin/env bash
# Drags files onto Fogata running as a Flatpak, by a real drag on KDE Plasma Wayland, and
# says what it took. In a session of session.sh (KWin on a virtual output, its own session
# bus, the desktop and document portals, /dev/fuse for the latter), each copy given (a
# Flatpak of `flutter build linux --debug -t integration_test/drop_portal_test.dart`, made
# by tools/beta/flatpak.sh --unsigned --test-bundle, with the application's own permissions:
# of the files, the downloads folder alone) runs in turn, and a GTK window outside the
# sandbox (drag_source.c, which offers what a GTK file manager offers: the list of files
# and the FileTransfer portal's targets, or with --uri-only the list alone) is dragged onto
#    1 ~/Downloads/report.pdf          (the folder the Flatpak sees)
#    2 ~/Documents/plan.pdf            (one it does not)
#    3 ~/.ssh/id_ed25519
#    4 ~/Documents/.env                (a hidden name)
#  5-8 the key named as a web page can write it, which the source's GIO still reads as
#      the key: with a fragment, with a query, with another host, with
#      `localhost.localdomain`
#    9 ~/Documents/plan.pdf and ~/.ssh/id_ed25519 together
#   10 ~/Documents/plan.pdf            from a source that offers the list alone
#   11 ~/Downloads/report.pdf          from a source that offers the list alone
#   12 ~/Documents/nowhere.pdf, which is not there, from a source that offers the list
#      alone: the copy must say what it says of drop 10
#   13 ~/Documents/plan.pdf and then ~/Downloads/report.pdf from another window, two
#      drags in a row: the second is taken as itself
#   14 ~/.ssh/id_ed25519 from a source that offers the portal's targets alone, no list
#   15 ~/Documents/plan.pdf from a source slow to give its list, and while the copy waits
#      for it, ~/.ssh/id_ed25519 dragged from another window: the second drag must not be
#      taken under the first's judgement
#   16 ~/Documents/innocent.jpg        (a link to ~/.ssh/id_ed25519)
#   17 the document portal's path drop 16 brought, named by a source that offers the
#      list alone, as a page could name a file the portal gave Fogata before (when
#      drop 16 brought one)
# The copy's drop zone writes, for each drop, the paths the drop plugin handed over, the
# files it took (with the folder shown) and what it said (drop_portal_test.dart); the drag
# source's log says which targets the copy asked for, and `flatpak documents` which files
# the portal has given the copy after each drop (a refused file the portal was asked for
# stays given). <out>/summary.txt has one line a drop.
#
# Usage: tools/smoke-wayland/drop.sh <out> <copy>...
#   <copy>  <name>=<Fogata.flatpak>; a copy named "before" is judged against what the
#           application did before it took the FileTransfer portal, any other against what it
#           does now
# Needs what session.sh needs, flatpak with the org.freedesktop.Platform 26.08 runtime, the
# document portal (xdg-document-portal, 1.19 or later for GetHostPaths), /dev/fuse, a C
# compiler, GTK 3's headers, wayland-scanner and plasma-wayland-protocols' fake-input.xml.
# Exits 0 when every drop did what it should, 1 when one did not, 3 when it cannot run here.
set -uo pipefail
APP_ID=dev.holypanda.fogata
here=$(cd "$(dirname "$0")" && pwd)

# A KWin script, run once in the session's KWin (as still.sh's).
kwin() {
  python3 -I -c '
import os, sys, time, dbus
path = os.path.join(sys.argv[1], "kwin.js")
with open(path, "w") as f:
    f.write(sys.argv[2])
bus = dbus.SessionBus()
n = bus.get_object("org.kde.KWin", "/Scripting").loadScript(
    path, dbus_interface="org.kde.kwin.Scripting")
script = bus.get_object("org.kde.KWin", "/Scripting/Script%d" % n)
script.run(dbus_interface="org.kde.kwin.Script")
time.sleep(0.2)
script.stop(dbus_interface="org.kde.kwin.Script")' "$DROP_OUT" "$1"
}

# The drag sources on the left (the second under the first), the copy on the right, apart.
place() {
  kwin 'for (const w of workspace.windowList()) {
  if (!w.normalWindow) continue;
  if (w.caption.indexOf("drag source two") >= 0) {
    w.frameGeometry = {x: 20, y: 420, width: 400, height: 300};
  } else if (w.caption.indexOf("drag source") >= 0) {
    w.frameGeometry = {x: 20, y: 20, width: 400, height: 300};
  } else {
    w.frameGeometry = {x: 520, y: 20, width: 1000, height: 760};
  }
}'
}

if [[ ${DROP_INSIDE:-} == 1 ]]; then
  out=$DROP_OUT
  tools=$out/tools
  failed=0
  : > "$out/summary.txt"
  say() { echo "$*" | tee -a "$out/summary.txt"; }

  # The document portal, started as a Flatpak starts it, and what it offers.
  gdbus call --session --dest org.freedesktop.portal.Documents \
    --object-path /org/freedesktop/portal/documents \
    --method org.freedesktop.portal.Documents.GetMountPoint > "$out/portal.txt" 2>&1
  gdbus call --session --dest org.freedesktop.portal.Documents \
    --object-path /org/freedesktop/portal/documents \
    --method org.freedesktop.DBus.Properties.Get org.freedesktop.portal.Documents version \
    >> "$out/portal.txt" 2>&1
  grep -q "doc" "$out/portal.txt" || { cat "$out/portal.txt" >&2; echo "no document portal" >&2; exit 3; }
  say "document portal: $(tr '\n' ' ' < "$out/portal.txt")"

  # drag <copy folder> <n> [--uri-only] <file>...: one drop, until the copy wrote it down.
  drag() {
    local res=$1 n=$2
    shift 2
    "$tools/drag_source" "$@" > "$res/source-$n.log" 2>&1 &
    local source=$!
    for _ in $(seq 100); do
      grep -q shown "$res/source-$n.log" 2> /dev/null && break
      sleep 0.1
    done
    sleep 1
    place
    sleep 1
    : > "$res/drop-$n-go"
    "$tools/fake_pointer" to 220 170 wait 400 press wait 300 \
      glide 220 170 1000 400 40 25 wait 600 glide 1000 400 1010 410 5 60 wait 500 \
      release wait 500 >> "$res/pointer.log" 2>&1 || echo "drop $n: the pointer failed" >> "$res/pointer.log"
    for _ in $(seq 400); do
      [[ -s $res/drop-$n.txt ]] && grep -q '^end' "$res/drop-$n.txt" && break
      sleep 0.1
    done
    kill "$source" 2> /dev/null
    wait "$source" 2> /dev/null
    documents "$res" "$n"
  }

  # The documents the portal holds for the copy after drop n: where their files really are
  # (each file a drop exported to it), and whether flatpak could say.
  documents() {
    flatpak documents --columns=origin "$APP_ID" > "$1/documents-$2.txt" 2>&1
    echo $? > "$1/documents-$2.status"
  }

  # drag_twice <copy folder> <n> <first source's arguments> -- <second's>: two drags in a
  # row, from two windows, each released on the copy, the second as soon as the first is.
  drag_twice() {
    local res=$1 n=$2
    shift 2
    local firsts=()
    while [[ $# -gt 0 && $1 != -- ]]; do
      firsts+=("$1")
      shift
    done
    shift
    "$tools/drag_source" "${firsts[@]}" > "$res/source-$n.log" 2>&1 &
    local first=$!
    "$tools/drag_source" --second "$@" > "$res/source-$n-second.log" 2>&1 &
    local second=$!
    for _ in $(seq 100); do
      grep -qs shown "$res/source-$n.log" && grep -qs shown "$res/source-$n-second.log" && break
      sleep 0.1
    done
    sleep 1
    place
    sleep 1
    : > "$res/drop-$n-go"
    "$tools/fake_pointer" to 220 170 wait 400 press wait 300 \
      glide 220 170 1000 400 30 20 wait 300 release wait 100 \
      to 220 570 wait 200 press wait 200 glide 220 570 1000 420 20 15 wait 300 release \
      wait 500 >> "$res/pointer.log" 2>&1 || echo "drop $n: the pointer failed" >> "$res/pointer.log"
    for _ in $(seq 400); do
      [[ -s $res/drop-$n.txt ]] && grep -q '^end' "$res/drop-$n.txt" && break
      sleep 0.1
    done
    kill "$first" "$second" 2> /dev/null
    wait "$first" "$second" 2> /dev/null
    documents "$res" "$n"
  }

  # A drop the copy did not write down, or wrote down as nothing come, proves nothing.
  came() {
    local file=$1/drop-$2.txt
    [[ -s $file ]] && grep -q '^end' "$file" && ! grep -q '^raw none' "$file"
  }

  # took <copy folder> <n> <what> <name>: drop n took the file called <name> and said
  # nothing (two drags in a row: the first may be taken too, or given up for the second).
  took() {
    local res=$1 n=$2 what=$3 name=$4
    local file=$res/drop-$n.txt verdict=ok
    if ! came "$res" "$n" || ! grep -q "^waiting .*/$name"$'\t' "$file" ||
      grep -q '^toast ' "$file"; then
      verdict=WRONG
      failed=1
    fi
    say "$(basename "$res") $n $what: $verdict: took $(grep '^waiting ' "$file" 2> /dev/null | cut -f1 | cut -c9- | tr '\n' ' '); told $(grep -c '^toast ' "$file" 2> /dev/null)"
  }

  # exported <copy folder> <n> <what> <name> yes|no: whether a document of the file named
  # (its real path's end) is given to the copy after drop n, by `flatpak documents`, which
  # must have answered.
  exported() {
    local res=$1 n=$2 what=$3 name=$4 want=$5 got=no
    grep -q -- "$name\$" "$res/documents-$n.txt" 2> /dev/null && got=yes
    [[ $(cat "$res/documents-$n.status" 2> /dev/null) == 0 ]] || got="unknown (flatpak documents failed)"
    local verdict=ok
    if [[ $got != "$want" ]]; then
      verdict=WRONG
      failed=1
    fi
    say "$(basename "$res") $n $what: $verdict: a document of $name given to the copy: $got"
  }

  # unasked <copy folder> <n> <what>: neither source of drop n was asked for the portal's
  # target (the portal was not asked for its files).
  unasked() {
    local res=$1 n=$2 what=$3 verdict=ok
    if grep -qs 'asked for application/vnd.portal' "$res/source-$n.log" "$res/source-$n-second.log"; then
      verdict=WRONG
      failed=1
    fi
    say "$(basename "$res") $n $what: $verdict: the portal's target asked for: $([[ $verdict == ok ]] && echo no || echo yes)"
  }

  # judge <copy folder> <n> <what> <taken> <folder> <told> [<notice>]: what drop n should
  # have done: how many files the room took, the folder the first is shown in ("-": none),
  # how many notices it gave, and words the notice has (English's). A drop that did not
  # come is wrong; with <told> "none", it is what should be (the copy took no drop at all).
  judge() {
    local res=$1 n=$2 what=$3 taken=$4 folder=$5 told=$6 notice=${7:-}
    local file=$res/drop-$n.txt
    local got_taken got_told got_folder got_notice raw asked
    got_taken=$(grep -c '^waiting ' "$file" 2> /dev/null)
    got_told=$(grep -c '^toast ' "$file" 2> /dev/null)
    got_folder=$(grep -m1 '^waiting ' "$file" 2> /dev/null | cut -f2)
    got_notice=$(grep '^toast ' "$file" 2> /dev/null | cut -c7- | tr '\n' ' ')
    raw=$(grep '^raw ' "$file" 2> /dev/null | cut -c5- | tr '\n' ' ')
    asked=$(cat "$res/source-$n.log" "$res/source-$n-second.log" 2> /dev/null |
      grep -o 'asked for [^:,]*' | cut -c11- | tr '\n' ' ')
    local verdict=ok
    if [[ $told == none ]]; then
      if [[ ! -s $file ]] || ! grep -q '^raw none' "$file" || [[ ${got_told:-0} != 0 ]]; then
        verdict=WRONG
        failed=1
      fi
    elif ! came "$res" "$n" || [[ ${got_taken:-0} != "$taken" || ${got_told:-0} != "$told" ||
      ($folder != - && $got_folder != "$folder") ||
      (-n $notice && $got_notice != *"$notice"*) ]]; then
      verdict=WRONG
      failed=1
    fi
    say "$(basename "$res") $n $what: $verdict: took ${got_taken:-0} (folder ${got_folder:--}), told ${got_told:-0}${got_notice:+ ($got_notice)}; asked for: ${asked:-nothing}; handed over: ${raw:-nothing}"
  }

  for copy in $DROP_COPIES; do
    name=${copy%%=*}
    bundle=${copy#*=}
    res=$out/$name
    rm -rf "$res"
    mkdir -p "$res"
    flatpak install --user --noninteractive -y --reinstall --bundle "$bundle" > "$res/install.log" 2>&1 ||
      { cat "$res/install.log" >&2; exit 1; }
    flatpak run --user --filesystem="$res" --env=FOGATA_TEST_OUT="$res" \
      --env=LP_NUM_THREADS=2 "$APP_ID" > "$res/app.log" 2>&1 &
    app=$!
    for _ in $(seq 900); do
      [[ -e $res/drop-ready ]] && break
      kill -0 "$app" 2> /dev/null || break
      sleep 0.2
    done
    if [[ ! -e $res/drop-ready ]]; then
      say "$name: the copy never got ready (see $res/app.log)"
      tail -n 40 "$res/app.log" >&2
      failed=1
      continue
    fi
    place
    h=$HOME
    key=$h/.ssh/id_ed25519
    drag "$res" 1 "$h/Downloads/report.pdf"
    drag "$res" 2 "$h/Documents/plan.pdf"
    drag "$res" 3 "$key"
    drag "$res" 4 "$h/Documents/.env"
    drag "$res" 5 --raw "file://$key#x"
    drag "$res" 6 --raw "file://$key?x"
    drag "$res" 7 --raw "file://evil$key"
    drag "$res" 8 --raw "file://localhost.localdomain$key"
    drag "$res" 9 "$h/Documents/plan.pdf" "$key"
    drag "$res" 10 --uri-only "$h/Documents/plan.pdf"
    drag "$res" 11 --uri-only "$h/Downloads/report.pdf"
    drag "$res" 12 --uri-only "$h/Documents/nowhere.pdf"
    drag_twice "$res" 13 "$h/Documents/plan.pdf" -- "$h/Downloads/report.pdf"
    drag "$res" 14 --portal-only "$key"
    drag_twice "$res" 15 --slow "$h/Documents/plan.pdf" -- "$key"
    drag "$res" 16 "$h/Documents/innocent.jpg"
    earlier=$(grep -m1 '^raw /run/user/' "$res/drop-16.txt" 2> /dev/null | cut -c5-)
    if [[ -n $earlier ]]; then
      drag "$res" 17 --uri-only "$earlier"
    else
      # No drag: the copy writes down that nothing came.
      : > "$res/drop-17-go"
      for _ in $(seq 400); do
        grep -qs '^end' "$res/drop-17.txt" && break
        sleep 0.1
      done
    fi
    : > "$res/drop-end"
    for _ in $(seq 100); do
      kill -0 "$app" 2> /dev/null || break
      sleep 0.2
    done
    kill "$app" 2> /dev/null
    wait "$app" 2> /dev/null
    flatpak kill "$APP_ID" 2> /dev/null

    if [[ $name == before ]]; then
      # Before the portal: the list of files always comes first, and the Flatpak opens
      # only what is in the downloads folder.
      judge "$res" 1 downloads 1 "$h/Downloads" 0
      judge "$res" 2 documents 0 - 0
      judge "$res" 3 ssh 0 - 1
      judge "$res" 4 hidden 0 - 1
      for n in 5 6 7 8; do judge "$res" $n key-written-otherwise 0 - 0; done
      judge "$res" 9 documents-and-ssh 0 - 1
      judge "$res" 10 documents-list-only 0 - 0
      judge "$res" 11 downloads-list-only 1 "$h/Downloads" 0
      judge "$res" 12 nowhere-list-only 0 - 0
      took "$res" 13 two-in-a-row report.pdf
      # No list: nothing it takes.
      judge "$res" 14 portal-only - - none
      say "before 15 slow-then-key: (not judged) $(grep -v '^seen' "$res/drop-15.txt" 2> /dev/null | tr '\n' ' ')"
      judge "$res" 16 link-to-ssh 0 - 0
    else
      private="hidden or private" unreached="could not be reached"
      declined="Drag the others again"
      judge "$res" 1 downloads 1 "$h/Downloads" 0
      judge "$res" 2 documents 1 "$h/Documents" 0
      exported "$res" 2 documents plan.pdf yes
      # The key named by the list beside the portal's target: the drop is declined before
      # the portal is asked, so no document of it is given to the copy.
      judge "$res" 3 ssh 0 - 1 "$declined"
      unasked "$res" 3 ssh
      exported "$res" 3 ssh .ssh/id_ed25519 no
      judge "$res" 4 hidden 0 - 1 "$declined"
      unasked "$res" 4 hidden
      exported "$res" 4 hidden Documents/.env no
      # A list whose entries are no plain file of this computer is not judged, and the
      # portal is not asked.
      for n in 5 6 7 8; do
        judge "$res" $n key-written-otherwise 0 - 1 "$unreached"
        unasked "$res" $n key-written-otherwise
        exported "$res" $n key-written-otherwise .ssh/id_ed25519 no
      done
      # One refused file declines the whole drop.
      judge "$res" 9 documents-and-ssh 0 - 1 "$declined"
      unasked "$res" 9 documents-and-ssh
      exported "$res" 9 documents-and-ssh .ssh/id_ed25519 no
      judge "$res" 10 documents-list-only 0 - 1 "$unreached"
      judge "$res" 11 downloads-list-only 1 "$h/Downloads" 0
      # The same notice for a file that is nowhere: the notice tells nothing of what the
      # person has outside the sandbox.
      judge "$res" 12 nowhere-list-only 0 - 1 "$unreached"
      took "$res" 13 two-in-a-row report.pdf
      # No list to judge: the portal is never asked.
      judge "$res" 14 portal-only 0 - 1 "$unreached"
      unasked "$res" 14 portal-only
      exported "$res" 14 portal-only .ssh/id_ed25519 no
      # The first drop is given up for the second, which is judged as itself: declined.
      judge "$res" 15 slow-then-key 0 - 1 "$declined"
      unasked "$res" 15 slow-then-key
      exported "$res" 15 slow-then-key .ssh/id_ed25519 no
      # A link in a plain folder is only found once the portal has given its file: the
      # document stays given to the copy, which cannot give it back.
      judge "$res" 16 link-to-ssh 0 - 1 "$private"
      exported "$res" 16 link-to-ssh .ssh/id_ed25519 yes
      if [[ -n $earlier ]]; then
        judge "$res" 17 earlier-document 0 - 1 "$private"
      else
        say "$name 17 earlier-document: WRONG: drop 16 brought no document portal path"
        failed=1
      fi
    fi
  done
  exit $failed
fi

# Outside: the helpers built, the person's files made, then the session.
[[ $# -ge 2 ]] || { echo "usage: $0 <out> <name>=<Fogata.flatpak>..." >&2; exit 2; }
out=$1
shift
mkdir -p "$out"
out=$(cd "$out" && pwd)
missing=()
for tool in flatpak cc pkg-config wayland-scanner gdbus; do
  command -v "$tool" > /dev/null || missing+=("$tool")
done
protocol=/usr/share/plasma-wayland-protocols/fake-input.xml
[[ -e $protocol ]] || missing+=("$protocol")
[[ -c /dev/fuse ]] || missing+=(/dev/fuse)
pkg-config --exists gtk+-3.0 wayland-client || missing+=("GTK 3's or Wayland's headers")
if [[ ${#missing[@]} -gt 0 ]]; then
  echo "drop: missing ${missing[*]}" >&2
  exit 3
fi
tools=$out/tools
mkdir -p "$tools"
# shellcheck disable=SC2046
cc -O1 -Wall -o "$tools/drag_source" "$here/drag_source.c" $(pkg-config --cflags --libs gtk+-3.0) ||
  exit 1
wayland-scanner client-header "$protocol" "$tools/fake-input-client-protocol.h" &&
  wayland-scanner private-code "$protocol" "$tools/fake-input-protocol.c" || exit 1
# shellcheck disable=SC2046
cc -O1 -Wall -I"$tools" -o "$tools/fake_pointer" "$here/fake_pointer.c" "$tools/fake-input-protocol.c" \
  $(pkg-config --cflags --libs wayland-client) || exit 1

# The person's files, in the session's home folder (session.sh's <folder>/home).
home=$out/session/home
mkdir -p "$home/Downloads" "$home/Documents" "$home/.ssh" "$home/.config"
# shellcheck disable=SC2016 # the file says $HOME itself
printf 'XDG_DOWNLOAD_DIR="$HOME/Downloads"\nXDG_DOCUMENTS_DIR="$HOME/Documents"\n' \
  > "$home/.config/user-dirs.dirs"
echo report > "$home/Downloads/report.pdf"
echo plan > "$home/Documents/plan.pdf"
echo key > "$home/.ssh/id_ed25519"
echo SECRET=1 > "$home/Documents/.env"
ln -sfn "$home/.ssh/id_ed25519" "$home/Documents/innocent.jpg"

copies=()
for copy in "$@"; do
  [[ $copy == *=* && -f ${copy#*=} ]] || { echo "drop: no Flatpak in $copy" >&2; exit 2; }
  copies+=("${copy%%=*}=$(cd "$(dirname "${copy#*=}")" && pwd)/$(basename "${copy#*=}")")
done
# Qt's KDE theme crashes KWin 6.6 at its start in a home folder with no Plasma settings
# (Ubuntu 26.04, Qt's handleThemeChanged): the generic one draws nothing a drag needs.
QT_QPA_PLATFORMTHEME=${QT_QPA_PLATFORMTHEME:-generic} \
SMOKE_FUSE=1 SMOKE_SESSION_CHECK=0 SMOKE_NO_PERMISSION_CHECKS=1 "$here/session.sh" "$out/session" \
  env DROP_INSIDE=1 DROP_OUT="$out" DROP_COPIES="${copies[*]}" "$here/drop.sh"
status=$?
case $status in
  0) echo "drop: passed ($out/summary.txt)" ;;
  3) echo "drop: SKIPPED: this computer cannot run it (see above)" ;;
  *) echo "drop: FAILED ($out/summary.txt)" ;;
esac
exit $status
