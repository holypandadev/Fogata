#!/usr/bin/env python3
"""The dependencies between the workspace's own crates, against the layers they are allowed.

    python3 -I tools/crate_layers.py            # runs cargo metadata --offline --no-deps
    python3 -I tools/crate_layers.py <metadata.json>

The layers (docs/architecture.md, "The crates and their layers"): a crate may use only crates
of a layer below its own, and only the ones ALLOWED names for it, so a new edge between two
crates (fogata-net reaching for fogata-media, say, or the bridge going around fogata-core
for one more crate) is a change of this file, made on purpose, not something that happens
in a Cargo.toml. A new crate of the workspace must be given its layer and edges here too.
Development dependencies (tests) are listed apart: they may reach further down or across
(fogata-state's tests compare it with its frozen copy, fogata-state-before), never up.
"""

import json
import pathlib
import subprocess
import sys

sys.dont_write_bytecode = True  # no __pycache__ in the repository

# Each crate's layer: what it is built on is below it.
LAYERS = {
    # Pieces of their own, on nothing of Fogata's.
    "fogata-crypto": 0,
    "fogata-denoise": 0,
    "fogata-tray": 0,
    "fogata-voice": 0,
    # What is said and kept: wire formats, the group log, friends, releases.
    "fogata-proto": 1,
    "fogata-state": 1,
    "fogata-friends": 1,
    "fogata-update": 1,
    "fogata-state-before": 1,  # the frozen level-one copy of fogata-state, for its tests
    "fogata-mls": 2,
    # The node: transport, groups, friends and files over the network.
    "fogata-net": 3,
    # Calls on top of the node.
    "fogata-media": 4,
    # The application's logic, which the programs drive.
    "fogata-core": 5,
    # The programs: the command line and the bridge of the Flutter app.
    "fogata": 6,
    "fogata-ffi": 6,
}

# The edges each crate may have (normal and build dependencies), today's: a new one goes here.
ALLOWED = {
    "fogata-crypto": set(),
    "fogata-denoise": set(),
    "fogata-tray": set(),
    "fogata-voice": set(),
    "fogata-proto": {"fogata-crypto"},
    "fogata-state": {"fogata-crypto"},
    "fogata-friends": {"fogata-crypto"},
    "fogata-update": {"fogata-crypto"},
    "fogata-state-before": {"fogata-crypto"},
    "fogata-mls": {"fogata-crypto", "fogata-state"},
    "fogata-net": {"fogata-crypto", "fogata-friends", "fogata-proto", "fogata-state",
                   "fogata-update"},
    "fogata-media": {"fogata-net"},
    "fogata-core": {"fogata-crypto", "fogata-friends", "fogata-media", "fogata-mls",
                    "fogata-net", "fogata-proto", "fogata-state", "fogata-update",
                    "fogata-voice"},
    # The programs still reach some crates around fogata-core; no more without a reason.
    "fogata": {"fogata-core", "fogata-crypto", "fogata-friends", "fogata-media", "fogata-net",
               "fogata-proto", "fogata-state", "fogata-update"},
    "fogata-ffi": {"fogata-core", "fogata-crypto", "fogata-denoise", "fogata-friends",
                   "fogata-net", "fogata-tray", "fogata-voice"},
}

# Development dependencies (tests), beyond the edges above.
ALLOWED_DEV = {
    "fogata-state": {"fogata-state-before"},
    "fogata-media": {"fogata-crypto", "fogata-state"},
    "fogata-ffi": {"fogata-proto"},
}


def problems(metadata):
    """What breaks the layers in `metadata` (cargo metadata's JSON, as a dict)."""
    found = []
    members = {p["name"] for p in metadata["packages"]}
    for package in sorted(metadata["packages"], key=lambda p: p["name"]):
        name = package["name"]
        if name not in LAYERS or name not in ALLOWED:
            found.append(f"{name} is a crate of the workspace with no layer in "
                         "tools/crate_layers.py")
            continue
        for dependency in package["dependencies"]:
            other = dependency["name"]  # the package's own name, whatever it is renamed to
            if other not in members and other not in LAYERS:
                continue  # a crate from outside (crates.io, or vendor/ through a patch)
            dev = dependency.get("kind") == "dev"
            allowed = ALLOWED[name] | (ALLOWED_DEV.get(name, set()) if dev else set())
            if other not in allowed:
                kind = "a development dependency" if dev else "a dependency"
                found.append(f"{name} has {other} as {kind}, an edge tools/crate_layers.py "
                             "does not allow")
            elif LAYERS.get(other, -1) > LAYERS[name] or (
                    not dev and LAYERS.get(other, -1) == LAYERS[name]):
                found.append(f"{name} (layer {LAYERS[name]}) uses {other} "
                             f"(layer {LAYERS.get(other)}), which is not below it")
    return found


def main():
    if len(sys.argv) > 1:
        metadata = json.loads(pathlib.Path(sys.argv[1]).read_text())
    else:
        root = pathlib.Path(__file__).resolve().parent.parent
        out = subprocess.run(["cargo", "metadata", "--offline", "--format-version", "1",
                              "--no-deps"], cwd=root, check=True, capture_output=True, text=True)
        metadata = json.loads(out.stdout)
    found = problems(metadata)
    if found:
        print("the crates of the workspace leave their layers (docs/architecture.md):",
              file=sys.stderr)
        for line in found:
            print("  " + line, file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
