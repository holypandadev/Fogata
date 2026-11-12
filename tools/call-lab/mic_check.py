"""Judges one mic.sh run from what the sound server saw each copy record
(recordings.log), the moments the copies and mic.sh wrote to meet/, and the copies'
results. Prints PASS or FAIL with the reasons; exits 1 on a failure.

Usage: mic_check.py <run folder> <muted|switch|unplug|late|alone|devices|media|broken|alsa|hang|quit>
"""

import pathlib
import re
import sys

run = pathlib.Path(sys.argv[1])
check = sys.argv[2]
meet = run / "meet"
problems, notes = [], []


def pid(role):
    return (run / f"{role}.pid").read_text().strip()


def moment(name):
    """A moment written to meet/, in seconds since the epoch, or None."""
    path = meet / name
    return int(path.read_text().split()[0]) / 1000 if path.exists() else None


# A recording the sound server has not linked to a source yet shows as "none"
# (recording.py). One is so for an instant when it starts and when its source is made
# again, which the sampler (every 0.2 s) sometimes sees; one that stays so this long
# records nothing, and is a fault.
UNLINKED_FOR = 1.0


def recordings(role, unlinked=False):
    """(time, sources) of each change in what the copy records: its sources then. A
    recording not linked to a source yet ("none") counts only once it has stayed so for
    UNLINKED_FOR seconds, or from its first instant with `unlinked` (for the checks where
    any recording at all is the fault)."""
    mine = pid(role)
    out = []
    for line in (run / "recordings.log").read_text().splitlines():
        time, *entries = line.split()
        out.append(
            (float(time), [e.split(":", 1)[1] for e in entries if e.split(":", 1)[0] == mine])
        )
    if unlinked:
        return out
    settled = []
    since = None  # when the copy's unlinked recording was first seen
    for i, (time, sources) in enumerate(out):
        if "none" not in sources:
            since = None
            settled.append((time, sources))
            continue
        since = time if since is None else since
        linked = [s for s in sources if s != "none"]
        # The log holds changes only: this state lasts until the next line, the last one
        # until the run ended.
        until = out[i + 1][0] if i + 1 < len(out) else float("inf")
        settled.append((time, linked))
        if until - since >= UNLINKED_FOR:
            settled.append((max(time, since + UNLINKED_FOR), sources))
    return settled


def sources_between(role, start, end, unlinked=False):
    """Every source the copy recorded from at some moment in [start, end)."""
    seen = sources_at(role, start, unlinked)
    for time, sources in recordings(role, unlinked):
        if start <= time < end:
            seen.update(sources)
    return seen


def sources_at(role, at, unlinked=False):
    current = []
    for time, sources in recordings(role, unlinked):
        if time > at:
            break
        current = sources
    return set(current)


def result(role):
    path = meet / f"{role}-result.txt"
    return path.read_text().splitlines()[0] if path.exists() else "none"


def crashed(role):
    text = (run / f"{role}.out").read_text(errors="replace")
    return re.findall(r"Segmentation fault|Aborted|core dumped|Fatal error in[^\n]*|Check failed[^\n]*|Unhandled Exception[^\n]*", text)


def log(role):
    path = meet / f"{role}.log"
    return path.read_text() if path.exists() else ""


for role in ("a", "b"):
    if crashed(role):
        problems.append(f"{role} crashed: {crashed(role)[:3]}")

if check == "muted":
    joined, unmute, mute, end = (moment(f"mark-b-{m}") for m in ("joined-muted", "unmute", "mute", "end"))
    if None in (joined, unmute, mute, end):
        problems.append(f"b did not get through the steps: {result('b')}")
    else:
        before = sources_between("b", 0, unmute, unlinked=True)
        if before:
            problems.append(f"b recorded while muted, before unmuting: {sorted(before)}")
        on = sources_at("b", unmute + 3)
        if on != {"labb_quiet_mic"}:
            problems.append(f"b unmuted but records from {sorted(on) or 'nothing'}")
        after = sources_between("b", mute + 2, end, unlinked=True)
        if after:
            problems.append(f"b still records 2 s after muting: {sorted(after)}")
        # When the recording ended after the mute.
        stopped = next((t for t, s in recordings("b") if t >= mute and not s), None)
        if stopped is not None:
            notes.append(f"recording ended {1000 * (stopped - mute):.0f} ms after muting")
        notes.append(f"muted for {unmute - joined:.0f} s at the start with no recording")
