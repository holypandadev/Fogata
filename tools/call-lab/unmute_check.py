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
Usage: unmute_check.py <run folder> headset|speakers
"""

import array
import math
import pathlib
import sys
import wave

WORD_UNDER = 6.0
FLOOR_UNDER = 6.0
TONE_UNDER = 20.0
RATE = 48000
F = RATE // 20
GAPS = [(0.25, 0.5), (2.65, 3.5), (5.65, 6.5), (8.65, 9.5), (11.65, 12.5), (14.65, 15.5)]
WORDS = [0.5, 3.5, 6.5, 9.5, 12.5]
HEARD = -45.0  # dBFS: b must hear a's last word at least this loud

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
    gaps = [pct(window(at, a, b), 0.2) for a, b in GAPS]
    words = [pct(window(at, w + 0.1, w + 2.0), 0.9) for w in WORDS]
    if len(window(at, *GAPS[-1])) < 5:
        problems.append(f"unmute {n}: b's recording ends before 15.5 s after it")
        continue
    rg, rw = gaps[-1], words[-1]
    g = [x - rg for x in gaps]
    wv = [x - rw for x in words]
    notes.append(
        f"unmute {n}: noise floor between the words "
        + " ".join(f"{x:+.0f}" for x in g)
        + f" dB against the last gap ({rg:.0f} dBFS); words "
        + " ".join(f"{x:+.0f}" for x in wv)
        + f" dB against the last ({rw:.0f} dBFS)"
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
if n == 0:
    problems.append("no unmute marked (mark-a-unmute-<n>)")
print(f"{'FAIL' if problems else 'PASS'} unmute:{kind}")
for p in problems:
    print(f"  problem: {p}")
for x in notes:
    print(f"  {x}")
sys.exit(1 if problems else 0)
