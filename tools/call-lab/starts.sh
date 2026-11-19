#!/usr/bin/env bash
# Starts copies of the app's call test (Linux) again and again, all at once, and catches one
# that ends by itself in its first seconds or, with STARTS_ENV_WATCH=1, one that changes its
# own environment where another thread may be reading it. Each round, in one sound session of
# its own (session.sh), on a virtual display started anew as each call of measure.sh has its
# own:
#   - the copies start at once, each as a copy of `measure.sh <copies> screen` does (lab.sh's
#     lab_copy: the same display, software rendering, LP_NUM_THREADS, devices and settings;
#     FOGATA_TEST_MEASURE, and FOGATA_TEST_SCREEN for copy a), so that their start is the same;
#   - they are left running for <seconds> (they wait for each other meanwhile, which is
#     fine: nothing here needs them to get anywhere), and every copy that ended by itself in
#     that time is written down with its exit status, the moment it ended and its output;
#   - the rest are ended with KILL, as the lab ends them, and the round's data folders,
#     meeting folder and temporary folders are deleted.
# The sound session, its devices and the copies' tones last the whole run, as they last a
# call. Every STARTS_EVERY rounds (default 10) a line says how far it is, with the
# computer's load, the processes in the session, /tmp's entries and the disk left, so that a
# leak between rounds shows; the last lines are the summary.
#
# Usage: tools/call-lab/starts.sh [copies, default 6] [rounds, default 100] [seconds, default 8]
# Build first: (cd app && flutter build linux --profile -t integration_test/call_test.dart)
# Environment: BUNDLE (the build; default the profile one), CALL_LAB_RUNS (where the run
# folder goes, default ~/call-lab), MEASURE_ENV (more VAR=value for every copy, as for
# measure.sh), LAB_DISPLAY (default 171), STARTS_EVERY, STARTS_ENV_WATCH=1 (below).
#
# STARTS_ENV_WATCH=1: every copy writes down each change it makes to its own environment
# (setenv, putenv, unsetenv, clearenv: a library of a few lines put before libc with
# LD_PRELOAD, built here with CC, default cc) to envwatch-<role>.log in the run folder: the
# variable's name (never its value), its thread, whether that is the process's main thread,
# how many threads the process had, when (ms after the copy started), whether the
# environment's array moved (glibc then frees the old one), and the stack. glibc's getenv
# reads that array with no lock: a change while another thread reads it can make the reader
# read freed memory. One start on 2026-11-18 crashed so, in getenv on Flutter's raster
# thread, as the Rust bridge's start set RUST_BACKTRACE on a worker thread. So the run fails
# (envwatch.txt says which change, where and when) on
#   - any change from a thread other than the process's main thread, and
#   - any change on the main thread once the process has more than one thread, except
#     GTK's own unsetenv of DESKTOP_STARTUP_ID and DESKTOP_AUTOSTART_ID as it opens the
#     display: GLib's threads exist by then, but they do not read the environment, and the
#     engine's threads, which do, are not started yet.
# The runner's setenv of PIPEWIRE_ALSA comes before any thread, so it passes either rule.
#
# The run folder (printed last) keeps lab.log, summary.txt, ended.txt (one line for each
# copy that ended by itself), ended/<round>-<role>.out (that copy's output) and, watched,
# envwatch.txt. The script ends with status 0 when no copy ended by itself and none changed
# its environment as above, 1 otherwise, 2 on a usage error. A crash's core is not looked
# for here: systemd-coredump keeps it (coredumpctl), outside.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
if [ "${1:-}" = --inside ]; then
  shift
  N=$1 ROUNDS=$2 HOLD=$3
  # shellcheck source=tools/call-lab/lab.sh
  source "$HERE/lab.sh"
  trap 'lab_stop; exit 1' TERM INT HUP
  roles=("${ROLES[@]:0:N}")
  every=${STARTS_EVERY:-10}
  mkdir -p "$RUN/ended"
  : > "$RUN/ended.txt"
  more=()
  [ -n "${MEASURE_ENV:-}" ] && read -r -a more <<< "$MEASURE_ENV"

  preload=()
  if [ "${STARTS_ENV_WATCH:-}" = 1 ]; then
    # Built here as measure.sh builds no-dlclose.so: no file of its own to keep in step.
    # The log's path is read once, before main and any thread; each change appends one
    # record (a line, the stack, an empty line) with one open, so that copies and threads
    # do not mix lines much.
    cat > "$RUN/envwatch.c" << 'EOF'
