#!/usr/bin/env python3
"""Tests of the call lab's unmute judge (tools/call-lab/unmute_check.py). With a headset: the
room's noise between a's words held down from the unmute on fails it, a later pause's dip
is noted, and only a deep one fails. With speakers: b's tone after each word is measured
from where the word ended at b, however late it came, not on the word's last syllable.
Each runs the real judge on a run folder made for the test: b's output as one unmute,
tones at the levels asked, no WebRTC log.

    python3 -I tools/test_unmute_check.py
"""

import math
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest
import wave

sys.dont_write_bytecode = True  # no __pycache__ in the repository
JUDGE = pathlib.Path(__file__).resolve().parent / "call-lab" / "unmute_check.py"
RATE = 48000
START = 1_800_000_000.0  # when b's recording starts (s since the epoch)
UNMUTE = 0.5  # s into it that a unmutes, its sound there at once
GAPS = [(0.25, 0.5), (2.65, 3.5), (5.65, 6.5), (8.65, 9.5), (11.65, 12.5), (14.65, 15.5)]
WORDS = [0.5, 3.5, 6.5, 9.5, 12.5]
LAST = -50.0  # dBFS: the room's noise in the last gap
SPOKEN = -20.0  # dBFS: a's words


def folder(tmp, under):
    """A run folder whose gaps are `under` dB against the last (one per gap before it)."""
    run = tmp / "run"
    (run / "meet").mkdir(parents=True)
    levels = [LAST + x for x in under] + [LAST]

    def level(t):
        if any(w <= t < w + 2.1 for w in WORDS):
            return SPOKEN
        # The pause after a word takes the level of the gap in it (or the next one's).
        return next((x for (_, b), x in zip(GAPS, levels) if t < b), LAST)

    frames = bytearray()
    for i in range(int((UNMUTE + 16.0) * RATE)):
        t = i / RATE - UNMUTE
        amp = 0.0 if t < 0 else 32768 * math.sqrt(2) * 10 ** (level(t) / 20)
        frames += int(amp * math.sin(2 * math.pi * 300 * i / RATE)).to_bytes(2, "little", signed=True)
    with wave.open(str(run / "played-b.wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(bytes(frames))
    (run / "played-b.start").write_text(f"{START}\n")
    (run / "meet" / "mark-a-mute-1").write_text(f"{int((START - 10) * 1000)}\n")
    (run / "meet" / "mark-a-unmute-1").write_text(f"{int((START + UNMUTE) * 1000)}\n")
    return run


SYLLABLES = [(0.00, 0.18), (0.20, 0.22), (0.65, 0.15), (0.82, 0.25), (1.35, 0.16), (1.53, 0.14), (1.70, 0.30)]


def speakers_folder(tmp, late, shaped=True):
    """A run folder of unmute:speakers: b's tone (220 Hz) back at b 50 dB under a's words,
    which reach b `late` s after they were said: syllables of 1300 Hz, the last one 220 Hz
    as loud as the words (a pitch falling through the tone's), or with `shaped` false one
    steady sound of 2.1 s."""
    run = tmp / "run"
    (run / "meet").mkdir(parents=True)
    amp = lambda dbfs: 32768 * math.sqrt(2) * 10 ** (dbfs / 20)
    word, tone, voice, quiet = amp(-16), amp(-66), amp(-16), amp(-30)
    frames = bytearray()
    for i in range(int((UNMUTE + 16.0) * RATE)):
        t = i / RATE - UNMUTE
        v = 0.0
        if t >= 0:
            v = tone * math.sin(2 * math.pi * 220 * i / RATE)
            u = next((t - w - late for w in WORDS if 0 <= t - w - late < 2.1), None)
            if u is not None and not shaped:
                v += word * math.sin(2 * math.pi * 1300 * i / RATE)
            elif u is not None:
                for k, (s, d) in enumerate(SYLLABLES):
                    if s <= u < s + d:
                        env = math.sin(math.pi * (u - s) / d) ** 2
                        last = k == len(SYLLABLES) - 1
                        v += (quiet if last else word) * env * math.sin(2 * math.pi * 1300 * i / RATE)
                        v += voice * env * math.sin(2 * math.pi * 220 * i / RATE) if last else 0.0
        frames += max(-32768, min(32767, int(v))).to_bytes(2, "little", signed=True)
    with wave.open(str(run / "played-b.wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(bytes(frames))
    (run / "played-b.start").write_text(f"{START}\n")
    (run / "meet" / "mark-a-mute-1").write_text(f"{int((START - 10) * 1000)}\n")
    (run / "meet" / "mark-a-unmute-1").write_text(f"{int((START + UNMUTE) * 1000)}\n")
    return run


class TailAfterWords(unittest.TestCase):
    """With speakers, what follows each word is judged from where the word ended at b."""

    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="fogata-unmute-test-"))

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def judge(self, late, shaped=True):
        res = subprocess.run([sys.executable, "-I", str(JUDGE), str(speakers_folder(self.tmp, late, shaped)), "speakers"],
                             capture_output=True, text=True)
        self.assertIn(res.returncode, (0, 1), res.stderr)
        return [l.strip() for l in res.stdout.splitlines()]

    def tails(self, lines):
        line = next(l for l in lines if "b's tone after each of a's words" in l)
        under = line.split(") ", 1)[1].split(" dB under")[0]
        return [int(x) for x in under.split()]

    def test_words_late_are_judged_from_their_end_at_b(self):
        lines = self.judge(0.25)
        self.assertEqual(lines[0], "PASS unmute:speakers", lines)
        line = next(l for l in lines if "a's words reached b" in l)
        found = [float(x) for x in line.split("reached b ")[1].split(" s after")[0].split()]
        self.assertTrue(all(abs(x - 0.25) <= 0.02 for x in found), line)
        # From the end the tone is as far under as b's tone itself (50 dB), not the word's
        # last syllable read at 220 Hz (a few dB under, from DELAY).
        self.assertTrue(all(x >= 45 for x in self.tails(lines)), lines)

    def test_a_last_syllable_is_not_taken_for_the_tone(self):
        lines = self.judge(0.40)
        self.assertEqual(lines[0], "PASS unmute:speakers", lines)
        self.assertFalse([l for l in lines if "problem:" in l], lines)

    def test_a_word_not_found_is_taken_at_the_least_delay(self):
        lines = self.judge(0.10, shaped=False)
        self.assertIn("unmute 1: a's word at 0.5, 3.5, 6.5, 9.5, 12.5 s not found at b: taken to reach it "
                      "0.10 s later", lines)
        self.assertIn("unmute 1: a's words reached b ? ? ? ? ? s after they were said", lines)


class FloorAfterUnmute(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="fogata-unmute-test-"))

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def judge(self, under):
        res = subprocess.run([sys.executable, "-I", str(JUDGE), str(folder(self.tmp, under)), "headset"],
                             capture_output=True, text=True)
        self.assertIn(res.returncode, (0, 1), res.stderr)
        lines = res.stdout.splitlines()
        return lines[0], [l for l in lines if "problem:" in l], [l for l in lines if "a pause's dip" in l]

    def test_steady_passes(self):
        verdict, problems, dips = self.judge([1, -2, 0, -1, 1])
        self.assertEqual((verdict, problems, dips), ("PASS unmute:headset", [], []))

    def test_held_down_from_the_unmute_fails_while_it_holds(self):
        verdict, problems, dips = self.judge([-11, -8, -7, -3, -1])
        self.assertEqual(verdict, "FAIL unmute:headset")
        self.assertEqual(len(problems), 3, problems)
        self.assertTrue(all("held down, then back as a step" in p for p in problems), problems)
        self.assertIn("0.25-0.50 s after it went out 11 dB", problems[0])
        self.assertIn("5.65-6.50 s after it went out 7 dB", problems[2])

    def test_held_down_in_the_first_gap_alone_fails(self):
        verdict, problems, _ = self.judge([-7, -2, 0, -7, 0])
        self.assertEqual(verdict, "FAIL unmute:headset")
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("0.25-0.50 s after it went out 7 dB", problems[0])

    def test_a_later_dip_is_noted(self):
        verdict, problems, dips = self.judge([0, 0, -7, 1, -1])
        self.assertEqual((verdict, problems), ("PASS unmute:headset", []))
        self.assertEqual(len(dips), 1)
        self.assertIn("5.65-6.50 s after it 7 dB under", dips[0])

    def test_a_deep_later_dip_fails(self):
        verdict, problems, _ = self.judge([0, -1, -11, 0, 0])
        self.assertEqual(verdict, "FAIL unmute:headset")
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("5.65-6.50 s after it went out 11 dB under its later level, after it had been back", problems[0])


if __name__ == "__main__":
    unittest.main()
