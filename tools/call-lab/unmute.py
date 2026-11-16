"""Measures one mic.sh unmute run: how loud copy b played copy a's sound as a's microphone
came back, against the same moments 3 s and 6 s later (the room's noise and the same few
words, played again 3 s apart: mic.sh), from b's output recorded throughout. A burst
shows as the first windows well above the later ones. Writes the table to unmute.txt and
prints it; writes b's recording as played-b.wav and a's microphone as mic-a.wav, and one
clip of b's for each unmute, 2 s before it to 4 s after, in clips/. Measured, not judged:
fails only when the run gave nothing to measure.

Each unmute's sound resumes at the first sample at b louder than -72 dBFS after the press
(`mark-a-unmute-<n>`, less 0.2 s), with the 20 ms after it louder than -75 dBFS RMS. The
windows start there: 100, 300, 1000 and 2500 ms. Levels are dBFS: RMS (a full-scale
sine is -3) and peak. A word starting a few tens of ms earlier or later in one window
than in the other moves these by many dB, so each unmute also gets its floor and its top
(the 10th and the 95th percentile of the 50 ms levels) over the first second after the
sound resumed against the same over seconds 3 to 6: a burst raises the first second's
floor (the room's noise) or top (the words), whenever the words fall.

Usage: unmute.py <run folder> [label, for the clips' names]
"""

import array
import math
import pathlib
import re
import sys
import time
import wave

run = pathlib.Path(sys.argv[1])
label = sys.argv[2] if len(sys.argv) > 2 else "today"
label = re.sub(r"[^A-Za-z0-9.-]+", "-", label).strip("-") or "today"
meet = run / "meet"
RATE = 48000
WINDOWS = (0.1, 0.3, 1.0, 2.5)
LATER = (3.0, 6.0)
problems = []


def moment(name):
    path = meet / name
    try:
        return int(path.read_text().split()[0]) / 1000
    except (OSError, ValueError, IndexError):
        return None


