"""Passes on what heaptrack's library writes (standard input to standard output) until the
time it notes reaches <seconds>, then reads the rest without passing it on: what comes out
is the run as if it had ended then. Usage: heap_until.py <seconds>"""

import sys

until = int(sys.argv[1]) * 1000
out = sys.stdout.buffer
passing = True
for line in sys.stdin.buffer:
    if passing:
        out.write(line)
        # "c <milliseconds since the start, in hexadecimal>"
        if line.startswith(b"c ") and int(line[2:], 16) >= until:
            passing = False
            out.close()