#define _GNU_SOURCE
#include <dlfcn.h>
#include <execinfo.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

extern char **environ;
static char log_path[1024];
static struct timespec started;

__attribute__((constructor)) static void envwatch_start(void) {
  const char *path = getenv("ENVWATCH_LOG");
  if (path != NULL) snprintf(log_path, sizeof log_path, "%s", path);
  clock_gettime(CLOCK_MONOTONIC, &started);
}

// The process's threads now (the 20th field of /proc/self/stat), 0 when unknown.
static long threads(void) {
  char buf[1024];
  int fd = open("/proc/self/stat", O_RDONLY | O_CLOEXEC);
  if (fd < 0) return 0;
  ssize_t n = read(fd, buf, sizeof buf - 1);
  close(fd);
  if (n <= 0) return 0;
  buf[n] = '\0';
  const char *after = strrchr(buf, ')');
  long count = 0;
  if (after == NULL ||
      sscanf(after + 1, " %*c %*d %*d %*d %*d %*d %*u %*u %*u %*u %*u %*u %*u %*d %*d %*d %*d %ld",
             &count) != 1)
    return 0;
  return count;
}

static void note(const char *call, const char *string, char **before) {
  if (log_path[0] == '\0') return;
  // The name alone: putenv's string holds the value too.
  char name[256] = "";
  if (string != NULL) snprintf(name, sizeof name, "%.*s", (int)strcspn(string, "="), string);
  long count = threads();
  int fd = open(log_path, O_WRONLY | O_APPEND | O_CREAT | O_CLOEXEC, 0644);
  if (fd < 0) return;
  struct timespec now;
  clock_gettime(CLOCK_MONOTONIC, &now);
  long ms = (now.tv_sec - started.tv_sec) * 1000 + (now.tv_nsec - started.tv_nsec) / 1000000;
  char thread[17] = "";
  prctl(PR_GET_NAME, thread);
  long tid = syscall(SYS_gettid);
  dprintf(fd, "pid %d thread %ld \"%s\" %s, threads %ld, %ld ms: %s(%s), environ %p -> %p%s\n",
          getpid(), tid, thread, tid == getpid() ? "main" : "other", count, ms, call, name,
          (void *)before, (void *)environ, before != environ ? " MOVED" : "");
  void *frames[40];
  int n = backtrace(frames, 40);
  backtrace_symbols_fd(frames, n, fd);
  dprintf(fd, "\n");
  close(fd);
}

int setenv(const char *name, const char *value, int overwrite) {
  static int (*real)(const char *, const char *, int);
  if (real == NULL) real = (int (*)(const char *, const char *, int))dlsym(RTLD_NEXT, "setenv");
  char **before = environ;
  int result = real(name, value, overwrite);
  note("setenv", name, before);
  return result;
}

int putenv(char *string) {
  static int (*real)(char *);
  if (real == NULL) real = (int (*)(char *))dlsym(RTLD_NEXT, "putenv");
  char **before = environ;
  int result = real(string);
  note("putenv", string, before);
  return result;
}

int unsetenv(const char *name) {
  static int (*real)(const char *);
  if (real == NULL) real = (int (*)(const char *))dlsym(RTLD_NEXT, "unsetenv");
  char **before = environ;
  int result = real(name);
  note("unsetenv", name, before);
  return result;
}

