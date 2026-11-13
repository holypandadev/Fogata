"""Writes a tone (48 kHz, mono, 16 bits) to the file named, for a lab microphone or a lab's
other application; or a voice that talks in turns with the other copy's; or waits for the
next period of the turns; or gates the lab's room echoes with the turns.

Usage: tone.py <file.wav> [hertz, default 440] [seconds, default 10]
       tone.py <file.wav> screen
       tone.py <file.wav> fan
       tone.py <file.wav> turns <slot 0|1> <steady hertz>
       tone.py wait
       tone.py room <loopback module of slot 0> <loopback module of slot 1>

turns: a PERIOD of sound, played from each multiple of PERIOD seconds of the computer's
clock (`tone.py wait` sleeps until the next one), so every copy's player keeps the same
turns (TURNS below). The copy in slot 0 talks during [0, 1.5) and [4, 5.5) of each
period, the one in slot 1 during [2, 3.5) and [6, 7.5): noise from 300 Hz to 2.5 kHz whose
loudness rises and falls 4 times a second, as speech does (an echo canceller finds the
echo of such a sound at once; one of a pure tone it may never place). Then each holds a
steady tone alone for 4 s, slot 0 from 8 s, slot 1 from 12 s, which mic_check.py tells as
each copy sends it (WebRTC's noise suppressor, which the application turns off, pushes one
down now and then; an echo canceller removes one the other plays back to it).

screen: another application's sound for a shared screen to take (lab_apps_tone): noise
from 5.3 to 6.5 kHz, above the voices, steady in level, 60 s. Its echo in a room, unlike a
steady tone's, is one an echo canceller finds and removes (a pure tone, looped into the
watcher's microphone, went back to the sharer at -9 dBFS and left the cancellers
suppressing every voice).

fan: a steady noise as a fan's in a microphone (mic.sh noise): noise from 80 Hz to 2 kHz,
steady in level, 60 s. A noise suppressor takes it away; echo cancelling and gain control
leave it as it is.

room: what each copy plays goes back into its own microphone (lab.sh's lab_room_echo)
only while the other talks (not while either holds its steady tone, which an echo
canceller cannot place, and which must reach the other untouched by one): a copy's own voice never reaches its own microphone through
its own loop, so each echo canceller has the other's voice as its far end and only its
echo in the microphone, never its own voice echoed back to it from the other side (a
chain of the two cancellers, where each takes its own voice for an echo and removes it).
"""

import json
import math
import random
import struct
import subprocess
import sys
import time
import wave

RATE = 48000
PERIOD = 16.0
TALK = 1.5
# (start, seconds) of each slot's turns: talking, then the steady tone.
TURNS = (((0.0, TALK), (4.0, TALK)), ((2.0, TALK), (6.0, TALK)))
STEADY = ((8.0, 4.0), (12.0, 3.9))
VOICE_RMS = 0.08
STEADY_AMPLITUDE = 0.1
FADE = 0.02
# Each copy's loop is on while the other talks or holds its tone, and for the call's
# delay after (under ECHO_AFTER seconds), never while it talks itself.
ECHO_AFTER = 0.45
SCREEN_RMS = 0.05
SCREEN_SECONDS = 60
FAN_RMS = 0.05
FAN_SECONDS = 60


def write(path, samples):
    with wave.open(path, "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        out.writeframes(
            b"".join(struct.pack("<h", int(32767 * max(-1.0, min(1.0, x)))) for x in samples)
        )


def biquad(kind, hertz):
    """A second-order Butterworth low-pass or high-pass filter (b, a), at RATE."""
    w = 2 * math.pi * hertz / RATE
    c = math.cos(w)
    alpha = math.sin(w) / (2 * math.sqrt(0.5))
    b = [(1 - c) / 2, 1 - c, (1 - c) / 2] if kind == "low" else [(1 + c) / 2, -(1 + c), (1 + c) / 2]
    a0 = 1 + alpha
    return [x / a0 for x in b], [-2 * c / a0, (1 - alpha) / a0]


