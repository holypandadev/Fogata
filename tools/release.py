#!/usr/bin/env python3
"""Checksums and their signature for the packages of a release.

    python3 tools/release.py sign --key <private key> <folder>
    python3 tools/release.py verify <folder>
    python3 tools/release.py verify --prerelease <folder>

The folder holds only the packages to hand out, each built in release mode, for example:

    Fogata-<version>-linux-x64.tar.gz
    Fogata-<version>-windows-x64.zip
    Fogata-<version>-macos.zip

`sign` looks inside every .zip and .tar archive and refuses a debug or profile build, writes
SHA256SUMS with the SHA-256 of every package, signs it with the release key (`ssh-keygen -Y
sign`, namespace "file") into SHA256SUMS.sig, and checks that signature against the public key
in tools/allowed_signers, which is the one people trust: it refuses to sign while that file
holds no key. ssh-keygen asks for the key's passphrase.

`verify` checks SHA256SUMS.sig against tools/allowed_signers and then every package's SHA-256,
as someone who downloaded the release does by hand (docs/releasing.md).

tools/allowed_signers names the key of public releases `fogata-release` and the keys of the
private beta `fogata-prerelease` (docs/releasing-beta.md). `sign` and `verify` take only a
`fogata-release` signature; `verify --prerelease` takes only a `fogata-prerelease` one, for a
folder of the beta's first installs. So a key of the beta, which lives on the publisher's
computer and signs every beta, can never vouch for a public release.

Both take --signers to check against another allowed signers file, for a test key.
"""

import argparse
import hashlib
import os
import pathlib
import re
import stat
import subprocess
import sys
import tarfile
import zipfile