elif check == "switch":
    quiet, tone = moment("mark-a-to-quiet"), moment("mark-a-to-tone")
    if None in (quiet, tone):
        problems.append(f"a did not get through the steps: {result('a')}")
    else:
        if sources_at("a", quiet - 0.5) != {"laba_mic"}:
            problems.append(f"a records from {sorted(sources_at('a', quiet - 0.5))} before the switch")
        if sources_at("a", quiet + 2) != {"laba_quiet_mic"}:
            problems.append(f"a records from {sorted(sources_at('a', quiet + 2))} after switching to the quiet input")
        if sources_at("a", tone + 2) != {"laba_mic"}:
            problems.append(f"a records from {sorted(sources_at('a', tone + 2))} after switching back")
        others = sources_between("a", 0, 1e12) - {"laba_mic", "laba_quiet_mic"}
        if others:
            problems.append(f"a recorded from other inputs too: {sorted(others)}")
elif check == "unplug":
    unplugged, replugged = moment("unplugged"), moment("replugged")
    if None in (unplugged, replugged):
        problems.append(f"the microphone was not unplugged and plugged in: {result('a')}")
    else:
        notes.append(f"while unplugged a records from {sorted(sources_at('a', unplugged + 3)) or 'nothing'}")
        # The copies end a few seconds after b hears a again.
        back = sources_at("a", replugged + 2)
        if back != {"laba_mic"}:
            problems.append(f"2 s after the microphone came back a records from {sorted(back) or 'nothing'}")
        again = next((t for t, s in recordings("a") if t >= replugged - 1 and "laba_mic" in s), None)
        if again is not None:
            notes.append(f"back on its microphone {1000 * (again - replugged):.0f} ms after it was made again")
        others = sources_between("a", 0, 1e12) - {"laba_mic", "lab_default_mic"}
        if others:
            problems.append(f"a recorded from {sorted(others)}")
        said = re.findall(r"(told: [^\n]*|while unplugged:[^\n]*|a silent for[^\n]*|devices after[^\n]*)", log("a") + log("b"))
        if not re.search(r"told: Your microphone was disconnected", log("a")):
            problems.append("a was not told its microphone went away")
        if not re.search(r"told: Your microphone is back", log("a")):
            problems.append("a was not told its microphone came back")
        notes += said
elif check == "devices":
    # a's own result says whether its devices stayed listed; its log, what each list held.
    notes += re.findall(r"(devices [^\n]*|held over[^\n]*|soak [^\n]*|calls [^\n]*)", log("a"))
elif check == "media":
    # Each copy's own result says whether anything was held; its log, what was.
    notes += re.findall(r"(held [^\n]*|round [^\n]*)", log("a") + log("b"))
elif check in ("late", "alone"):
    if "laba_mic" not in sources_between("a", 0, 1e12):
        problems.append("a never recorded from its microphone")
    notes += re.findall(r"a heard speaking[^\n]*", log("b"))
elif check in ("broken", "alsa", "hang"):
    notes += re.findall(r"no sound server:[^\n]*", log("a"))
    answers = re.findall(r"call [0-9:.]+: sound server: (\w+)", (run / "a.out").read_text(errors="replace"))
    notes.append(f"the platform answered {answers}")
    if check == "hang" and answers[:2] != ["timeout", "timeout"]:
        problems.append(f"a server that never answers was not asked twice: {answers}")
    if sources_between("a", 0, 1e12, unlinked=True):
        problems.append("a recorded something from the lab's server")
elif check == "quit":
    quit_at = moment("mark-a-quit")
    text = (run / "quit.txt").read_text().strip() if (run / "quit.txt").exists() else "no quit"
    notes.append(text)
    notes += re.findall(r"quitting:[^\n]*", log("a"))
    if quit_at is None:
        problems.append(f"a never quit: {result('a')}")
    elif not text.startswith("ended with status 0"):
        problems.append(text)
    else:
        ended = int(text.split()[-1]) / 1000
        notes.append(f"ended {1000 * (ended - quit_at):.0f} ms after quitting")
        late = sources_between("a", ended + 0.5, 1e12, unlinked=True)
        if late:
            problems.append(f"something of a records after it ended: {sorted(late)}")
        recorded = sources_between("a", 0, 1e12)
        notes.append(f"a recorded from {sorted(recorded) or 'nothing'} before it ended")

if check != "quit":
    for role in ("a", "b"):
        if result(role) != "passed":
            problems.append(f"{role}: {result(role)}")

print(f"{'FAIL' if problems else 'PASS'} {check}")
for line in problems:
    print(f"  problem: {line}")
for line in notes:
    print(f"  {line}")
sys.exit(1 if problems else 0)
