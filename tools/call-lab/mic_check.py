"""Judges one mic.sh run from what the sound server saw each copy record
(recordings.log), the moments the copies and mic.sh wrote to meet/, and the copies'
results. Prints PASS or FAIL with the reasons; exits 1 on a failure.

Usage: mic_check.py <run folder> <muted|switch|unplug|late|alone|devices|media|rejoin|echo|noise|broken|alsa|hang|quit|restart>
"""

import json
import math
import pathlib
import re
import struct
import sys
import time

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
elif check == "restart":
    # The session's sound server stopped (sound-stopped) and started again, and the lab's
    # devices made again (devices-back): each copy must record from its own microphone
    # again within RESTART_WITHIN of that (call_test.dart's _restartWithin), as the
    # sound server saw it; what each copy saw and said is in its log.
    RESTART_WITHIN = 20.0
    settings = (run / "settings.txt").read_text().splitlines()
    notes += settings[1:]
    stopped, back = moment("sound-stopped"), moment("devices-back")
    if None in (stopped, back):
        problems.append(f"the sound server was not stopped and started again: {result('a')}")
    else:
        notes.append(f"the lab's devices were back {back - stopped:.1f} s after the server stopped")
        for role in ("a", "b"):
            mine = f"lab{role}_mic"
            if mine not in sources_at(role, stopped - 0.5):
                problems.append(
                    f"{role} did not record from its microphone before the restart: "
                    f"{sorted(sources_at(role, stopped - 0.5)) or 'nothing'}"
                )
            # What already records from it when the devices are back counts from then:
            # recordings.log has a line only when something changes.
            again = (
                back
                if mine in sources_at(role, back)
                else next(
                    (t for t, sources in recordings(role) if t >= back and mine in sources),
                    None,
                )
            )
            if again is None or again - back > RESTART_WITHIN:
                problems.append(
                    f"{role} did not record from its microphone again within "
                    f"{RESTART_WITHIN:.0f} s of the devices being back"
                )
            else:
                notes.append(
                    f"{role} records from its microphone again {1000 * (again - back):.0f} ms "
                    "after the devices were back"
                )
            # Before the lab's devices are made again the server may link a recording to
            # whatever source comes first, which is told; the other copy's microphone,
            # never (a device chosen by its place in a list that changed).
            others = sources_between(role, 0, 1e12) - {mine, "lab_default_mic"}
            theirs = "laba_mic" if role == "b" else "labb_mic"
            if theirs in others:
                problems.append(f"{role} recorded from the other copy's microphone")
            elif others:
                notes.append(f"{role} also recorded from {sorted(others)}")
        notes += re.findall(r"restart[^\n]*", log("a") + log("b"))
        # How a's audio device lost the server and found it again (FOGATA_WEBRTC_LOG=info).
        device = re.findall(
            r"[^\n]*PulseAudio(?: sound server|:)[^\n]*",
            (run / "a.out").read_text(errors="replace"),
        )
        notes += [f"a's WebRTC: {line.strip()[:160]}" for line in device[:12]]
        lab = run / "lab.log"
        if lab.exists():
            notes += [f"lab: {line}" for line in lab.read_text().splitlines()]
elif check == "devices":
    # a's own result says whether its devices stayed listed; its log, what each list held.
    notes += re.findall(r"(devices [^\n]*|held over[^\n]*|soak [^\n]*|calls [^\n]*)", log("a"))
elif check == "media":
    # Each copy's own result says whether anything was held; its log, what was.
    notes += re.findall(r"(held [^\n]*|round [^\n]*)", log("a") + log("b"))
elif check == "rejoin":
    # Each copy logs every time it measured: who left, the pause, and how soon the other's
    # voice reached it after the one leaving entered again (call_test.dart, _rejoin).
    REJOIN_WITHIN_MS = 6000  # call_test.dart's _rejoinWithin
    times = {}
    for role in ("a", "b"):
        for tag, left, pause, direction, ms in re.findall(
            r"rejoin (\w+): (\w) left, pause (\d+) s, voice (to|from) the one that left after (-?\d+) ms",
            log(role),
        ):
            times.setdefault((left, direction), []).append(int(ms))
            if int(ms) > REJOIN_WITHIN_MS:
                problems.append(f"{role}: voice back {ms} ms after {left} entered again ({tag})")
    if not times:
        problems.append("no time measured")
    for (left, direction), ms in sorted(times.items()):
        ms.sort()
        notes.append(
            f"{left} left, voice {direction} the one that left: min {ms[0]} median "
            f"{ms[len(ms) // 2]} max {ms[-1]} ms ({len(ms)})"
        )
    for left in ("a", "b"):
        if len(times.get((left, "to"), [])) != len(times.get((left, "from"), [])):
            problems.append(f"{left} left: the copies measured a different number of times")
