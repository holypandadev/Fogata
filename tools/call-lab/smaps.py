"""Prints what a process's memory is made of now, in one line: its resident memory in all,
then each kind of mapping that holds 1 MB or more of it, largest first, in MB:

    rss=612 arena=231 anon-64M=40 [heap]=77 [anon:dart-heap]=51 libflutter_linux_gtk.so=38 ...

Kinds: a file by its name; a named anonymous mapping by its name ([heap], [stack], what the
Dart VM names); an unnamed anonymous one as "arena" when it is one of glibc's allocator
arenas beside the main one (they start on a multiple of 64 MB), else as "anon-<size>" by
the size of the mapping, rounded up to a power of two ("anon-8M": mappings of 4 to 8 MB).
A leak shows as the kind that keeps growing between two lines.

Usage: smaps.py <pid>
"""

import collections
import os
import re
import sys

ARENA = 64 << 20  # glibc's HEAP_MAX_SIZE on 64 bits: its other arenas are aligned to it.
HEADER = re.compile(r"^([0-9a-f]+)-([0-9a-f]+) (\S+) \S+ \S+ \S+\s*(.*)$")


def size_name(size):
    power = 4096
    while power < size:
        power *= 2
    for unit, name in ((1 << 30, "G"), (1 << 20, "M"), (1 << 10, "K")):
        if power >= unit:
            return f"{power // unit}{name}"
    return str(power)


def kinds(text):
    """{kind: resident kB} of a /proc/<pid>/smaps."""
    found = collections.Counter()
    kind = None
    for line in text.splitlines():
        header = HEADER.match(line)
        if header:
            start, end = int(header.group(1), 16), int(header.group(2), 16)
            name = header.group(4)
            if name.startswith("/"):
                kind = os.path.basename(name.removesuffix(" (deleted)"))
            elif name:
                kind = name
            elif "w" in header.group(3) and start % ARENA == 0:
                kind = "arena"
            else:
                kind = f"anon-{size_name(end - start)}"
        elif line.startswith("Rss:") and kind is not None:
            found[kind] += int(line.split()[1])
    return found


def line(text):
    found = kinds(text)
    parts = [f"rss={sum(found.values()) // 1024}"]
    parts += [f"{kind}={kb // 1024}" for kind, kb in found.most_common() if kb >= 1024]
    return " ".join(parts)


if __name__ == "__main__":
    try:
        with open(f"/proc/{sys.argv[1]}/smaps") as smaps:
            print(line(smaps.read()))
    except OSError as error:
        print(f"gone: {error.strerror}")
