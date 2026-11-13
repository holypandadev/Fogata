"""Logs, once a second and inside the lab's sound session, how the sound server moves the
sound along: PipeWire's graph as pw-top's batch mode shows it (every node's block size
QUANT, its RATE and its xruns ERR) and every stream's buffer and device latency as
PipeWire's PulseAudio service tells them (pactl), WebRTC's own playout and recording and
the lab's loops among them. mic_check.py tells what changed before a turn whose echo got
through.

Usage: timing.py <log file>

Lines: "<time> pw <pw-top line>" (a line "<time> pw S ID QUANT ..." starts each of its
blocks) and "<time> <in|out> <index> <program>/<media name> buffer=<us> device=<us>
module=<owner module or ->" for each sink input (in) and source output (out), and
"<time> load <1 min load average> cpus=<processors>".
"""

import json
import os
import signal
import subprocess
import sys
import threading
import time

path = sys.argv[1]
lock = threading.Lock()
out = open(path, "a", buffering=1)


def write(line):
    with lock:
        out.write(line + "\n")


# pw-top's own refresh is a second; it goes with this logger (lab_stop's TERM).
top = subprocess.Popen(
    ["pw-top", "-b"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True
)


def stop(*_):
    top.terminate()
    sys.exit(0)


signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)


def graph():
    for line in top.stdout:
        line = line.rstrip()
        if line:
            write(f"{time.time():.3f} pw {line}")


def streams(kind, tag, device_key):
    done = subprocess.run(
        ["pactl", "-f", "json", "list", kind], capture_output=True, text=True
    )
    try:
        entries = json.loads(done.stdout or "[]")
    except json.JSONDecodeError:
        return
    now = time.time()
    for entry in entries:
        props = entry.get("properties", {})
        name = (
            f"{props.get('application.process.binary', '?')}/"
            f"{props.get('media.name', '?')}".replace(" ", "_")
        )
        write(
            f"{now:.3f} {tag} {entry.get('index')} {name} "
            f"buffer={entry.get('buffer_latency_usec', '?')} "
            f"device={entry.get(device_key, '?')} module={entry.get('owner_module', '-')}"
        )


threading.Thread(target=graph, daemon=True).start()
while True:
    # The computer's load (1 min average) and how many processors it has.
    try:
        load = open("/proc/loadavg").read().split()[0]
    except OSError:
        load = "?"
    write(f"{time.time():.3f} load {load} cpus={os.cpu_count()}")
    streams("sink-inputs", "in", "sink_latency_usec")
    streams("source-outputs", "out", "source_latency_usec")
    time.sleep(1.0)
