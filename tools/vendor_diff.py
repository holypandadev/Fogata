#!/usr/bin/env python3
"""What Fogata changed in each vendored crate and each forked Dart package, against the one
published on crates.io or pub.dev.

    python3 -I tools/vendor_diff.py [--diff <file>] [<crate>...]

For each folder of vendor/ (or the crates named), downloads the crate at the version its
Cargo.toml records from crates.io, unpacks it in a temporary folder, and compares it with
vendor/<name>: the files changed, added and removed, each changed file checked against the
crate's FOGATA.md, which must name it (a change no note explains is reported), and the newest
version crates.io has, so a fork that fell behind is seen. Without crates named, the same for
each package of app/third_party (flutter_webrtc, tray_manager, desktop_drop), against pub.dev
at the version its pubspec.yaml records, a fork that fell behind also as a warning in the
run's annotations. A fork whose FOGATA.md has a line "Reviewed against <name> <version>"
was read against that newer version (what it brings taken or left out, each said there):
while it is the newest one, the fork is reported as reviewed, not behind, and gives no
warning. --diff writes the whole diff (`diff -ruN`) there. It only reports: the
exit status is 0 unless a package cannot be compared at all (the registry unreachable, a
checksum that does not match, an archive that is not one).

The published crate is checked against the SHA-256 crates.io's index records for that
version, a Dart package against the one pub.dev's API gives; the archive is unpacked by this
script, member by member, refusing links and paths that climb out. Needs the network; run by
.github/workflows/weekly.yml.
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
    """The files of the archive `data` (a crate's: its members under `prefix/`; a pub
    package's, with `prefix` None: its members at the top), written into `into`: plain files
    and folders only, never a link or a path that climbs out."""
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for member in archive.getmembers():
            path = pathlib.PurePosixPath(member.name)
            parts = [p for p in path.parts if p != "."]
            if prefix is not None:
                if parts[:1] != [prefix]:
                    raise SystemExit(f"vendor_diff.py: {member.name} is outside the crate")
                parts = parts[1:]
            if ".." in parts or path.is_absolute():
                raise SystemExit(f"vendor_diff.py: {member.name} is outside the package")
            if member.isdir() or not parts:
                continue
            if not member.isfile():
                raise SystemExit(f"vendor_diff.py: {member.name} is not a plain file")
            target = into.joinpath(*parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(archive.extractfile(member).read())


def reviewed(folder, name):
    """The newer version of `name` the fork in `folder` was read against, as its FOGATA.md
    says ("Reviewed against <name> <version>" at the start of a line; the last one), or
    None."""
    notes = folder / "FOGATA.md"
    if not notes.is_file():
        return None
    line = rf"^Reviewed against {re.escape(name)} (\d[\w.+-]*\w)"
    found = re.findall(line, notes.read_text(), re.M)
    return found[-1] if found else None


def behind(registry, version, newest, seen):
    """What the report says of a fork of `version` when `registry` has `newest` and the fork
    was read against `seen` (or None), and whether that is worth a warning: a newer version
    than the fork's that nobody read the fork against."""
    if newest in (None, version):
        return "", False
    if newest == seen:
        return f"; {registry} has {newest}, reviewed against it (FOGATA.md)", False
    also = f", reviewed against {seen} only" if seen else ""
    return f"; {registry} has {newest}{also}", True


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
        late, _ = behind("crates.io", version, newest, reviewed(folder, vendored_name))
        return report(f"vendor/{name}", f"{vendored_name} {version}{late}", folder, upstream,
                      out, "src/")


def report(where, what, folder, upstream, out, code):
    """Prints what differs in the fork `folder` (`where` in the repository) from `upstream`,
    as published (`what`: its name and version), adds the diff to `out`; returns the number
    of changed files FOGATA.md does not name. A file removed outside the folder `code` (the
    package's own code) needs no note."""
    ours, theirs = files(folder), files(upstream)
    changed = sorted(p for p in ours & theirs
                     if (folder / p).read_bytes() != (upstream / p).read_bytes())
    added, removed = sorted(ours - theirs), sorted(theirs - ours)
    for path in changed + added + removed:
        old = (upstream / path).read_text(errors="replace").splitlines(True) \
            if path in theirs else []
        new = (folder / path).read_text(errors="replace").splitlines(True) \
            if path in ours else []
        out.writelines(difflib.unified_diff(old, new, f"a/{where}/{path}", f"b/{where}/{path}"))
    notes = folder / "FOGATA.md"
    told = notes.read_text() if notes.is_file() else ""
    # A note may name several files at once, as `linux/x.{h,cc}`.
    told += "\n".join(f"{stem}.{ending}" for stem, endings in
                      re.findall(r"([\w./-]+)\.\{([\w.,]+)\}", told)
                      for ending in endings.split(","))
    print(f"{where}: {what}: {len(changed)} changed, {len(added)} added, {len(removed)} removed")
    unexplained = 0
    for kind, paths in (("changed", changed), ("added", added), ("removed", removed)):
        for path in paths:
            # A file is explained when FOGATA.md names it (as `src/x.rs` or `x.rs`). A licence
            # added, or a file left out that is no code of the package (changelogs, tests,
            # benches, examples), needs no note.
            named = (path in told or pathlib.PurePosixPath(path).name in told
                     or path.startswith("LICENSE")
                     or (kind == "removed" and not path.startswith(code)))
            unexplained += 0 if named else 1
            print(f"  {kind:8} {path}{'' if named else '   (not named in FOGATA.md)'}")
    if not notes.is_file():
        print(f"  no FOGATA.md in {where}")
    return unexplained


def pub_recorded(folder):
    """The name and version the fork's pubspec.yaml records."""
    text = (folder / "pubspec.yaml").read_text()
    name = re.search(r"^name: *(\S+)", text, re.M)
    version = re.search(r"^version: *(\S+)", text, re.M)
    if not name or not version:
        raise SystemExit(f"vendor_diff.py: no name or version in {folder}/pubspec.yaml")
    return name.group(1), version.group(1)


def pub_published(info):
    """{version: (archive URL, its SHA-256)} of every version pub.dev's API answer `info`
    lists, and the newest one (pub.dev's `latest`)."""
    versions = {v["version"]: (v["archive_url"], v.get("archive_sha256"))
                for v in info.get("versions", [])}
    return versions, info.get("latest", {}).get("version")


def compare_pub(folder, out):
    """The same as compare, for a fork of a Dart package in app/third_party, against the
    package pub.dev publishes at the version its pubspec.yaml records (checked against the
    SHA-256 pub.dev's API gives for that archive)."""
    name, version = pub_recorded(folder)
    versions, newest = pub_published(json.loads(fetch(f"https://pub.dev/api/packages/{name}")))
    if version not in versions:
        raise SystemExit(f"vendor_diff.py: pub.dev has no {name} {version}")
    url, digest = versions[version]
    data = fetch(url)
    if not digest or hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit(f"vendor_diff.py: {name} {version} does not match the checksum "
                         "pub.dev records")
    where = folder.relative_to(ROOT).as_posix()
    late, warn = behind("pub.dev", version, newest, reviewed(folder, name))
    if warn:
        # Seen in the run's annotations too, not only in its log.
        print(f"::warning::{where} is a fork of {name} {version}{late}: read its changes for "
              "fixes to take (FOGATA.md lists what Fogata changed), and say in FOGATA.md "
              f'"Reviewed against {name} {newest}" once read')
    with tempfile.TemporaryDirectory(prefix="fogata-vendor-") as tmp:
        upstream = pathlib.Path(tmp) / "upstream"
        unpack(data, upstream, None)
        return report(where, f"{name} {version}{late}", folder, upstream, out, "lib/")


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
    # Without crates named, the forks of Dart packages too.
    if not args.crates:
        for folder in sorted((ROOT / "app/third_party").iterdir()):
            if (folder / "pubspec.yaml").is_file():
                unexplained += compare_pub(folder, out)
    if args.diff:
        args.diff.write_text(out.getvalue())
    if unexplained:
        print(f"{unexplained} file(s) differ from the published packages without a note in "
              "FOGATA.md")


if __name__ == "__main__":
    main()
