"""Judges a run of mic.sh unmute:headset or unmute:speakers: what copy b heard of copy a
right after a turned its microphone on again while b held a steady tone (UNMUTE_FAR=tone).

a's microphone has a room's noise throughout and a few words 0.5, 3.5, 6.5, 9.5 and 12.5 s
after each unmute (UNMUTE_WORDS_AT); b's output is recorded throughout (played-b, from
unmute.py). For each unmute, against the same unmute's last gap (14.65 to 15.5 s) and last
word (12.5 s), which a settled echo canceller lets through as it does mid-call:
- headset (no echo path): each word's level (90th percentile of 50 ms RMS levels over the
  word) may be no more than WORD_UNDER dB under the last word's, and the noise floor in the
  first gap between the words judged (20th percentile; 0.25-0.5, 2.65-3.5, 5.65-6.5,
  8.65-9.5, 11.65-12.5 s), and in each later one while all before it were, no more than
  FLOOR_UNDER dB under the last gap's: held down from the unmute on. A later gap may be
  FLOOR_DEEP under it, as the floor b plays sinks in every pause; a dip past FLOOR_UNDER
  there is noted. A new echo canceller held the
  microphone down while b's sound played, until its transparent mode had seen 6 s of it
  without echo: the first words went out 8 dB weaker on average (17 dB at worst) and the
  floor of the first 0.5 s 12 dB lower, then it came back at once. With the echo canceller
  kept knowing the room (WebRTC patch 0010) both stayed within 2 dB in this lab.
- speakers (b's tone goes back into a's microphone): b's tone as a sent it (220 Hz) must
  stay TONE_UNDER dB under a's words, the bar the echo check (mic_check.py) sets for a call's
  first turn, before a says anything: from a's first sound at b (0.25 s after the unmute
  at the earliest) until a's first word reaches b. That is where an echo canceller that
  kept a room without echo (a headset taken off for speakers, no device change seen) lets
  the tone through. That window ends DELAY after the first word was said, the least it
  takes to reach b, so it never holds the word. A word's last syllable ends WORD_END into
  it; with b's tone playing the words reached b 0.10 to 0.75 s after a said them, so each
  is found in b's recording by its syllables (arrival) and what follows it is judged from
  its end there and up to where the next one was found (DELAY for one not found, noted).
  After a's words b's steady tone comes back for moments, and that is WebRTC's own, left
  as WebRTC has it: AEC3 holds a gentler suppression for 200 ms after the near end stops
  talking (its dominant near-end detector, dominant_nearend_detection.hold_duration, 50
  blocks), and b's tone itself, as b sends it, drops for moments while a's word plays at
  b (b's own processing), and around those changes a's echo canceller lets it through for
  another 0.2 to 0.35 s, up to 0.85 s after the word's end. In this lab and on the
  runners those moments came as loud as 10 dB under a's words (the tone's echo itself
  reaches a's microphone 17 to 25 dB under them on the runners), at any moment of a call,
  not only after an unmute. So from each word's end to the next word
  they are measured and noted for every unmute, and fail by TONE_UNDER only when the tone
  stays above that bar all the way from one word to the next (an echo let through, no
  moment of double talk), and at their loudest only louder than TAIL_UNDER dB under the
  words, which no echo of the lab's can reach without something amplifying it.
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
took that long (the microphone's start, or an offer and answer between the two where
the voice channel only received while muted), which is not the echo canceller's doing,
and what a said before it came is not judged.
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
# The room's noise as b plays it sinks in every pause while a's microphone hears it steady:
# by 2.7 dB from the first half of a gap to the second (median; more than 4 dB in one gap
# in 5), so a gap's floor against the last gap's spreads by 2 dB (sd) and reached 5.7 dB
# under it in the first judged gap and 6.6 in a later one, in 107 unmutes with no echo
# controller made (GitHub's runners and this lab, 2026-11-16 to 11-19). Held down, the
# floor was so from the unmute on: the first judged gap 6.9 to 13.9 dB under, each later
# one less deep than it (8 unmutes with an echo controller made at the unmute: before
# WebRTC patch 0010, and with libwebrtc 8078.7 as the processing initialized itself
# again). So the first judged gap, and each one after it while all before it were too,
# may be no more than FLOOR_UNDER dB under the last; a later gap only FLOOR_DEEP, and a
# dip between the two is noted.
FLOOR_UNDER = 6.0
FLOOR_DEEP = 10.0
TONE_UNDER = 20.0
TAIL_UNDER = 5.0  # dB: the bar after each of a's words, where WebRTC lets the tone through
WORD_END = 2.0  # s into each word that its last syllable ends (unmute-sound.sh)
DELAY = 0.10  # s from a's microphone to b at the least (0.10 measured: below)
# a's words as unmute-sound.sh makes them: each syllable's start and length (s).
SYLLABLES = [(0.00, 0.18), (0.20, 0.22), (0.65, 0.15), (0.82, 0.25), (1.35, 0.16), (1.53, 0.14), (1.70, 0.30)]
# With speakers each word is found where it reached b, and what follows it is judged from
# its end there. With b's tone playing it reached b 0.10 to 0.75 s after a said it (median
# 0.28, 95 % by 0.50; 478 of 495 words found, 33 runs on GitHub's runners, 2026-11-17 to
# 11-19), its way to b and the time b's recording lost or gained, where DELAY put the
# window after it on the word's last syllable, whose pitch falls through the tone's 220 Hz:
# the tone after a word read 24 dB under a's words there (median; 25 or less in 280 of
# 495), 45 from the word's end at b (25 or less in 26).
HOP = 0.01  # s: the envelope's step
SEEK = 1.0  # s after a word is said that its start is looked for at b
FOUND = 6.0  # dB: a word's syllables over its pauses at the match, at the least
LAST_FROM = 1.30  # s into a word: its last syllables, matched again for its end
AGREE = 0.10  # s: how far the last syllables may sit from where the whole word put them
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


def envelope(at, a, b):
    """b's output from a to b s after `at`, as 10 ms levels (dB) of its second difference:
    the tone (220 and 660 Hz) and the fan's noise weak there, a's words' formants strong."""
    i0, step = int((at - t0 + a) * RATE), int(RATE * HOP)
    out = []
    for i in range(max(2, i0), min(len(pcm) - step, int((at - t0 + b) * RATE)), step):
        s = 0
        for k in range(i, i + step):
            d = pcm[k] - 2 * pcm[k - 1] + pcm[k - 2]
            s += d * d
        out.append(10 * math.log10(max(s / step, 1e-6)))
    return [0.0] * max(0, (2 - i0 + step - 1) // step) + out  # before the recording: silence


def match(e, pattern, lags):
    """(lag in hops, dB) where the syllables of `pattern` stand highest above its pauses in
    e (levels under the span's 20th percentile raised to it: the silence before a's first
    sound weighs as the room does)."""
    heard = sorted(x for x in e if x > 0)
    floor = heard[len(heard) // 5] if heard else 0.0
    best = (None, -math.inf)
    for lag in lags:
        seg = [max(x, floor) for x in e[lag : lag + len(pattern)]]
        if len(seg) < len(pattern):
            break
        on = [x for x, p in zip(seg, pattern) if p]
        off = [x for x, p in zip(seg, pattern) if not p]
        c = sum(on) / len(on) - sum(off) / len(off)
        if c > best[1]:
            best = (lag, c)
    return best


def arrival(at, w):
    """(start, end) in s: how much later than said a's word at w began and ended at b, or
    None when not found. The whole word's syllables are matched over SEEK s, then its last
    ones alone near there; both must stand FOUND dB above the pauses and agree within AGREE."""
    pattern = [any(s <= k * HOP < s + d for s, d in SYLLABLES) for k in range(int(2.1 / HOP))]
    last = pattern[int(LAST_FROM / HOP) :]
    e = envelope(at, w, w + SEEK + 2.1)
    lag, c = match(e, pattern, range(int(SEEK / HOP) + 1))
    if lag is None or c < FOUND:
        return None
    near = int(AGREE / HOP)
    tail = e[int(LAST_FROM / HOP) :]
    end, c2 = match(tail, last, range(max(0, lag - near), lag + near + 1))
    if end is None or c2 < FOUND:
        return None
    return lag * HOP, end * HOP


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
            + f" after the unmute (at most {LATE:.0f}): the unmute took that long, the "
            "microphone's start or an offer and answer; what a said before is not judged"
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
        held = True  # every gap judged so far more than FLOOR_UNDER under the last
        for k, x in enumerate(g[:-1]):
            if x != x:
                continue
            a, b = GAPS[k]
            if x < -FLOOR_UNDER and held:
                problems.append(
                    f"unmute {n}: the room's noise {a:.2f}-{b:.2f} s after it went out {-x:.0f} dB "
                    f"under its later level (at most {FLOOR_UNDER:.0f}): held down, then back as "
                    "a step"
                )
            elif x < -FLOOR_DEEP:
                problems.append(
                    f"unmute {n}: the room's noise {a:.2f}-{b:.2f} s after it went out {-x:.0f} dB "
                    f"under its later level, after it had been back (at most {FLOOR_DEEP:.0f} there)"
                )
            elif x < -FLOOR_UNDER:
                notes.append(
                    f"unmute {n}: the room's noise {a:.2f}-{b:.2f} s after it {-x:.0f} dB under its "
                    f"later level after an earlier gap within {FLOOR_UNDER:.0f}: a pause's dip, not "
                    f"held down from the unmute (fails from {FLOOR_DEEP:.0f})"
                )
            held = held and x < -FLOOR_UNDER
    else:
        own = pct([x for x in words if x == x], 0.5)

        def tones(a, b):
            """b's tone (dBFS) in each 0.25 s from a to b, a step of 0.05 s apart, with where
            each starts."""
            steps = [a + k * 0.05 for k in range(max(1, int(round((b - a - 0.25) / 0.05)) + 1))]
            return [(tone_db(at, t, min(b, t + 0.25)), t) for t in steps]

        def loudest(a, b):
            return max(tones(a, b))

        # Before a's first word: from a's first sound at b to the word reaching it.
        first = (max(GAPS[0][0], 15.0 if came is None else came + 0.05), WORDS[0] + DELAY)
        if first[1] - first[0] >= 0.15:
            tone, t = loudest(*first)
            notes.append(f"unmute {n}: b's tone as a sent it before a's first word at most {tone:.0f} "
                         f"dBFS, {own - tone:.0f} dB under a's words ({t:.2f} s on)")
            if tone > own - TONE_UNDER:
                problems.append(
                    f"unmute {n}: b's tone went back to it at {tone:.0f} dBFS, {own - tone:.0f} dB "
                    f"under a's words, {t:.2f} s after the unmute, before a said anything (at least "
                    f"{TONE_UNDER:.0f}): a's echo canceller let it through"
                )
        else:
            notes.append(f"unmute {n}: a's sound reached b too late to judge b's tone before a's "
                         "first word")
        # After each word, to the next one reaching b (the last: to 15.5 s): WebRTC's double
        # talk, judged at its loudest only by TAIL_UNDER; and throughout, by TONE_UNDER: an
        # echo let through all the way from one word to the next is no moment of double talk.
        # Each from where the word was found at b (DELAY when it was not).
        found = [arrival(at, w) for w in WORDS]
        lost = [w for w, x in zip(WORDS, found) if x is None]
        if lost:
            notes.append(
                f"unmute {n}: a's word at " + ", ".join(f"{w:.1f}" for w in lost) + " s not found at "
                f"b: taken to reach it {DELAY:.2f} s later"
            )
        reach = [w + (DELAY if x is None else x[0]) for w, x in zip(WORDS, found)]
        ends = [w + WORD_END + (DELAY if x is None else x[1]) for w, x in zip(WORDS, found)]
        notes.append(
            f"unmute {n}: a's words reached b "
            + " ".join("?" if x is None else f"{x[0]:.2f}" for x in found)
            + " s after they were said"
        )
        spans = [tones(e, b) for e, b in zip(ends, reach[1:] + [GAPS[-1][1]])]
        tails = [max(x) for x in spans]
        for w, x in zip(WORDS, spans):
            low, t = min(x)
            if low > own - TONE_UNDER:
                problems.append(
                    f"unmute {n}: b's tone went back to it all the way from a's word at {w:.1f} s to "
                    f"the next, at {low:.0f} dBFS at the least, {own - low:.0f} dB under a's words (at "
                    f"least {TONE_UNDER:.0f}): a's echo canceller let it through"
                )
        notes.append(
            f"unmute {n}: b's tone after each of a's words (WebRTC's double talk, at least "
            f"{TAIL_UNDER:.0f} dB under) " + " ".join(f"{own - x:.0f}" for x, _ in tails)
            + " dB under a's words, at "
            + " ".join(f"{t - e:.2f}" for (_, t), e in zip(tails, ends))
            + " s after each word's end at b"
        )
        for w, (x, t) in zip(WORDS, tails):
            if x > own - TAIL_UNDER:
                problems.append(
                    f"unmute {n}: b's tone after a's word at {w:.1f} s came back at {x:.0f} dBFS, "
                    f"{own - x:.0f} dB under a's words (at least {TAIL_UNDER:.0f} there): louder "
                    "than the echo of it can be"
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