def recording(name):
    """(start in seconds since the epoch, samples) of a raw recording mic.sh made, or
    of the WAV this wrote from it before."""
    raw, wav, start = run / f"{name}.raw", run / f"{name}.wav", run / f"{name}.start"
    if not start.exists():
        return None, []
    at = float(start.read_text().split()[0])
    if raw.exists():
        data = raw.read_bytes()
        data = data[: len(data) // 2 * 2]
        with wave.open(str(wav), "wb") as out:
            out.setnchannels(1)
            out.setsampwidth(2)
            out.setframerate(RATE)
            out.writeframes(data)
        raw.unlink()
    elif wav.exists():
        with wave.open(str(wav), "rb") as f:
            data = f.readframes(f.getnframes())
    else:
        return None, []
    samples = array.array("h")
    samples.frombytes(data)
    if sys.byteorder != "little":
        samples.byteswap()
    return at, samples


def db(value):
    return 20 * math.log10(value) if value > 0 else -120.0


def levels(samples, a, b):
    """(RMS, peak) in dBFS of samples [a, b), or None past the recording."""
    a, b = max(0, int(a)), int(b)
    if b > len(samples) or b <= a:
        return None
    part = samples[a:b]
    rms = math.sqrt(sum(x * x for x in part) / len(part)) / 32768
    peak = max(abs(x) for x in part) / 32768
    return db(rms), db(peak)


def spread(samples, a, b):
    """(10th, 95th percentile) of the 50 ms RMS levels in [a, b), or None."""
    step = RATE // 20
    out = []
    i = max(0, int(a))
    while i + step <= min(len(samples), int(b)):
        out.append(levels(samples, i, i + step)[0])
        i += step
    if not out:
        return None
    out.sort()
    return out[int(0.1 * (len(out) - 1))], out[int(0.95 * (len(out) - 1))]


def onset(samples, start, end):
    """The first sample from `start` louder than -72 dBFS whose next 20 ms are louder
    than -75 dBFS RMS, or None."""
    floor = 32768 * 10 ** (-72 / 20)
    i = max(0, int(start))
    end = min(len(samples), int(end))
    while i < end:
        if abs(samples[i]) >= floor:
            after = levels(samples, i, i + RATE // 50)
            if after and after[0] > -75:
                return i
            i += RATE // 100
        else:
            i += 1
    return None


def clip(samples, a, b, path):
    a, b = max(0, int(a)), min(len(samples), int(b))
    with wave.open(str(path), "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        part = samples[a:b]
        if sys.byteorder != "little":
            part.byteswap()
        out.writeframes(part.tobytes())


def clock(t):
    return time.strftime("%H:%M:%S", time.localtime(t)) + f".{int(t * 1000) % 1000:03d}"


b_start, played = recording("played-b")
a_start, heard = recording("mic-a")
offs = (run / "offs.txt").read_text().split() if (run / "offs.txt").exists() else []
lines = [f"unmute, {label}: " + (run / "settings.txt").read_text().strip().replace("\n", "; ")]
sound = run / "sound" / "sound.txt"
if sound.exists():
    lines.append("sound at a's microphone: " + sound.read_text().strip())
if not played:
    problems.append("no recording of what b played")
rows = []
n = 0
clips = run / "clips"
clips.mkdir(exist_ok=True)
while played:
    n += 1
    at = moment(f"mark-a-unmute-{n}")
    if at is None:
        break
    muted = moment(f"mark-a-mute-{n}")
    off = offs[n - 1] if n <= len(offs) else (f"{at - muted:.0f}" if muted else "?")
    pos = (at - b_start) * RATE
    clip(played, pos - 2 * RATE, pos + 4 * RATE, clips / f"{label}-u{n}-off{off}s.wav")
    room = None
    if heard:
        apos = (at - a_start) * RATE
        room = levels(heard, apos - 1.5 * RATE, apos - 0.5 * RATE)
    before = levels(played, pos - 1.0 * RATE, pos - 0.2 * RATE)
    start = onset(played, pos - 0.2 * RATE, pos + 6 * RATE)
    row = {"n": n, "off": off, "room": room, "before": before, "onset": start}
    if start is None:
        problems.append(f"unmute {n}: b played nothing within 6 s")
    else:
        row["delay"] = (start - pos) / RATE * 1000
        for w in WINDOWS:
            first = levels(played, start, start + w * RATE)
            later = [levels(played, start + d * RATE, start + (d + w) * RATE) for d in LATER]
            row[w] = (first, later)
        row["spread"] = (spread(played, start, start + RATE), spread(played, start + 3 * RATE, start + 6 * RATE))
    rows.append(row)
if played and not rows:
    problems.append("no unmute marked by a")


def fmt(x):
    return "  n/a" if x is None else f"{x:5.1f}"


lines.append("")
lines.append(
    "levels at b after each unmute, dBFS: first = the window from when the sound resumed;"
    " +3 s, +6 s = the same window 3 and 6 s later (the same noise and words);"
    " d = first minus +3 s"
)
head = "unmute  off(s)  delay(ms)  room@a  b before |" + "|".join(
    f" {int(w * 1000):>4} ms rms first/+3s/+6s   d  | peak first/+3s   d " for w in WINDOWS
)
lines.append(head)
deltas = {w: [] for w in WINDOWS}
peaks = {w: [] for w in WINDOWS}
for row in rows:
    cells = [
        f"{row['n']:>6}",
        f"{row['off']:>7}",
        f"{row['delay']:>10.0f}" if "delay" in row else "      none",
        fmt(row["room"][0] if row["room"] else None).rjust(7),
        fmt(row["before"][0] if row["before"] else None).rjust(9),
    ]
    text = " ".join(cells) + " |"
    for w in WINDOWS:
        first, later = row.get(w, (None, [None, None]))
        r = [first[0] if first else None] + [l[0] if l else None for l in later]
        p = [first[1] if first else None, later[0][1] if later[0] else None]
        d = r[0] - r[1] if r[0] is not None and r[1] is not None else None
        dp = p[0] - p[1] if p[0] is not None and p[1] is not None else None
        if d is not None:
            deltas[w].append(d)
        if dp is not None:
            peaks[w].append(dp)
        text += f" {fmt(r[0])}/{fmt(r[1])}/{fmt(r[2])} {fmt(d)} | {fmt(p[0])}/{fmt(p[1])} {fmt(dp)} |"
    lines.append(text)
lines.append("")
lines.append("floor and top (10th and 95th percentile of 50 ms RMS), first second vs seconds 3 to 6, dBFS:")
floors, tops = [], []
for row in rows:
    first, later = row.get("spread", (None, None))
    if first and later:
        floors.append(first[0] - later[0])
        tops.append(first[1] - later[1])
        lines.append(
            f"  unmute {row['n']} (off {row['off']} s): floor {first[0]:6.1f} vs {later[0]:6.1f} ({first[0] - later[0]:+5.1f}),"
            f" top {first[1]:6.1f} vs {later[1]:6.1f} ({first[1] - later[1]:+5.1f})"
        )
if floors:
    lines.append(f"{label}: mean floor {sum(floors) / len(floors):+5.1f} dB, mean top {sum(tops) / len(tops):+5.1f} dB")
lines.append("")
lines.append(
    f"{label}: mean d (first minus +3 s), RMS / peak: "
    + ", ".join(
        f"{int(w * 1000)} ms {fmt(sum(deltas[w]) / len(deltas[w]) if deltas[w] else None)}"
        f" / {fmt(sum(peaks[w]) / len(peaks[w]) if peaks[w] else None)}"
        for w in WINDOWS
    )
)

# What a's WebRTC said about its audio processing around each unmute (its log, with
# FOGATA_WEBRTC_LOG): whether it started anything over.
said = re.compile(
    r"InitializeLocked|echo controller made anew|ApplyConfig|ApplyOptions|"
    r"StartRecording|StopRecording|output_will_be_muted|\[agc\]"
)
out = run / "a.out"
if out.exists():
    found = [l for l in out.read_text(errors="replace").splitlines() if l.startswith("webrtc ") and said.search(l)]
    lines.append("")
    lines.append(f"a's WebRTC log, processing lines: {len(found)}")
    for row in rows:
        at = moment(f"mark-a-unmute-{row['n']}")
        lines.append(f"  unmute {row['n']} pressed at {clock(at)}")
    for l in found[:200]:
        lines.append("  " + l[:220])

lines.append("")
lines.append(f"{'FAIL' if problems else 'PASS'} unmute ({label}, measured, not judged)")
for p in problems:
    lines.append(f"  problem: {p}")
text = "\n".join(lines) + "\n"
(run / "unmute.txt").write_text(text)
print(text, end="")
sys.exit(1 if problems else 0)
