#!/usr/bin/env python3
"""The app's limits of what a person types, against the Rust ones they mirror.

    python3 -I tools/limits.py [<root>]

A message, a bio, a name: the Rust crates refuse what is longer (and other devices do too),
and the Dart UI stops the person typing at the same length. The two are written apart, so a
Rust limit changed alone would leave fields that take what the core then refuses, or stop
short of what it takes. This reads both and fails when they differ:

- the Dart constants of DART_CONSTANTS against the Rust constants they mirror;
- every `maxLength:` of a field in app/lib: a Dart constant above, or a number, given with
  the field's label, which FIELDS ties to a Rust constant. A new field with a number for its
  limit must be added to FIELDS, so that it is checked too.
"""

import pathlib
import re
import sys

sys.dont_write_bytecode = True  # no __pycache__ in the repository

# The Rust constants, and the file that defines each.
RUST = {
    "MAX_CHAT_CHARS": "crates/fogata-state/src/room.rs",
    "MAX_GROUP_NAME_CHARS": "crates/fogata-state/src/state.rs",
    "MAX_ROOM_NAME_CHARS": "crates/fogata-state/src/state.rs",
    "MAX_NAME_CHARS": "crates/fogata-friends/src/profile.rs",
    "MAX_BIO_CHARS": "crates/fogata-friends/src/bio.rs",
    "MAX_BIO_LINES": "crates/fogata-friends/src/bio.rs",
}

# Dart constants (file, name) and the Rust constant each mirrors.
DART_CONSTANTS = {
    ("app/lib/src/format.dart", "maxMessage"): "MAX_CHAT_CHARS",
    ("app/lib/src/names.dart", "maxBioChars"): "MAX_BIO_CHARS",
    ("app/lib/src/names.dart", "maxBioLines"): "MAX_BIO_LINES",
}

# A field whose maxLength is a number, by its label, and the Rust constant it mirrors.
FIELDS = {
    "l10n.newGroupName": "MAX_GROUP_NAME_CHARS",
    "l10n.groupSettingsNameField": "MAX_GROUP_NAME_CHARS",
    "l10n.groupSettingsRoomName": "MAX_ROOM_NAME_CHARS",
    "l10n.settingsName": "MAX_NAME_CHARS",
}


def rust_value(root, name):
    text = (root / RUST[name]).read_text()
    found = re.search(rf"^pub const {name}: usize = ([0-9_]+);", text, re.M)
    if not found:
        raise SystemExit(f"limits.py: no `pub const {name}: usize = ...;` in {RUST[name]}")
    return int(found.group(1).replace("_", ""))


def dart_value(root, path, name):
    text = (root / path).read_text()
    found = re.search(rf"^const {name} = ([0-9_]+);", text, re.M)
    if not found:
        raise SystemExit(f"limits.py: no `const {name} = ...;` in {path}")
    return int(found.group(1).replace("_", ""))


def problems(root):
    root = pathlib.Path(root)
    rust = {name: rust_value(root, name) for name in RUST}
    found = []
    constants = {}
    for (path, name), mirrored in DART_CONSTANTS.items():
        value = dart_value(root, path, name)
        constants[name] = mirrored
        if value != rust[mirrored]:
            found.append(f"{path}: {name} is {value}, {mirrored} ({RUST[mirrored]}) is "
                         f"{rust[mirrored]}")
    for path in sorted((root / "app/lib").rglob("*.dart")):
        lines = path.read_text().splitlines()
        where = path.relative_to(root)
        for number, line in enumerate(lines):
            limit = re.search(r"\bmaxLength:\s*([A-Za-z_][A-Za-z0-9_]*|[0-9_]+)\s*,?", line)
            if not limit:
                continue
            value = limit.group(1)
            if value in constants:
                continue  # checked above
            if not value.replace("_", "").isdigit():
                found.append(f"{where}:{number + 1}: maxLength {value} is no constant "
                             "tools/limits.py knows")
                continue
            # The field's label: on this line or the next ones of the same call.
            label = None
            for later in lines[number:number + 8]:
                named = re.search(r"\blabel:\s*(l10n\.[A-Za-z0-9_]+)", later)
                if named:
                    label = named.group(1)
                    break
            mirrored = FIELDS.get(label)
            if mirrored is None:
                found.append(f"{where}:{number + 1}: a field limited to {value} characters "
                             f"(label {label}) that tools/limits.py does not tie to a Rust limit")
            elif int(value.replace("_", "")) != rust[mirrored]:
                found.append(f"{where}:{number + 1}: {label} takes {value} characters, "
                             f"{mirrored} ({RUST[mirrored]}) is {rust[mirrored]}")
    return found


def main():
    root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else
                        pathlib.Path(__file__).resolve().parent.parent)
    found = problems(root)
    if found:
        print("the app's limits differ from the Rust ones they mirror:", file=sys.stderr)
        for line in found:
            print("  " + line, file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
