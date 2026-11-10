#!/usr/bin/env python3
"""What Fogata changed in each vendored crate, against the crate published on crates.io.

    python3 -I tools/vendor_diff.py [--diff <file>] [<crate>...]

For each folder of vendor/ (or the crates named), downloads the crate at the version its
Cargo.toml records from crates.io, unpacks it in a temporary folder, and compares it with
vendor/<name>: the files changed, added and removed, each changed file checked against the
crate's FOGATA.md, which must name it (a change no note explains is reported), and the newest
version crates.io has, so a fork that fell behind is seen. --diff writes the whole diff
(`diff -ruN`) there. It only reports: the exit status is 0 unless a crate cannot be compared
at all (crates.io unreachable, a checksum that does not match, an archive that is not one).

The published crate is checked against the SHA-256 crates.io's index records for that
version; the archive is unpacked by this script, member by member, refusing links and paths
that climb out. Needs the network; run by .github/workflows/weekly.yml.
"""

import argparse
import difflib
import hashlib
import io
import json
import pathlib
import re
import sys
import tarfile
import tempfile
import urllib.request

sys.dont_write_bytecode = True  # no __pycache__ in the repository
ROOT = pathlib.Path(__file__).resolve().parent.parent
# What crates.io adds to a published crate, and Fogata's own notes: not part of the fork.
IGNORED = {".cargo_vcs_info.json", ".cargo-ok", "Cargo.toml.orig", "Cargo.lock", "FOGATA.md"}
AGENT = {"User-Agent": "fogata-vendor-diff (https://github.com/holypandadev/Fogata)"}


def fetch(url):
    request = urllib.request.Request(url, headers=AGENT)
    with urllib.request.urlopen(request, timeout=60) as answer:
        return answer.read()


def index_path(name):
    """The crate's file in crates.io's sparse index."""
    name = name.lower()
    if len(name) <= 2:
        return f"{len(name)}/{name}"
    if len(name) == 3:
        return f"3/{name[0]}/{name}"
    return f"{name[:2]}/{name[2:4]}/{name}"


def published(name):
    """{version: checksum} of every version of `name` crates.io's index has, yanked or not,
    and the newest version not yanked."""
    versions, newest = {}, None
    for line in fetch(f"https://index.crates.io/{index_path(name)}").decode().splitlines():
        entry = json.loads(line)
        versions[entry["vers"]] = entry["cksum"]
        if not entry.get("yanked") and "-" not in entry["vers"]:
            newest = entry["vers"]  # the index lists versions in the order they came
    return versions, newest


def recorded(folder):
    """The name and version vendor/<folder>/Cargo.toml records."""
    text = (folder / "Cargo.toml").read_text()
    package = text.split("[package]", 1)[1]
    name = re.search(r'^name = "([^"]+)"', package, re.M)
    version = re.search(r'^version = "([^"]+)"', package, re.M)
    if not name or not version:
        raise SystemExit(f"vendor_diff.py: no name or version in {folder}/Cargo.toml")
    return name.group(1), version.group(1)


def unpack(data, into, prefix):
    """The files of the crate archive `data` (its members under `prefix/`), written into
    `into`: plain files and folders only, never a link or a path that climbs out."""
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for member in archive.getmembers():
            path = pathlib.PurePosixPath(member.name)
            if path.parts[:1] != (prefix,) or ".." in path.parts or path.is_absolute():
                raise SystemExit(f"vendor_diff.py: {member.name} is outside the crate")
            if member.isdir():
                continue
            if not member.isfile():
                raise SystemExit(f"vendor_diff.py: {member.name} is not a plain file")
            target = into.joinpath(*path.parts[1:])
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(archive.extractfile(member).read())


def files(folder):
    return {p.relative_to(folder).as_posix() for p in folder.rglob("*")
            if p.is_file() and p.name not in IGNORED}


def compare(name, folder, out):
    """Prints what differs in `folder` from `name` as published, adds the diff to `out`;
    returns the number of changed files FOGATA.md does not name."""
    vendored_name, version = recorded(folder)
    versions, newest = published(vendored_name)
    if version not in versions:
        raise SystemExit(f"vendor_diff.py: crates.io has no {vendored_name} {version}")
    data = fetch(f"https://static.crates.io/crates/{vendored_name}/"
                 f"{vendored_name}-{version}.crate")
    if hashlib.sha256(data).hexdigest() != versions[version]:
        raise SystemExit(f"vendor_diff.py: {vendored_name} {version} does not match the "
                         "checksum crates.io's index records")
    with tempfile.TemporaryDirectory(prefix="fogata-vendor-") as tmp:
        upstream = pathlib.Path(tmp) / "upstream"
        unpack(data, upstream, f"{vendored_name}-{version}")
        ours, theirs = files(folder), files(upstream)
        changed = sorted(p for p in ours & theirs
                         if (folder / p).read_bytes() != (upstream / p).read_bytes())
        added, removed = sorted(ours - theirs), sorted(theirs - ours)
        for path in changed + added + removed:
            old = (upstream / path).read_text(errors="replace").splitlines(True) \
                if path in theirs else []
            new = (folder / path).read_text(errors="replace").splitlines(True) \
                if path in ours else []
            out.writelines(difflib.unified_diff(old, new, f"a/{name}/{path}", f"b/{name}/{path}"))
    notes = folder / "FOGATA.md"
    told = notes.read_text() if notes.is_file() else ""
    behind = "" if newest in (None, version) else f"; crates.io has {newest}"
    print(f"{name}: {vendored_name} {version}{behind}: {len(changed)} changed, "
          f"{len(added)} added, {len(removed)} removed")
    unexplained = 0
    for kind, paths in (("changed", changed), ("added", added), ("removed", removed)):
        for path in paths:
            # A file is explained when FOGATA.md names it (as `src/x.rs` or `x.rs`). A licence
            # added, or a file left out that is no code of the crate (changelogs, tests,
            # benches, examples), needs no note.
            named = (path in told or pathlib.PurePosixPath(path).name in told
                     or path.startswith("LICENSE")
                     or (kind == "removed" and not path.startswith("src/")))
            unexplained += 0 if named else 1
            print(f"  {kind:8} {path}{'' if named else '   (not named in FOGATA.md)'}")
    if not notes.is_file():
        print(f"  no FOGATA.md in vendor/{name}")
    return unexplained


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--diff", type=pathlib.Path, help="write the whole diff there")
    parser.add_argument("crates", nargs="*")
    args = parser.parse_args()
    names = args.crates or sorted(p.name for p in (ROOT / "vendor").iterdir()
                                  if (p / "Cargo.toml").is_file())
    out = io.StringIO()
    unexplained = 0
    for name in names:
        unexplained += compare(name, ROOT / "vendor" / name, out)
    if args.diff:
        args.diff.write_text(out.getvalue())
    if unexplained:
        print(f"{unexplained} file(s) differ from the published crates without a note in "
              "FOGATA.md")


if __name__ == "__main__":
    main()
