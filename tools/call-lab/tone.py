"""Writes a 10 s tone (48 kHz, mono, 16 bits) to the file named, for a lab microphone.
Usage: tone.py <file.wav> [hertz, default 440]"""

import math
import struct
import sys
import wave

RATE = 48000
PITCH = float(sys.argv[2]) if len(sys.argv) > 2 else 440.0
with wave.open(sys.argv[1], "wb") as out:
    out.setnchannels(1)
    out.setsampwidth(2)
    out.setframerate(RATE)
    out.writeframes(
        b"".join(
            struct.pack("<h", int(0.3 * 32767 * math.sin(2 * math.pi * PITCH * i / RATE)))
            for i in range(10 * RATE)
        )
    )