REPOSITORY = pathlib.Path(__file__).resolve().parent.parent
SIGNERS = REPOSITORY / "tools" / "allowed_signers"
# The names the keys have in allowed signers files: the SSH key of public releases, and the
# keys of the beta (`fogata release keygen`); and what the signature is for.
PRINCIPAL = "fogata-release"
PRERELEASE_PRINCIPAL = "fogata-prerelease"
NAMESPACE = "file"
SUMS = "SHA256SUMS"
SIGNATURE = SUMS + ".sig"
# Names that read the same in SHA256SUMS, in a shell and on every system.
PACKAGE_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]*")
# Flutter puts the Dart program as kernel code, run by the VM's JIT, only into debug builds;
# release builds carry it compiled ahead of time (libapp.so, app.so, App.framework).
DEBUG_MARKER = "kernel_blob.bin"
# Flutter's engine library, as each system's package carries it. Profile builds are compiled
# ahead of time like release ones, but their engine runs the Dart VM service, which lets any
# local program run code inside the app; only that engine names it.
ENGINES = ("libflutter_linux_gtk.so", "flutter_windows.dll", "FlutterMacOS")
PROFILE_MARKER = b"Dart VM service"


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as file:
        for block in iter(lambda: file.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def archive_names(path):
    """The names inside a .zip or .tar archive, or None for any other kind of file."""
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            return archive.namelist()
    try:
        with tarfile.open(path) as archive:
            return archive.getnames()
    except (tarfile.TarError, OSError):
        return None


def archive_file(path, name):
    """The bytes of the plain file `name` inside a .zip or .tar archive, or None."""
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            return archive.read(name)
    with tarfile.open(path) as archive:
        member = archive.getmember(name)
        if not member.isfile():
            return None
        return archive.extractfile(member).read()


def packages(folder):
    """The packages in the folder, by name; anything else in it stops the release."""
    names = []
    for entry in sorted(folder.iterdir()):
        if entry.name in (SUMS, SIGNATURE):
            continue
        if not entry.is_file() or entry.is_symlink():
            sys.exit(f"{entry} is not a plain file: the folder must hold only the packages")
        if not PACKAGE_NAME.fullmatch(entry.name):
            sys.exit(f"{entry.name!r}: use only letters, digits and . _ + - in package names")
        names.append(entry.name)
    if not names:
        sys.exit(f"no packages in {folder}")
    return names


def build_kind(names, read):
    """Whether the app whose files are `names` is a "debug" or a "profile" build, with the
    file that shows it, or (None, None) for a release build; (None, "") when it holds no
    Flutter engine to tell. Names are separated by "/" or, as zips made on Windows may have
    them, by "\\". `read(name)` gives a file's bytes, or None."""
    def base(name):
        return name.replace("\\", "/").rsplit("/", 1)[-1]
    marker = next((n for n in names if base(n) == DEBUG_MARKER), None)
    if marker is not None:
        return "debug", marker
    engines = [n for n in names if base(n) in ENGINES]
    if not engines:
        return None, ""
    for engine in engines:
        data = read(engine)
        if data is not None and PROFILE_MARKER in data:
            return "profile", engine
    return None, None


def refuse_debug_builds(folder, names):
    """Stops on any archive that holds a debug or profile build, and says which files were not
    opened."""
    unopened = []
    for name in names:
        inside = archive_names(folder / name)
        if inside is None:
            unopened.append(name)
            continue
        kind, shown_by = build_kind(inside, lambda n, name=name: archive_file(folder / name, n))
        if kind is not None:
            sys.exit(f"{name} is a {kind} build (see {shown_by}): build it with --release")
        if shown_by == "":
            unopened.append(name)
    for name in unopened:
        print(f"  not an app archive, not checked: {name} (make sure it is a release build)")


def write_sums(folder, names):
    lines = [f"{sha256(folder / name)}  {name}" for name in names]
    path = folder / SUMS
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path, lines


def signer_keys(signers, principal):
    """The lines of an allowed signers file that name a key for `principal`."""
    if not signers.is_file():
        return []
    return [
        line
        for line in signers.read_text(encoding="utf-8").splitlines()
        if line.strip()
        and not line.lstrip().startswith("#")
        and principal in line.split(None, 1)[0].split(",")
    ]


def check_key_file(key):
    if not key.is_file():
        sys.exit(f"no key at {key}")
    if REPOSITORY in key.resolve().parents:
        sys.exit(f"{key} is inside the repository: keep the release key outside it")
    if os.name == "posix" and stat.S_IMODE(key.stat().st_mode) & 0o077:
        sys.exit(f"{key} can be read by others: chmod 600 {key}")


def sign(path, key):
    signature = path.with_name(path.name + ".sig")
    signature.unlink(missing_ok=True)
    # ssh-keygen asks for the passphrase on the terminal.
    result = subprocess.run(
        ["ssh-keygen", "-q", "-Y", "sign", "-f", str(key), "-n", NAMESPACE, str(path)]
    )
    if result.returncode != 0:
        discard(path, signature)
        sys.exit(f"ssh-keygen could not sign {path.name} (wrong passphrase or not a key?)")
    return signature


def signature_holds(path, signers, principal=PRINCIPAL):
    signature = path.with_name(path.name + ".sig")
    if not signature.is_file():
        return False
    with open(path, "rb") as data:
        result = subprocess.run(
            [
                "ssh-keygen", "-Y", "verify",
                "-f", str(signers),
                "-I", principal,
                "-n", NAMESPACE,
                "-s", str(signature),
            ],
            stdin=data,
            capture_output=True,
            text=True,
        )
    return result.returncode == 0


def discard(*paths):
    for path in paths:
        path.unlink(missing_ok=True)


def sign_command(args):
    folder = args.folder
    # A signature is worth the key people check it with: that key is committed first
    # (docs/releasing.md), and the new signature is checked against it.
    if not signer_keys(args.signers, PRINCIPAL):
        sys.exit(f"{args.signers} holds no {PRINCIPAL} key: add the release key's public line first")
    check_key_file(args.key)
    names = packages(folder)
    refuse_debug_builds(folder, names)
    sums, lines = write_sums(folder, names)
    signature = sign(sums, args.key)

    if not signature_holds(sums, args.signers):
        discard(sums, signature)
        sys.exit(f"the signature does not match the key in {args.signers}: wrong key?")

    print(f"{SUMS} and {SIGNATURE} in {folder} (checked against {args.signers})")
    for line in lines:
        print(f"  {line}")


def verify_command(args):
    folder = args.folder
    sums = folder / SUMS
    signers = args.signers
    principal = PRERELEASE_PRINCIPAL if args.prerelease else PRINCIPAL
    if not signer_keys(signers, principal):
        sys.exit(f"{signers} holds no {principal} key to check with")
    if not sums.is_file():
        sys.exit(f"no {SUMS} in {folder}")
    if not signature_holds(sums, signers, principal):
        sys.exit(f"{SIGNATURE} is missing or not a signature of {SUMS} by a {principal} key")
    print(f"{SUMS}: good signature by {principal}")
    bad = 0
    for line in sums.read_text(encoding="utf-8").splitlines():
        digest, _, name = line.partition("  ")
        path = folder / name
        if not PACKAGE_NAME.fullmatch(name):
            print(f"  {name!r}: not a package name")
            bad += 1
        elif not path.is_file():
            print(f"  {name}: missing")
            bad += 1
        elif sha256(path) != digest:
            print(f"  {name}: DIFFERENT from the signed checksum")
            bad += 1
        else:
            print(f"  {name}: OK")
    # Anything else beside them was not signed, and someone may open it believing it was.
    listed = {line.partition("  ")[2] for line in sums.read_text(encoding="utf-8").splitlines()}
    for entry in sorted(folder.iterdir()):
        if entry.name not in listed | {SUMS, SIGNATURE}:
            print(f"  {entry.name}: NOT in the signed list")
            bad += 1
    if bad:
        sys.exit(f"{bad} file(s) failed")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    signing = commands.add_parser("sign", help="write and sign SHA256SUMS")
    signing.add_argument("--key", required=True, type=pathlib.Path, help="private release key")
    checking = commands.add_parser("verify", help="check SHA256SUMS.sig and every checksum")
    checking.add_argument(
        "--prerelease",
        action="store_true",
        help=f"take a signature by a key of the beta ({PRERELEASE_PRINCIPAL}), and no other",
    )
    for command in (signing, checking):
        command.add_argument("folder", type=pathlib.Path)
        command.add_argument("--signers", type=pathlib.Path, default=SIGNERS)
    args = parser.parse_args()
    if not args.folder.is_dir():
        sys.exit(f"no folder {args.folder}")
    if args.command == "sign":
        sign_command(args)
    else:
        verify_command(args)


if __name__ == "__main__":
    main()
