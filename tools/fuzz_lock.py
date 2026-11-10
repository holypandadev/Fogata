#!/usr/bin/env python3
"""fuzz/Cargo.lock against Cargo.lock: the fuzz targets must fuzz the crates the app ships.

    python3 -I tools/fuzz_lock.py [<root>]

fuzz/ is a workspace of its own with a lock of its own, which cargo moves on its own schedule
(a `cargo update` in one workspace leaves the other alone). Every crate of the fuzz lock that
the main lock has too must be there at a version (and from a source) the main lock has: a
crate only the fuzz workspace uses (libfuzzer-sys, arbitrary...) may be anything, and the
main lock may hold versions the fuzz workspace never needs. Fails, naming each crate, when a
version of the fuzz lock is not in the main one; `cargo update --manifest-path
fuzz/Cargo.toml -p <crate> --precise <version>` moves it.

Reads the locks as plain text (no tomllib: macOS's own Python 3.9 runs this too).
"""

import pathlib
import re
import sys

sys.dont_write_bytecode = True  # no __pycache__ in the repository


def packages(text):
    """{name: {(version, source)}} of a Cargo.lock; a path crate's source is "path"."""
    found = {}
    for block in text.split("[[package]]")[1:]:
        fields = dict(re.findall(r'^(name|version|source) = "([^"]*)"$', block, re.M))
        if "name" not in fields or "version" not in fields:
            raise ValueError("a [[package]] without its name or version")
        found.setdefault(fields["name"], set()).add(
            (fields["version"], fields.get("source", "path")))
    return found


def differences(main_text, fuzz_text):
    """The lines naming each crate of the fuzz lock at a version the main lock lacks."""
    main, fuzz = packages(main_text), packages(fuzz_text)
    lines = []
    for name in sorted(set(main) & set(fuzz)):
        extra = fuzz[name] - main[name]
        if extra:
            have = ", ".join(sorted(v for v, _ in main[name]))
            for version, source in sorted(extra):
                lines.append(f"{name} {version} ({source}) is in fuzz/Cargo.lock; "
                             f"Cargo.lock has {have}")
    return lines


def main():
    root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else
                        pathlib.Path(__file__).resolve().parent.parent)
    lines = differences((root / "Cargo.lock").read_text(),
                        (root / "fuzz/Cargo.lock").read_text())
    if lines:
        print("fuzz/Cargo.lock holds crates at versions the app does not ship:", file=sys.stderr)
        for line in lines:
            print("  " + line, file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
