"""Judges a run of mic.sh unmute:headset or unmute:speakers: what copy b heard of copy a
right after a turned its microphone on again while b held a steady tone (UNMUTE_FAR=tone).

a's microphone has a room's noise throughout and a few words 0.5, 3.5, 6.5, 9.5 and 12.5 s
after each unmute (UNMUTE_WORDS_AT); b's output is recorded throughout (played-b, from
unmute.py). For each unmute, against the same unmute's last gap (14.65 to 15.5 s) and last
word (12.5 s), which a settled echo canceller lets through as it does mid-call:
- headset (no echo path): each word's level (90th percentile of 50 ms RMS levels over the
  word) may be no more than WORD_UNDER dB under the last word's, and the noise floor in each
  gap between the words (20th percentile; 0.25-0.5, 2.65-3.5, 5.65-6.5, 8.65-9.5, 11.65-12.5
  s) no more than FLOOR_UNDER dB under the last gap's. A new echo canceller held the
  microphone down while b's sound played, until its transparent mode had seen 6 s of it
  without echo: the first words went out 8 dB weaker on average (17 dB at worst) and the
  floor of the first 0.5 s 12 dB lower, then it came back at once. With the echo canceller
  kept knowing the room (WebRTC patch 0010) both stayed within 2 dB in this lab.
- speakers (b's tone goes back into a's microphone): b's tone as a sent it (220 Hz, between
  a's words in the first 3.5 s) must stay TONE_UNDER dB under a's words, the bar the echo
  check (mic_check.py) sets for a call's first turn.
b plays nothing of a at all (under SILENT: digital silence, or a few steps of the
converter at most) until a's first sound after the unmute reaches it: on a slow computer
later than 0.25 s after the mark, and the sound then takes up to 0.25 s to be all
there: the step it begins in, then one to four more that came 3 to 48 dB under, in this
lab and on GitHub's runners alike, mostly after the shortest mute. That is the
microphone and the sound's way starting, not the echo canceller holding the microphone
down (which lowers the noise by about 12 dB, never to nothing, and for seconds), so the
first gap is judged from STARTS after the first sound b plays, on the steps that are
not silent, and not at all when less than 0.15 s of them is left before the first
word: only when the sound came within 0.1 s. The note says when it came. The words,
the later gaps and the echo controllers made judge every unmute.
A sound that reaches b more than LATE after the unmute fails it by that name: the unmute
took that long (its offer and answer between the two, or the microphone's start), which
is not the echo canceller's doing, and what a said before it came is not judged.
An echo controller made between a mute and the end of the next unmute's words fails it
too, whatever the levels, from a's WebRTC log (a.out): one made with none made anew on
purpose, and one made anew on purpose in a run that changes no device (settings.txt:
neither UNMUTE_UNPLUG nor UNMUTE_REJOIN), where nothing gives a reason for it.
Usage: unmute_check.py <run folder> headset|speakers
"""

import array
import math
import pathlib
import re
import sys
import time
import wave

WORD_UNDER = 6.0
FLOOR_UNDER = 6.0
TONE_UNDER = 20.0
RATE = 48000
F = RATE // 20
GAPS = [(0.25, 0.5), (2.65, 3.5), (5.65, 6.5), (8.65, 9.5), (11.65, 12.5), (14.65, 15.5)]
WORDS = [0.5, 3.5, 6.5, 9.5, 12.5]
HEARD = -45.0  # dBFS: b must hear a's last word at least this loud
SILENT = -90.0  # dBFS: under it b plays nothing of a: no sound of a's has reached it yet
STARTS = 0.25  # s after a's first sound that b plays it starting, not yet as it is
LATE = 2.0  # s after an unmute by which b must play a's sound

run = pathlib.Path(sys.argv[1])
kind = sys.argv[2]
problems, notes = [], []


def db(x):
    return 20 * math.log10(max(x, 1e-9) / 32768)


def pct(vals, p):
    vals = sorted(vals)
    return vals[min(len(vals) - 1, int(p * len(vals)))] if vals else float("nan")


wav = run / "played-b.wav"
start = run / "played-b.start"
if not wav.exists() or not start.exists():
    print(f"FAIL unmute:{kind}\n  problem: no recording of b's output (played-b.wav)")
    sys.exit(1)
