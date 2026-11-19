#!/usr/bin/env python3
"""Tests of the call lab's unmute judge with a headset (tools/call-lab/unmute_check.py): the
room's noise between a's words held down from the unmute on fails it, a later pause's dip
is noted, and only a deep one fails. Each runs the real judge on a run folder made for the
test: b's output as one unmute, steady tones at the levels asked, no WebRTC log.

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