elif check == "echo":
    # What each copy played in each stretch b marked (lab_phase_recorder: 16-bit mono at
    # 16 kHz, with the recording's start time by the clock in <stretch>-<role>.start),
    # measured in the turns the copies keep (tone.py turns, lab_tones with LAB_TURNS; each
    # copy's slot and steady tone in turns.txt): each copy talks (noise from 300 Hz to
    # 2.5 kHz, rising and falling as speech) in turns of its own, then holds a steady tone
    # alone; the other application's sound a's screen takes is noise from 5.3 to 6.5 kHz
    # (tone.py screen), told from the voices by its band.
    RATE = 16000
    # As tone.py keeps them.
    PERIOD = 16.0
    TURNS = (((0.0, 1.5), (4.0, 1.5)), ((2.0, 1.5), (6.0, 1.5)))
    STEADY = ((8.0, 4.0), (12.0, 3.9))
    turns = {}
    for line in (run / "turns.txt").read_text().split("\n"):
        if not line.strip():
            continue
        role, *fields = line.split()
        values = dict(field.split("=") for field in fields)
        turns[role] = (int(values["slot"]), float(values["steady"]))
    # A copy's voice coming back to it, against the other copy's playing of it: this much
    # or louder is an echo it hears (a room's echo the other copy did not cancel, its own
    # call carried back in a's screen's sound). An echo canceller leaves 20 dB and less; no
    # echo cancelling, about as loud (0 dB), the loop being as loud as what is played.
    ECHO_WITHIN = 10.0
    # What a copy sends while only the other talks (its level after its echo canceller, from
    # call_test.dart's level log), against its own voice: this close or closer, it sends the
    # other's voice on (with its echo canceller working, 45 dB under and more; with none,
    # within 10 dB). Measured from the start of each call, which the recordings above
    # miss.
    SENT_ECHO_UNDER = 30.0
    # The first turn the other takes in each call of a copy is judged on its own, looser:
    # the echo canceller finds the echo's delay from the other's voice, in about half a
    # second (a new one at the first call: 60 to 76 dB under the copy's own voice over the
    # whole turn, its loudest moment 17 dB under). Kept from the last call (libwebrtc
    # without WebRTC patch 0009), it held that call's alignment until it saw too many played
    # blocks, 1.2 to 2 s, and let the whole turn through (3 to 12 dB under).
    FIRST_TURN_UNDER = 20.0
    # A steady tone this weak early on is not there (nothing recorded, the microphone off).
    STEADY_THERE = -60.0
    # The processing the application asks WebRTC for, for its microphone (media.dart's
    # _micProcessing; the application's noise suppression is its own, after WebRTC's
    # processing): what each copy's WebRTC last applied before each stretch must match.
    ASKED = {
        "high_pass_filter": 0,
        "echo_canceller": 1,
        "noise_suppression": 0,
        "gain_controller1": 1,
    }
    # Heard at all: a voice or the screen's sound above -40 dBFS.
    HEARD = 10 ** (-40 / 20)

    def samples_of(path):
        data = path.read_bytes() if path.exists() else b""
        n = len(data) // 2
        return struct.unpack(f"<{n}h", data[: 2 * n])

    def started(path):
        start = path.with_suffix(".start")
        return float(start.read_text().split()[0]) if start.exists() else None

    def tone(samples, hertz):
        """The amplitude of the tone at `hertz` in `samples`, as a share of full scale
        (Goertzel, Hann window)."""
        m = len(samples)
        if m < RATE // 10:
            return 0.0
        k = 2 * math.cos(2 * math.pi * hertz / RATE)
        s1 = s2 = 0.0
        gain = 0.0
        for i, x in enumerate(samples):
            w = 0.5 - 0.5 * math.cos(2 * math.pi * i / (m - 1))
            gain += w
            s1, s2 = x * w + k * s1 - s2, s1
        return 2 * math.sqrt(max(s1 * s1 + s2 * s2 - k * s1 * s2, 0.0)) / gain / 32768

    def biquad(kind, hertz):
        """A second-order Butterworth low-pass or high-pass filter (b, a), at RATE."""
        w = 2 * math.pi * hertz / RATE
        c = math.cos(w)
        alpha = math.sin(w) / (2 * math.sqrt(0.5))
        if kind == "low":
            b = [(1 - c) / 2, 1 - c, (1 - c) / 2]
        else:
            b = [(1 + c) / 2, -(1 + c), (1 + c) / 2]
        a0 = 1 + alpha
        return [x / a0 for x in b], [-2 * c / a0, (1 - alpha) / a0]

    def band(samples, low, high):
        """The RMS of `samples` (a share of full scale) between `low` and `high` Hz
        (fourth-order Butterworth on either side)."""
        if not samples:
            return 0.0
        stages = [
            biquad("high", low),
            biquad("high", low),
            biquad("low", high),
            biquad("low", high),
        ]
        state = [[0.0, 0.0, 0.0, 0.0] for _ in stages]
        power = 0.0
        for x in samples:
            for (b, a), z in zip(stages, state):
                y = b[0] * x + b[1] * z[0] + b[2] * z[1] - a[0] * z[2] - a[1] * z[3]
                z[1], z[0], z[3], z[2] = z[0], x, z[2], y
                x = y
            power += x * x
        return math.sqrt(power / len(samples)) / 32768

    def fft(re, im):
        """An iterative radix-2 FFT of re + i im, in place (their length a power of 2)."""
        size = len(re)
        j = 0
        for i in range(1, size):
            bit = size >> 1
            while j & bit:
                j ^= bit
                bit >>= 1
            j |= bit
            if i < j:
                re[i], re[j] = re[j], re[i]
                im[i], im[j] = im[j], im[i]
        length = 2
        while length <= size:
            angle = -2 * math.pi / length
            wr, wi = math.cos(angle), math.sin(angle)
            for start in range(0, size, length):
                cr, ci = 1.0, 0.0
                half = length // 2
                for k in range(start, start + half):
                    tr = re[k + half] * cr - im[k + half] * ci
                    ti = re[k + half] * ci + im[k + half] * cr
                    re[k + half], im[k + half] = re[k] - tr, im[k] - ti
                    re[k] += tr
                    im[k] += ti
                    cr, ci = cr * wr - ci * wi, cr * wi + ci * wr
            length *= 2

    def spectrum_power(samples, low, high):
        """The power of `samples` between `low` and `high` Hz, from a Hann-windowed FFT
        (a share of full scale squared): nothing outside the band leaks in more than the
        window's far sidelobes (under -80 dB two kHz away)."""
        m = len(samples)
        if m < 64:
            return 0.0
        size = 1 << (m - 1).bit_length()
        window = [0.5 - 0.5 * math.cos(2 * math.pi * i / (m - 1)) for i in range(m)]
        re = [x * w / 32768 for x, w in zip(samples, window)] + [0.0] * (size - m)
        im = [0.0] * size
        fft(re, im)
        first = int(low * size / RATE)
        last = int(high * size / RATE)
        power = sum(re[k] * re[k] + im[k] * im[k] for k in range(first, last + 1))
        # Parseval, with the window's own power: the mean square of the band's part (both
        # halves of the spectrum).
        gain = sum(w * w for w in window)
        return 2 * power / (size * gain)

    def lag(played, heard, longest):
        """How many samples (RATE / 2, halved by pairs) after `played` its copy in `heard`
        comes, up to `longest`, and how alike they are there (0 to 1), from the peak of
        their cross-correlation."""
        x = [played[i] + played[i + 1] for i in range(0, len(played) - 1, 2)]
        y = [heard[i] + heard[i + 1] for i in range(0, len(heard) - 1, 2)]
        size = 1 << (len(x) + len(y)).bit_length()
        xr, xi = x + [0.0] * (size - len(x)), [0.0] * size
        yr, yi = y + [0.0] * (size - len(y)), [0.0] * size
        fft(xr, xi)
        fft(yr, yi)
        # The conjugate of X times Y, transformed back (as the FFT of its conjugate).
        rr = [a * c + b * d for a, b, c, d in zip(xr, xi, yr, yi)]
        ri = [-(a * d - b * c) for a, b, c, d in zip(xr, xi, yr, yi)]
        fft(rr, ri)
        best = max(range(longest // 2 + 1), key=lambda k: rr[k])
        norm = math.sqrt(sum(v * v for v in x) * sum(v * v for v in y)) or 1.0
        return 2 * best, rr[best] / size / norm

    def voice(samples):
        """What is heard of a voice: 200 Hz to 2.6 kHz (the voices are noise from 300 Hz to
        2.5 kHz), where the screen's sound (5.3 to 6.5 kHz, eighth order) is 50 dB down."""
        return math.sqrt(spectrum_power(samples, 200, 2600))

    def screen_sound(samples):
        """What is heard of the screen's sound: 5 to 7 kHz, above the voices."""
        return band(samples, 5000, 7000)

    def windows(start, n, spans, begin, end):
        """The pieces (sample ranges) of a recording that began at `start` by the clock
        and holds `n` samples, at `begin` to `end` seconds into each of `spans` (start,
        seconds) of each period, from 3 s into the recording (the gain settles)."""
        out = []
        seconds = n / RATE
        period = math.floor(start / PERIOD) * PERIOD
        while period < start + seconds:
            for span, _ in spans:
                b = period + span + begin - start
                e = period + span + end - start
                if b >= 3.0 and e <= seconds - 0.1:
                    out.append((int(b * RATE), int(e * RATE)))
            period += PERIOD
        return out

    def db(value):
        return f"{20 * math.log10(value):.0f}" if value > 0 else "-inf"

    def decibels(value):
        return 20 * math.log10(value) if value > 0 else -999.0

    def median(values):
        values = sorted(values)
        return values[len(values) // 2] if values else None

    stretches = sorted(
        {p.name[len("mark-b-") : -len("-start")] for p in meet.glob("mark-b-*-start")},
        key=lambda name: moment(f"mark-b-{name}-start"),
    )
    if "before" not in stretches or not any(n.startswith("after") for n in stretches):
        problems.append(f"b did not get through the steps: {result('b')}")
    recordings_of = {}
    for name in stretches:
        for role in ("a", "b"):
            path = run / "played" / f"{name}-{role}.raw"
            start = started(path)
            samples = samples_of(path)
            if start is None or len(samples) < 6 * RATE:
                problems.append(f"{name}: too little of what {role} played was recorded")
                continue
            recordings_of[(name, role)] = (start, samples)
    # The room's echo (ECHO_ROOM, on unless 0) is gated with the turns by tone.py room,
    # which tells in room.log the loop of each copy it found; ungated, the two echo
    # cancellers chain and the echo measured is theirs together.
    if "ECHO_ROOM=0" not in (run / "settings.txt").read_text():
        room = (run / "room.log").read_text() if (run / "room.log").exists() else ""
        for slot in ("0", "1"):
            if f"slot {slot}: module" not in room:
                problems.append(f"the room's echo of slot {slot} was not gated with the turns")
    a_voice = {True: [], False: []}
    heard_itself = set()
    # Told among the problems after the other copy's sending of each turn (below).
    hears_itself = []
    for name in stretches:
        watching = name == "before" or name.startswith("after")
        both = not name.startswith("gone")
        for role in ("a", "b"):
            if (name, role) not in recordings_of:
                continue
            start, samples = recordings_of[(name, role)]
            other = "b" if role == "a" else "a"
            # The screen's sound plays at b only (a sharer never plays its own).
            screen = screen_sound(samples[RATE:-RATE])
            notes.append(f"{name}, {role} played the screen's sound at {db(screen)} dBFS")
            if role == "b":
                if watching and screen < HEARD:
                    problems.append(
                        f"{name}: b watches but hears no screen sound ({db(screen)} dBFS)"
                    )
                if not watching and screen >= HEARD:
                    problems.append(
                        f"{name}: b does not watch but hears the screen's sound "
                        f"({db(screen)} dBFS)"
                    )
            if not both:
                continue
            slot, steady = turns[role]
            other_slot, other_steady = turns[other]
            # The other's voice as this copy plays it, in the other's turns (0.5 s on,
            # what the call delays a voice by staying under that).
            heard = [
                decibels(voice(samples[b:e]))
                for b, e in windows(start, len(samples), TURNS[other_slot], 0.5, 1.5)
            ]
            # This copy's own voice coming back to it, in its own turns, later by the
            # round trip (under 0.9 s), against the other's playing of it then.
            if (name, other) in recordings_of:
                other_start, other_samples = recordings_of[(name, other)]
                played = [
                    decibels(voice(other_samples[b:e]))
                    for b, e in windows(
                        other_start, len(other_samples), TURNS[slot], 0.5, 1.5
                    )
                ]
                back = [
                    decibels(voice(samples[b:e]))
                    for b, e in windows(start, len(samples), TURNS[slot], 0.7, 1.9)
                ]
                if played and back and median(played) < decibels(HEARD):
                    notes.append(
                        f"{name}: {role}'s own voice coming back not measured: {other} does "
                        f"not play it ({median(played):.0f} dBFS)"
                    )
                elif played and back:
                    returned = median(back) - median(played)
                    line = (
                        f"{name}: {role}'s own voice comes back to it at {median(back):.0f} "
                        f"dBFS, {returned:+.0f} dB from {other}'s playing of it "
                        f"({median(played):.0f} dBFS; {len(back)} turns, loudest "
                        f"{max(back):.0f})"
                    )
                    if returned >= -ECHO_WITHIN:
                        hears_itself.append((name, role, other, f"{line}: {role} hears itself"))
                        heard_itself.add((name, role))
                    else:
                        notes.append(line)
            if not heard:
                problems.append(f"{name}: no turn of {other} measured in what {role} played")
                continue
            notes.append(
                f"{name}: {role} plays {other}'s voice at {median(heard):.0f} dBFS "
                f"({len(heard)} turns, {min(heard):.0f} to {max(heard):.0f})"
            )
            if median(heard) < decibels(HEARD):
                problems.append(f"{name}: {role} does not hear {other} ({median(heard):.0f} dBFS)")
            if role == "b":
                a_voice[watching].append(median(heard))
    # The room's loop, timed: how long after a copy played the other's voice its microphone
    # heard it (<stretch>-<role>-mic.raw, recorded beside what it played: the two
    # recordings start a few ms apart, so the number holds within a stretch, not across).
    # Steady within a stretch, the room's echo kept its delay; whatever moved an echo
    # canceller's delay then moved in the copy's own buffers.
    if "ECHO_ROOM=0" not in (run / "settings.txt").read_text():
        for name in stretches:
            if name.startswith("gone"):
                continue
            for role in ("a", "b"):
                path = run / "played" / f"{name}-{role}-mic.raw"
                probe = run / "played" / f"{name}-{role}-probe.raw"
                if (name, role) not in recordings_of or not (path.exists() or probe.exists()):
                    continue
                start, samples = recordings_of[(name, role)]
                # Started right after what it played (lab_phase_recorder), timed by it.
                # Both in one recording (ECHO_TIMING=1): left what it played, right its
                # microphone, in step; otherwise two recordings that may slide.
                if probe.exists():
                    both = samples_of(probe)
                    samples, mic = both[0::2], both[1::2]
                    how = "in one recording"
                else:
                    mic = samples_of(path)
                    how = "two recordings, which slide under load"
                mic_start = start
                other_slot = turns["b" if role == "a" else "a"][0]
                found = []
                for b, e in windows(start, len(samples), TURNS[other_slot], 0.5, 1.3):
                    at = start + b / RATE
                    m = int((at - mic_start) * RATE)
                    if m < 0 or m + (e - b) + RATE // 5 > len(mic):
                        continue
                    late, alike = lag(samples[b:e], mic[m : m + (e - b) + RATE // 5], RATE // 5)
                    if alike > 0.3:
                        found.append(1000 * late / RATE)
                if found:
                    notes.append(
                        f"{name}: {role}'s microphone heard what it played "
                        f"{min(found):.1f} to {max(found):.1f} ms later ({len(found)} turns: "
                        f"{', '.join(f'{v:.1f}' for v in found)}; {how})"
                    )
                else:
                    notes.append(f"{name}: {role}'s room echo not timed (no turn alike)")
    # ECHO_PROOF: another application plays again what a plays, b's voice among it, and a's
    # screen carries it back to b: b must hear itself in every stretch it watches, or the
    # check does not catch what it is for.
    if "ECHO_PROOF=1" in (run / "settings.txt").read_text():
        for name in stretches:
            if (name == "before" or name.startswith("after")) and (name, "b") not in heard_itself:
                problems.append(
                    f"{name}: ECHO_PROOF: b did not hear its own voice through a's screen's "
                    "sound: the check missed the deliberate echo"
                )
    marks = [
        (name, moment(f"mark-b-{name}-start"), moment(f"mark-b-{name}-end"))
        for name in stretches
        if not name.startswith("gone")
    ]
    levels_of = {}
    for copy in ("a", "b"):
        path = meet / f"level-{copy}.txt"
        levels = []
        if path.exists():
            for line in path.read_text().splitlines():
                fields = line.split()
                if len(fields) == 2:
                    levels.append((int(fields[0]) / 1000, float(fields[1])))
        levels_of[copy] = levels
    # Each copy's steady tone as its own processing put it out (call_test.dart's
    # level-<role>.txt: its microphone's level after WebRTC's processing, before the codec,
    # every 200 ms), in each one held whole while both are in the call, early (0.3 to 0.9
    # s) and from 2.5 s on. Told, not judged: WebRTC's noise suppressor mostly leaves a
    # loud steady tone alone, and an echo canceller removes one the other plays back (with
    # ECHO_PROOF, b's own).
    for copy in ("a", "b"):
        levels = levels_of[copy]
        slot, steady = turns[copy]
        steady_start, seconds = STEADY[slot]
        for name, begin, end in marks:
            if begin is None or end is None:
                continue
            period = math.floor(begin / PERIOD) * PERIOD
            while period < end:
                at = period + steady_start
                period += PERIOD
                if at < begin or at + seconds > end:
                    continue
                early = median([v for t, v in levels if at + 0.3 <= t < at + 0.9])
                late = median([v for t, v in levels if at + 2.5 <= t < at + seconds - 0.1])
                if early is None or late is None:
                    notes.append(f"{name}: no level of {copy}'s own steady tone logged")
                    continue
                line = (
                    f"{name}: {copy}'s steady tone as it sends it: {db(early)} dBFS early, "
                    f"{db(late)} dBFS from 2.5 s on ({decibels(late) - decibels(early):+.0f} dB)"
                )
                notes.append(f"{line}: absent" if decibels(early) < STEADY_THERE else line)
    # What each copy sends while only the other talks, turn by turn from the start of each
    # of its calls (a gap of over a second in its level log: out of the room), against its
    # own voice in its own turns: the echo its echo canceller left. The other's voice
    # reaches it within 0.4 s and its loop stops 0.45 s after the other's turn; its own
    # turn starts 0.5 s after that.
    def stretch_at(t):
        for name, begin, end in marks:
            if begin is not None and end is not None and begin <= t < end:
                return name
        return "between stretches"

    # The sound server's own timing (ECHO_TIMING=1, timing.py's timing.log): snapshots of
    # PipeWire's graph (pw-top) and of every stream's latencies, once a second.
    graph_at, streams_at, loads = [], [], []
    timing = run / "timing.log"
    if timing.exists():
        nodes = None
        for line in timing.read_text(errors="replace").splitlines():
            fields = line.split()
            if len(fields) < 3:
                continue
            try:
                at = float(fields[0])
            except ValueError:
                continue
            if fields[1] == "pw":
                if fields[2] == "S" and "QUANT" in fields:
                    nodes = {}
                    graph_at.append((at, nodes))
                elif nodes is not None and len(fields) >= 12:
                    # S ID QUANT RATE WAIT BUSY W/Q B/Q ERR [FORMAT] NAME
                    nodes[f"{fields[-1]}#{fields[3]}"] = (fields[4], fields[5], fields[10])
            elif fields[1] == "load" and len(fields) >= 4:
                try:
                    loads.append((at, float(fields[2]), int(fields[3].split("=")[1])))
                except (ValueError, IndexError):
                    pass
            elif fields[1] in ("in", "out") and len(fields) >= 7:
                values = dict(f.split("=", 1) for f in fields[4:] if "=" in f)
                streams_at.append((at, f"{fields[1]} {fields[3]}", values))

    def timing_changes(begin, end):
        """What changed in the sound server's timing from just before `begin` to `end`."""
        told = []
        before = [nodes for at, nodes in graph_at if at < begin]
        if before:
            last = before[-1]
            for at, nodes in graph_at:
                if not begin <= at <= end:
                    continue
                for name, (quant, rate, errors) in nodes.items():
                    old = last.get(name)
                    if old is None:
                        told.append(f"{at - begin:+.1f} s node {name} came, quantum {quant}")
                    elif (quant, rate) != old[:2]:
                        told.append(
                            f"{at - begin:+.1f} s node {name} quantum {old[0]}->{quant} "
                            f"rate {old[1]}->{rate}"
                        )
                    elif errors != old[2]:
                        told.append(f"{at - begin:+.1f} s node {name} xruns {old[2]}->{errors}")
                for name in last:
                    if name not in nodes:
                        told.append(f"{at - begin:+.1f} s node {name} went")
                last = nodes
        latest = {}
        for at, key, values in streams_at:
            if at < begin:
                latest[key] = values
            elif at <= end and key in latest:
                for field in ("buffer", "device"):
                    try:
                        old, new = int(latest[key][field]), int(values[field])
                    except (KeyError, ValueError):
                        continue
                    if abs(new - old) >= 2000:
                        told.append(
                            f"{at - begin:+.1f} s {key} {values.get('module', '-')} {field} "
                            f"latency {old / 1000:.0f}->{new / 1000:.0f} ms"
                        )
                latest[key] = values
        if not graph_at and not streams_at:
            return ["no timing log (ECHO_TIMING=0 was set)"]
        return told or ["nothing changed in the sound server's timing"]

    # The lab's own nodes in PipeWire's graph: its outputs and microphones, their loops,
    # the players and recorders (not the copies' own streams).
    LAB_NODE = re.compile(r"^(input\.|output\.)?(lab|loopback)|^(parec|paplay|pw-cat)$")

    def lab_xruns(begin, end):
        """How many xruns the lab's own nodes had from just before `begin` to `end`, and
        on which node first: the sound server missing its deadlines (the computer too busy
        to move the lab's sound in time), which moves an echo canceller's echo."""
        before = [nodes for at, nodes in graph_at if at < begin]
        if not before:
            return 0, None
        last, count, first = before[-1], 0, None
        for at, nodes in graph_at:
            if not begin <= at <= end:
                continue
            for name, (_, _, errors) in nodes.items():
                if not LAB_NODE.search(name.rsplit("#", 1)[0]) or name not in last:
                    continue
                try:
                    more = int(errors) - int(last[name][2])
                except ValueError:
                    continue
                if more > 0:
                    count += more
                    first = first or f"{name.rsplit('#', 1)[0]} at {at - begin - 5:+.1f} s"
            last = nodes
        return count, first

    # Every turn whose echo got through fails. Xruns of the lab's nodes were tried as an
    # excuse and are not one: on an idle PC a third of the turns follow some, the room's
    # loop timed in one recording stayed at the same delay through every turn let through
    # while the echo canceller found the echo up to 12 ms earlier (so what moved is between
    # WebRTC and the sound server, not the loop), and a turn got through with none before
    # it. They are told beside each such turn, with what else changed.
    turns_judged, leaks = [0], [0]

    def leak(copy, line, at, stretch):
        """Fails a turn whose echo got through, and tells what the timing log shows from 5 s
        before it to its end."""
        count, first = lab_xruns(at - 5, at + 1.8)
        problems.append(line)
        leaks[0] += 1
        notes.append(
            f"{stretch}: {copy}'s turn let through at {at % 1000:.1f}: {count} xruns of the "
            f"lab's nodes from 5 s before" + (f" (first {first})" if first else "")
        )
        notes.extend(
            f"  in the 5 s before {copy}'s leak at {at % 1000:.1f}: {c}"
            for c in timing_changes(at - 5, at + 1.8)
        )

    for copy in ("a", "b"):
        other = "b" if copy == "a" else "a"
        levels = levels_of[copy]
        if not levels:
            problems.append(f"no level of {copy} logged (call_test.dart's level-{copy}.txt)")
            continue
        starts = [levels[0][0]] + [t for (p, _), (t, _) in zip(levels, levels[1:]) if t - p > 1.0]
        ends = [p for (p, _), (t, _) in zip(levels, levels[1:]) if t - p > 1.0] + [levels[-1][0]]
        for number, (call_start, call_end) in enumerate(zip(starts, ends), 1):
            inside = [(t, v) for t, v in levels if call_start <= t <= call_end]
            own = median(
                [
                    decibels(v)
                    for t, v in inside
                    if any(s + 0.4 <= t % PERIOD < s + 1.4 for s, _ in TURNS[turns[copy][0]])
                ]
            )
            if own is None or own < decibels(HEARD):
                notes.append(f"{copy}'s call {number}: its own voice not sent, its echo not judged")
                continue
            first = True
            judged = []
            period = math.floor(call_start / PERIOD) * PERIOD
            while period < call_end:
                for span, _ in TURNS[turns[other][0]]:
                    at = period + span
                    if at < call_start - 0.5 or at + 1.8 > call_end:
                        continue
                    sent = [decibels(v) for t, v in inside if at + 0.4 <= t < at + 1.8]
                    if not sent:
                        continue
                    line = (
                        f"{stretch_at(at + 1)}: {copy} sends {other}'s voice on at "
                        f"{median(sent):.0f} dBFS ({median(sent) - own:+.0f} dB from its own "
                        f"voice, loudest {max(sent):.0f}; {other}'s turn {at - call_start:+.1f} s "
                        f"into {copy}'s call {number})"
                    )
                    leaked = median(sent) > own - SENT_ECHO_UNDER
                    turns_judged[0] += 1
                    if first:
                        line = f"{line}, the call's first turn"
                        if median(sent) > own - FIRST_TURN_UNDER:
                            leak(
                                copy,
                                f"{line}: {copy}'s echo canceller let it through as the "
                                "call started",
                                at,
                                stretch_at(at + 1),
                            )
                        else:
                            notes.append(line)
                    elif leaked:
                        leak(
                            copy,
                            f"{line}: {copy}'s echo canceller let it through",
                            at,
                            stretch_at(at + 1),
                        )
                    else:
                        judged.append(median(sent) - own)
                    first = False
                period += PERIOD
            if judged:
                notes.append(
                    f"{copy}'s call {number}: {other}'s voice sent on {max(judged):+.0f} dB from "
                    f"{copy}'s own at most, in {len(judged)} turns judged ({own:.0f} dBFS own)"
                )
    problems.extend(line for _, _, _, line in hears_itself)
    notes.append(f"{leaks[0]} of {turns_judged[0]} turns judged at the sender were let through")
    if loads:
        first_mark = min((b for _, b, _ in marks if b is not None), default=0)
        last_mark = max((e for _, _, e in marks if e is not None), default=0)
        during = [(load, cpus) for at, load, cpus in loads if first_mark <= at <= last_mark]
        if during:
            load, cpus = max(during)
            notes.append(f"the computer's load went up to {load:.1f} on {cpus} processors")
            if load > cpus:
                problems.append(
                    f"the computer's load reached {load:.1f} on {cpus} processors: too busy to "
                    "judge the echo"
                )
    elif not graph_at:
        notes.append("no timing log (ECHO_TIMING=0): nothing told of the sound server")
    # The processing each copy's WebRTC last applied to its microphone before each stretch
    # (its log, FOGATA_WEBRTC_LOG: "AudioProcessing::ApplyConfig: ..." with the time of day),
    # against what the application asks; no such line fails, so that the judgement cannot
    # pass unseen if the log's wording changes.
    def day_seconds_to_clock(seconds, near):
        """The clock time of `seconds` into the local day nearest `near`."""
        day = time.localtime(near)
        midnight = time.mktime((day.tm_year, day.tm_mon, day.tm_mday, 0, 0, 0, 0, 0, -1))
        at = midnight + seconds
        if at - near > 43200:
            at -= 86400
        elif near - at > 43200:
            at += 86400
        return at

    near = next((begin for _, begin, _ in marks if begin is not None), time.time())
    for copy in ("a", "b"):
        applied = []
        for hours, minutes, seconds, config in re.findall(
            r"^webrtc (\d\d):(\d\d):(\d\d\.\d+) \([^)]*\): AudioProcessing::ApplyConfig: (.*)$",
            (run / f"{copy}.out").read_text(errors="replace"),
            re.MULTILINE,
        ):
            at = day_seconds_to_clock(
                3600 * int(hours) + 60 * int(minutes) + float(seconds), near
            )
            values = {}
            for key in ASKED:
                found = re.search(rf"\b{key}: {{ enabled: ([01])", config)
                if found:
                    values[key] = int(found.group(1))
            applied.append((at, values, f"{hours}:{minutes}:{seconds}"))
        # The stretches each applied processing was the last before, in order; None for
        # those with none before them.
        last = {}
        for name, begin, _ in marks:
            if begin is None:
                continue
            before = [entry for entry in applied if entry[0] < begin]
            key = before[-1][2] if before else None
            last.setdefault(key, (before[-1] if before else None, []))[1].append(name)
        for key, (entry, names) in last.items():
            where = ", ".join(names)
            if entry is None:
                problems.append(
                    f"{where}: no processing applied by {copy}'s WebRTC before it in "
                    f"{copy}.out (no \"AudioProcessing::ApplyConfig\" line: FOGATA_WEBRTC_LOG "
                    "off, or worded otherwise)"
                )
                continue
            _, values, clock = entry
            shown = ", ".join(f"{k} {values.get(k, '?')}" for k in ASKED)
            if values != ASKED:
                problems.append(
                    f"{where}: {copy}'s WebRTC last applied {shown} ({clock}), not what the "
                    f"application asks ({', '.join(f'{k} {v}' for k, v in ASKED.items())})"
                )
            else:
                notes.append(f"{where}: {copy}'s WebRTC last applied {shown} ({clock}), as asked")
    # Told, not judged: with the gain control on, a voice's level moves by a few dB from
    # one stretch to the next whatever is sent.
    if a_voice[True] and a_voice[False]:
        out = sum(a_voice[True]) / len(a_voice[True])
        apart = sum(a_voice[False]) / len(a_voice[False])
        notes.append(
            f"a's voice at b: {out:.0f} dBFS while a's screen's sound goes out to b, "
            f"{apart:.0f} dBFS while it does not"
        )
    settings = (run / "settings.txt").read_text().splitlines()
    notes += settings[1:]
    inputs = run / "sink-inputs.json"
    if inputs.exists():
        try:
            players = sorted(
                {
                    f"{i.get('properties', {}).get('application.process.binary', '?')}"
                    f"({i.get('properties', {}).get('media.name', '?')})"
                    for i in json.loads(inputs.read_text() or "[]")
                }
            )
            notes.append(f"playing when b first watched: {', '.join(players)}")
        except json.JSONDecodeError:
            pass
    # What a's screen took (its log with FOGATA_CALL_LOG, which the lab sets): each output
    # node linked, by program, and how many times (once each time b watched). Anything but
    # the lab's other application (pw-cat) is the lab's fault or the product's.
    taken = re.findall(
        r"\[ScreenSound\] linking node (\d+): pid=-?\d+ binary=(\S+) link_group=(\S+)",
        (run / "a.out").read_text(errors="replace"),
    )
    counts = {}
    for node, binary, group in taken:
        key = f"node {node} ({binary}" + ("" if group == "-" else f" in {group}") + ")"
        counts[key] = counts.get(key, 0) + 1
    notes.append(
        "a's screen took "
        + (", ".join(f"{k} {n} times" for k, n in sorted(counts.items())) or "nothing")
    )
    others = sorted({binary for _, binary, _ in taken} - {"pw-cat"})
    if others and "ECHO_FROM=pw-cat (unset)" in (run / "settings.txt").read_text():
        problems.append(f"a's screen took other programs' sound too: {others}")
elif check == "noise":
    # Copy a's level as its processing puts it out (call_test.dart's level log: the time
    # in milliseconds and the level, 0 to 1, every 200 ms), over the stretches it marked:
    # as it entered with noise suppression on and its microphone on (start), with noise
    # suppression turned off (off), on again (on), and after muting and unmuting (again).
    # A fan's steady noise is all its microphone has (tone.py fan).
    # The application's noise suppression lowers such a noise by 25 dB and more; this
    # much is asked of every stretch with it on. libwebrtc without WebRTC patch 0008 left
    # its processing of the microphone off from the entry until the microphone was muted
    # and unmuted, and the noise suppression with it: start and on as loud as off.
    NOISE_UNDER = 10.0
    # The noise suppression takes a moment to settle after a change.
    SETTLE = 1.5
    # Weaker than this with noise suppression off, the noise is not there.
    NOISE_THERE = -60.0
    path = meet / "level-a.txt"
    levels = []
    for line in path.read_text().splitlines() if path.exists() else []:
        fields = line.split()
        if len(fields) == 2:
            levels.append((int(fields[0]) / 1000, float(fields[1])))

    # A level the log gives as 0 (under one step of 16 bits) counts as this.
    FLOOR = -96.0
    # With noise suppression on, a reading this loud is the noise getting through for a
    # moment: counted and told, not judged (the level over a stretch is its median).
    THROUGH = -40.0

    def stretch(name):
        """The readings of a stretch in dBFS, or None when a did not mark or log it."""
        start, end = moment(f"mark-a-{name}-start"), moment(f"mark-a-{name}-end")
        if start is None or end is None:
            return None
        inside = [v for t, v in levels if start + SETTLE <= t <= end]
        if len(inside) < 5:
            return None
        return sorted(20 * math.log10(v) if v > 0 else FLOOR for v in inside)

    names = ("start", "off", "on", "again")
    read = {name: stretch(name) for name in names}
    if None in read.values():
        missing = [name for name in names if read[name] is None]
        problems.append(f"a's level was not logged in every stretch (not in {missing}): {result('a')}")
    else:
        sent = {name: read[name][len(read[name]) // 2] for name in names}
        notes.append("a's noise as it sends it (median): " + ", ".join(f"{name} {sent[name]:.1f} dBFS" for name in names))
        through = {name: sum(1 for v in read[name] if v > THROUGH) for name in names if name != "off"}
        if any(through.values()):
            notes.append(
                f"readings over {THROUGH:.0f} dBFS with noise suppression on: "
                + ", ".join(f"{name} {through[name]} of {len(read[name])}" for name in through)
            )
        if sent["off"] < NOISE_THERE:
            problems.append(f"no noise from a's microphone with noise suppression off ({sent['off']:.1f} dBFS)")
        for name, when in (("start", "as a entered"), ("on", "turned on again"), ("again", "after muting and unmuting")):
            lowered = sent["off"] - sent[name]
            if lowered < NOISE_UNDER:
                problems.append(
                    f"noise suppression {when} lowered the noise by {lowered:.1f} dB only "
                    f"(under {NOISE_UNDER:.0f} dB: {sent[name]:.1f} dBFS, {sent['off']:.1f} dBFS with it off)"
                )
    notes += re.findall(r"noise suppression (?:on|off)[^\n]*", (run / "a.out").read_text(errors="replace"))[:6]
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