def band_noise(seconds, seed, low, high, order=4):
    """Noise from `low` to `high` Hz (Butterworth of `order` on either side), RMS 1."""
    rng = random.Random(seed)
    stages = [biquad("high", low)] * (order // 2) + [biquad("low", high)] * (order // 2)
    state = [[0.0, 0.0, 0.0, 0.0] for _ in stages]
    out = []
    for _ in range(int(seconds * RATE)):
        x = rng.gauss(0.0, 1.0)
        for (b, a), z in zip(stages, state):
            y = b[0] * x + b[1] * z[0] + b[2] * z[1] - a[0] * z[2] - a[1] * z[3]
            z[1], z[0], z[3], z[2] = z[0], x, z[2], y
            x = y
        out.append(x)
    rms = math.sqrt(sum(x * x for x in out) / len(out))
    return [x / rms for x in out]


def noise(seconds, seed):
    """A voice's noise: 300 Hz to 2.5 kHz, RMS 1; next to nothing from 5 kHz up, where the
    screen's sound is."""
    return band_noise(seconds, seed, 300, 2500)


def turns(slot, steady):
    """One period of the copy in `slot`, but the last 0.1 s (the player starting again
    takes a moment)."""
    length = PERIOD - 0.1
    out = [0.0] * int(length * RATE)
    for number, (start, talk) in enumerate(TURNS[slot]):
        sound = noise(talk, seed=100 * slot + number)
        for i, x in enumerate(sound):
            into = i / RATE
            fade = min(1.0, into / FADE, (talk - into) / FADE)
            envelope = 0.6 + 0.4 * math.cos(2 * math.pi * 4 * into)
            out[int(start * RATE) + i] += fade * VOICE_RMS * envelope * x / 0.6
    start, seconds = STEADY[slot]
    for i in range(int(seconds * RATE)):
        into = i / RATE
        fade = min(1.0, into / FADE, (seconds - into) / FADE)
        out[int(start * RATE) + i] += (
            fade * STEADY_AMPLITUDE * math.sin(2 * math.pi * steady * (start + into))
        )
    return out


def loop_on(slot, at):
    """Whether the loop of the copy in `slot` is on at `at` seconds into a period: while the
    other copy talks, and ECHO_AFTER after."""
    return any(
        start <= at < start + seconds + ECHO_AFTER for start, seconds in TURNS[1 - slot]
    )


def screen():
    """Noise from 5.3 to 6.5 kHz (eighth order on either side: 50 dB down by 2.6 kHz,
    where the voices are measured), RMS SCREEN_RMS, SCREEN_SECONDS long."""
    return [SCREEN_RMS * x for x in band_noise(SCREEN_SECONDS, 7, 5300, 6500, order=8)]


def fan():
    """Noise from 80 Hz to 2 kHz (second order on either side), RMS FAN_RMS, FAN_SECONDS
    long."""
    return [FAN_RMS * x for x in band_noise(FAN_SECONDS, 11, 80, 2000, order=2)]


def sink_input_of(module):
    """The sink input a loopback module plays into its sink through, or None yet."""
    out = subprocess.run(
        ["pactl", "-f", "json", "list", "sink-inputs"], capture_output=True, text=True
    ).stdout
    try:
        inputs = json.loads(out or "[]")
    except json.JSONDecodeError:
        return None
    for entry in inputs:
        if str(entry.get("owner_module")) == str(module):
            return str(entry.get("index"))
    return None


def room(modules):
    inputs = [None, None]
    muted = [None, None]
    while True:
        at = time.time() % PERIOD
        for slot in (0, 1):
            if inputs[slot] is None:
                inputs[slot] = sink_input_of(modules[slot])
                if inputs[slot] is None:
                    continue
                # What lab.sh's room.log tells: each loop found, and every change after.
                print(f"{time.time():.3f} slot {slot}: module {modules[slot]} plays "
                      f"through sink input {inputs[slot]}", flush=True)
                muted[slot] = None
            mute = not loop_on(slot, at)
            if mute != muted[slot]:
                done = subprocess.run(
                    ["pactl", "set-sink-input-mute", inputs[slot], "1" if mute else "0"],
                    capture_output=True,
                )
                if done.returncode == 0:
                    muted[slot] = mute
                    print(f"{time.time():.3f} slot {slot}: {'off' if mute else 'on'}", flush=True)
                else:
                    # Made again (a sound server restarted): looked up again.
                    inputs[slot] = None
        time.sleep(0.02)


if sys.argv[1] == "wait":
    time.sleep(PERIOD - time.time() % PERIOD)
elif sys.argv[1] == "room":
    room(sys.argv[2:4])
elif len(sys.argv) > 2 and sys.argv[2] == "screen":
    write(sys.argv[1], screen())
elif len(sys.argv) > 2 and sys.argv[2] == "fan":
    write(sys.argv[1], fan())
elif len(sys.argv) > 2 and sys.argv[2] == "turns":
    write(sys.argv[1], turns(int(sys.argv[3]), float(sys.argv[4])))
else:
    pitch = float(sys.argv[2]) if len(sys.argv) > 2 else 440.0
    seconds = int(sys.argv[3]) if len(sys.argv) > 3 else 10
    write(
        sys.argv[1],
        [0.3 * math.sin(2 * math.pi * pitch * i / RATE) for i in range(seconds * RATE)],
    )