int clearenv(void) {
  static int (*real)(void);
  if (real == NULL) real = (int (*)(void))dlsym(RTLD_NEXT, "clearenv");
  char **before = environ;
  int result = real();
  note("clearenv", NULL, before);
  return result;
}
EOF
    "${CC:-cc}" -shared -fPIC -O1 -o "$RUN/envwatch.so" "$RUN/envwatch.c" -ldl ||
      { echo "cannot build envwatch.so (CC)" | tee -a "$RUN/lab.log" >&2; exit 1; }
    preload=(LD_PRELOAD="$RUN/envwatch.so")
  fi

  lab_start "$N"

  # A display of its own for each round, as each call of measure.sh starts one: the copies'
  # first frames go to a display nothing has drawn on yet. A display server that had to be
  # killed leaves its lock and socket, which would keep the next one from starting.
  new_display() {
    [ -e "$RUN/xvfb.pid" ] && lab_kill "$(cat "$RUN/xvfb.pid")" Xvfb
    rm -f "/tmp/.X$X-lock" "/tmp/.X11-unix/X$X"
    Xvfb ":$X" -screen 0 "${SCREEN:-1600x900}x24" -nolisten tcp >> "$RUN/xvfb.log" 2>&1 &
    echo $! > "$RUN/xvfb.pid"
    for _ in $(seq 50); do xdpyinfo -display ":$X" > /dev/null 2>&1 && return 0; sleep 0.1; done
    echo "the display did not start" | tee -a "$RUN/lab.log" >&2
    return 1
  }

  # Whether process $1 has ended: a zombie not reaped yet, or gone (bash reaps its
  # children as they end and keeps their status for `wait`).
  ended() {
    local s
    s=$(sed 's/.*) //' "/proc/$1/stat" 2> /dev/null) || return 0
    [ -z "$s" ] || [ "${s:0:1}" = Z ]
  }

  starts=0 gone_total=0 round=0
  declare -A pids gone
  while [ "$round" -lt "$ROUNDS" ]; do
    round=$((round + 1))
    # Nothing of the last round's copies left: their folders go, and the meeting folder
    # through which they find each other starts empty. Their temporary files go to a
    # folder of each copy's own (TMPDIR), deleted with it, not to the computer's /tmp.
    rm -rf "$RUN/data" "$M" "$RUN/tmp"
    mkdir -p "$RUN/data" "$M" "$RUN/tmp"
    new_display || break
    for r in "${roles[@]}"; do
      mkdir -p "$RUN/tmp/$r"
      extra=(FOGATA_TEST_MEASURE=90 TMPDIR="$RUN/tmp/$r")
      [ "$r" = a ] && extra+=(FOGATA_TEST_SCREEN=1)
      if [ ${#preload[@]} -gt 0 ]; then
        printf -- '--- round %s\n\n' "$round" >> "$RUN/envwatch-$r.log"
        extra+=("${preload[@]}" ENVWATCH_LOG="$RUN/envwatch-$r.log")
      fi
      lab_copy "$r" "$N" 1 "${extra[@]}" "${more[@]}"
      pids[$r]=$(cat "$RUN/$r.pid")
    done
    t0=$(lab_ms)
    gone=()
    # Every 0.2 s, which copies have ended, and when.
    while true; do
      el=$(($(lab_ms) - t0))
      for r in "${roles[@]}"; do
        [ -n "${gone[$r]:-}" ] && continue
        ended "${pids[$r]}" && gone[$r]=$el
      done
      [ "$el" -ge $((HOLD * 1000)) ] && break
      sleep 0.2
    done
    for r in "${roles[@]}"; do
      if [ -n "${gone[$r]:-}" ]; then
        wait "${pids[$r]}" 2> /dev/null
        status=$?
        why="status $status"
        [ "$status" -gt 128 ] && why="$why (SIG$(kill -l $((status - 128)) 2> /dev/null))"
        cp "$RUN/$r.out" "$RUN/ended/$round-$r.out"
        line="round $round copy $r (pid ${pids[$r]}): ended by itself within ${gone[$r]} ms of its start, $why; output ended/$round-$r.out"
        echo "$line" | tee -a "$RUN/ended.txt" "$RUN/lab.log"
        gone_total=$((gone_total + 1))
      else
        lab_kill "${pids[$r]}" "$BUNDLE" KILL
      fi
    done
    # A copy's own children, if any outlived it, would pile up round after round.
    left=$(pgrep -f -- "$BUNDLE" | tr '\n' ' ')
    if [ -n "$left" ]; then
      echo "round $round: processes of the build left after the copies ended: $left" >> "$RUN/lab.log"
      # shellcheck disable=SC2086
      kill -9 $left 2> /dev/null
    fi
    starts=$((starts + N))
    if [ $((round % every)) = 0 ] || [ "$round" = "$ROUNDS" ]; then
      echo "round $round of $ROUNDS: $starts starts, $gone_total ended by themselves;" \
        "load $(cut -d' ' -f1-3 /proc/loadavg), $(find /proc -maxdepth 1 -name '[0-9]*' | wc -l)" \
        "processes in the session, /tmp $(find /tmp -mindepth 1 -maxdepth 1 2> /dev/null | wc -l)" \
        "entries, $(df -h --output=avail "$RUN" | tail -n 1 | tr -d ' ') free, $SECONDS s"
    fi
  done
  lab_stop
  rm -rf "$RUN/data" "$M" "$RUN/tmp"

  changed=0
  if [ ${#preload[@]} -gt 0 ]; then
    # Every record judged by the rules above: a line for each change that breaks one (to
    # the output and lab.log), and its whole record, stack and all, to envwatch.txt.
    : > "$RUN/envwatch.txt"
    for r in "${roles[@]}"; do
      [ -e "$RUN/envwatch-$r.log" ] || continue
      awk -v role="$r" -v out="$RUN/envwatch.txt" '
        /^--- round / { round = $3; bad = 0; next }
        $1 == "pid" && $2 != "" {
          other = ($0 ~ /" other, threads [0-9]+, /)
          n = $0; sub(/.*, threads /, "", n); n = n + 0
          name = $0; sub(/^[^:]*: [a-z]+\(/, "", name); sub(/\).*/, "", name)
          late = ($0 ~ /" main, threads [0-9]+, /) && n > 1 &&
            name != "DESKTOP_STARTUP_ID" && name != "DESKTOP_AUTOSTART_ID"
          bad = other || late
          if (bad) {
            what = $0; sub(/^pid [0-9]+ /, "", what); sub(/, environ .*/, "", what)
            print "environment changed: copy " role ", round " round ", " what \
              (other ? " (not the main thread)" : " (the main thread, other threads running)")
          }
        }
        bad { print >> out }
        NF == 0 { bad = 0 }
      ' "$RUN/envwatch-$r.log" | tee -a "$RUN/lab.log"
    done
    changed=$(grep -c '^pid ' "$RUN/envwatch.txt")
    if [ "$changed" != 0 ]; then
      echo "--- each change with its stack (envwatch.txt)"
      cat "$RUN/envwatch.txt"
    fi
  fi
  summary="starts: $round rounds of $N copies, $starts starts, held $HOLD s, $gone_total ended by themselves"
  [ ${#preload[@]} -gt 0 ] && summary="$summary, $changed changes of the environment against the rules"
  echo "$summary, $SECONDS s" | tee "$RUN/summary.txt"
  [ "$gone_total" = 0 ] && [ "$changed" = 0 ]
  exit
fi

N=${1:-6} ROUNDS=${2:-100} HOLD=${3:-8}
[[ $N =~ ^[1-8]$ ]] || { echo "1 to 8 copies" >&2; exit 2; }
[[ $ROUNDS =~ ^[0-9]+$ ]] && [ "$ROUNDS" -ge 1 ] || { echo "rounds: a whole number" >&2; exit 2; }
[[ $HOLD =~ ^[0-9]+$ ]] && [ "$HOLD" -ge 1 ] || { echo "seconds: a whole number" >&2; exit 2; }
REPO=$(cd "$HERE/../.." && pwd)
export BUNDLE=${BUNDLE:-$REPO/app/build/linux/x64/profile/bundle/fogata}
[ -x "$BUNDLE" ] || { echo "no build at $BUNDLE" >&2; exit 2; }
RUN=${CALL_LAB_RUNS:-$HOME/call-lab}/starts-$N-$(date +%m%d-%H%M%S)
export RUN
mkdir -p "$RUN"
echo "copies $N, $ROUNDS rounds, held $HOLD s, $(git -C "$REPO" describe --always --dirty), ${MEASURE_ENV:-}, watch ${STARTS_ENV_WATCH:-0}, processors $(nproc), started $(date -Is), load $(cut -d' ' -f1-3 /proc/loadavg)" > "$RUN/settings.txt"
"$HERE/session.sh" "$RUN/session" "$0" --inside "$N" "$ROUNDS" "$HOLD"
status=$?
echo "$RUN"
exit $status