with wave.open(str(wav)) as w:
    raw = w.readframes(w.getnframes())
pcm = array.array("h", raw[: len(raw) // 2 * 2])
t0 = float(start.read_text().split()[0])
lev = [db(math.sqrt(sum(v * v for v in pcm[i : i + F]) / F)) for i in range(0, len(pcm) - F, F)]


def window(at, a, b):
    i, j = int((at - t0 + a) * 20), int((at - t0 + b) * 20)
    return lev[max(0, i) : max(0, j)]


def tone_db(at, a, b, hertz=220):
    i, j = int((at - t0 + a) * RATE), int((at - t0 + b) * RATE)
    seg = pcm[max(0, i) : max(0, j)]
    if len(seg) < 100:
        return float("nan")
    w = 2 * math.pi * hertz / RATE
    re = sum(v * math.cos(w * k) for k, v in enumerate(seg))
    im = sum(v * math.sin(w * k) for k, v in enumerate(seg))
    return db(2 * math.sqrt(re * re + im * im) / len(seg) / math.sqrt(2))


meet = run / "meet"
n = 0
while (meet / f"mark-a-unmute-{n + 1}").exists():
    n += 1
    at = int((meet / f"mark-a-unmute-{n}").read_text().split()[0]) / 1000
    # When a's first sound after the unmute reached b (None: not within the first word).
    first = int((at - t0) * 20)
    came = next((k for k in range(max(0, first), min(len(lev), first + 300)) if lev[k] > SILENT), None)
    came = None if came is None else max(0.0, (came - first) / 20)
    there = 15.0 if came is None else came + STARTS  # from here on b plays a as it is
    gaps = [pct(window(at, a, b), 0.2) for a, b in GAPS]
    for k, (a, b) in enumerate(GAPS[:-1]):
        if there > a:
            left = [x for x in window(at, there, b) if x > SILENT]
            gaps[k] = pct(left, 0.2) if len(left) >= 3 else float("nan")
    # A word is judged on what b played of it once the sound was there, half a second at least.
    words = [window(at, max(w + 0.1, there), w + 2.0) for w in WORDS]
    words = [pct(x, 0.9) if len(x) >= 10 else float("nan") for x in words]
    if len(window(at, *GAPS[-1])) < 5:
        problems.append(f"unmute {n}: b's recording ends before 15.5 s after it")
        continue
    rg, rw = gaps[-1], words[-1]
    g = [x - rg for x in gaps]
    wv = [x - rw for x in words]
    notes.append(
        f"unmute {n}: noise floor between the words "
        + " ".join("none" if x != x else f"{x:+.0f}" for x in g)
        + f" dB against the last gap ({rg:.0f} dBFS); words "
        + " ".join("none" if x != x else f"{x:+.0f}" for x in wv)
        + f" dB against the last ({rw:.0f} dBFS)"
    )
    if there > GAPS[0][0]:
        notes.append(
            f"unmute {n}: a's first sound reached b "
            + ("more than 15 s" if came is None else f"{came:.2f} s")
            + " after the unmute; the first gap is judged from then on"
            + ("" if gaps[0] == gaps[0] else " (too little of it left: not judged)")
        )
    if came is None or came > LATE:
        problems.append(
            f"unmute {n}: a's sound reached b "
            + ("more than 15 s" if came is None else f"{came:.2f} s")
            + f" after the unmute (at most {LATE:.0f}): the unmute took that long, its offer "
            "and answer or the microphone's start; what a said before is not judged"
        )
    if rw < HEARD:
        problems.append(f"unmute {n}: b heard a's last word at {rw:.0f} dBFS: a not heard")
        continue
    if kind == "headset":
        for k, x in enumerate(wv[:-1]):
            if x < -WORD_UNDER:
                problems.append(
                    f"unmute {n}: a's word at {WORDS[k]:.1f} s went out {-x:.0f} dB under its "
                    f"last (at most {WORD_UNDER:.0f}): the echo canceller held the microphone down"
                )
        for k, x in enumerate(g[:-1]):
            if x < -FLOOR_UNDER:
                a, b = GAPS[k]
                problems.append(
                    f"unmute {n}: the room's noise {a:.2f}-{b:.2f} s after it went out {-x:.0f} dB "
                    f"under its later level (at most {FLOOR_UNDER:.0f}): held down, then back as "
                    "a step"
                )
    else:
        tone = max(tone_db(at, a, b) for a, b in ((0.25, 0.5), (2.65, 2.9), (2.9, 3.15), (3.15, 3.4)))
        own = pct([x for x in words if x == x], 0.5)
        notes.append(f"unmute {n}: b's tone as a sent it between the first words {tone:.0f} dBFS, "
                     f"{own - tone:.0f} dB under a's words")
        if tone > own - TONE_UNDER:
            problems.append(
                f"unmute {n}: b's tone went back to it at {tone:.0f} dBFS, {own - tone:.0f} dB under "
                f"a's words (at least {TONE_UNDER:.0f}): a's echo canceller let it through"
            )

# a's WebRTC log (a.out, with FOGATA_WEBRTC_LOG): an echo controller made between a mute
# and the end of the following unmute's words (AEC3's "AEC3 created") that none made anew
# on purpose explains ("echo controller made anew": a call's start, a change of devices)
# lost the room the unmute's restart keeps, whatever the levels came out: the audio
# processing initialized itself again (WebRTC patch 0010 says why: "initialized again
# for ...: echo controller made again"), as a block recorded at 8000 Hz as the
# microphone turned off did before that patch kept the format.
STAMP = re.compile(r"^webrtc (\d\d):(\d\d):(\d\d)\.(\d\d\d) ")


def logged(line, near):
    """The moment of a log line (time of day, local) as seconds since the epoch, on the
    day of `near`."""
    m = STAMP.match(line)
    if not m:
        return None
    h, mi, s, ms = (int(x) for x in m.groups())
    day = time.localtime(near)
    midnight = time.mktime((day.tm_year, day.tm_mon, day.tm_mday, 0, 0, 0, 0, 0, -1))
    t = midnight + h * 3600 + mi * 60 + s + ms / 1000
    return t + 86400 if t < near - 43200 else t - 86400 if t > near + 43200 else t


settings = run / "settings.txt"
settings = settings.read_text(errors="replace") if settings.exists() else ""
changes = any(re.search(rf"\b{k}=\S", settings) for k in ("UNMUTE_UNPLUG", "UNMUTE_REJOIN"))
out = run / "a.out"
log = [l for l in out.read_text(errors="replace").splitlines() if STAMP.match(l)] if out.exists() else []
if not log:
    notes.append("a's WebRTC log (a.out) not there: echo controllers made not judged")
for k in range(1, n + 1) if log else []:
    try:
        muted = int((meet / f"mark-a-mute-{k}").read_text().split()[0]) / 1000
        on = int((meet / f"mark-a-unmute-{k}").read_text().split()[0]) / 1000
    except (OSError, ValueError, IndexError):
        notes.append(f"unmute {k}: no mute marked before it: echo controllers made not judged")
        continue
    seen = [(logged(l, muted), l) for l in log]
    seen = [(t, l) for t, l in seen if t is not None and muted <= t <= on + GAPS[-1][1]]
    anew = [t for t, l in seen if "echo controller made anew" in l]
    made = [(t, l) for t, l in seen if "AEC3 created" in l]
    said = [l for t, l in seen if "echo controller made again" in l]
    if made and not anew:
        when = ", ".join(time.strftime("%H:%M:%S", time.localtime(t)) + f".{int(t * 1000) % 1000:03d}" for t, _ in made)
        problems.append(
            f"unmute {k}: {len(made)} echo controller(s) made between the mute and the words "
            f"after it with none made anew ({when}): the room it kept was lost"
            + ("; " + "; ".join(l[l.find("AudioProcessing"):] for l in said) if said else "")
        )
    elif made and not changes:
        problems.append(
            f"unmute {k}: {len(made)} echo controller(s) made anew on purpose between the mute "
            "and the words after it, in a run that changes no device: the room was not kept"
        )
    elif made:
        notes.append(f"unmute {k}: {len(made)} echo controller(s) made, made anew on purpose")

if n == 0:
    problems.append("no unmute marked (mark-a-unmute-<n>)")
print(f"{'FAIL' if problems else 'PASS'} unmute:{kind}")
for p in problems:
    print(f"  problem: {p}")
for x in notes:
    print(f"  {x}")
sys.exit(1 if problems else 0)
