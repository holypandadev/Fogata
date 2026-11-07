#!/usr/bin/env python3
"""Packs a WebRTC build into the archive the plugin's CMake unpacks, laid out like
webrtc-sdk's releases (<os>-<arch>-release/ with lib/, include/ and LICENSE), plus
NOTICE.md: the licence of every library inside, as WebRTC's own licence script lists them
for the target. The archive's files have fixed dates and order, so the same build gives
the same SHA-256.

    package.py --os linux|win --arch x64 --src <webrtc src> --out <build dir> --into <dir>
"""

import argparse
import hashlib
import html
import os
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent


def pins():
    values = {}
    for line in (HERE / "pins.sh").read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            key, value = line.split("=", 1)
            values[key] = value
    return values


def licences(src: Path, out: Path) -> str:
    """WebRTC's LICENSE.md for the libwebrtc target: its own licence, then each
    third-party library the target is built from, with its licence text."""
    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run(
            [
                sys.executable,
                "-B",
                str(src / "tools_webrtc/libs/generate_licenses.py"),
                "--target",
                "//libwebrtc:libwebrtc",
                tmp,
                str(out),
            ],
            check=True,
            cwd=src,
        )
        # The script escapes the texts for HTML; the notice is read as plain text.
        return html.unescape((Path(tmp) / "LICENSE.md").read_text())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--os", required=True, choices=["linux", "win"])
    parser.add_argument("--arch", required=True)
    parser.add_argument("--src", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--into", required=True, type=Path)
    args = parser.parse_args()

    version = pins()["LIBWEBRTC_VERSION"]
    top = f"{args.os}-{args.arch}-release"
    if args.os == "linux":
        libs = ["libwebrtc.so"]
    else:
        libs = ["libwebrtc.dll", "libwebrtc.dll.lib"]

    files = {}
    for name in libs:
        files[f"{top}/lib/{name}"] = (args.out / name).read_bytes()
    include = args.src / "libwebrtc/include"
    for path in sorted(include.rglob("*.h")):
        files[f"{top}/include/{path.relative_to(include).as_posix()}"] = path.read_bytes()
    files[f"{top}/LICENSE"] = (args.src / "libwebrtc/LICENSE").read_bytes()
    notice = (
        f"# {version}\n\n"
        "WebRTC (https://webrtc.googlesource.com/src) with webrtc-sdk's C++ wrapper\n"
        "(https://github.com/webrtc-sdk/libwebrtc), built by Fogata's tools/webrtc. The\n"
        "licences of WebRTC and of every library built into it follow.\n\n"
    )
    files[f"{top}/NOTICE.md"] = (notice + licences(args.src, args.out)).encode()

    # Named as the store and a release would name it: <version>/libwebrtc-<os>-<arch>-release.zip
    (args.into / version).mkdir(parents=True, exist_ok=True)
    archive = args.into / version / f"libwebrtc-{args.os}-{args.arch}-release.zip"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for name in sorted(files):
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            zf.writestr(info, files[name])
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    (archive.parent / (archive.name + ".sha256")).write_text(f"{digest}  {archive.name}\n")
    print(f"{archive}\nsha256_{args.os}-{args.arch} = {digest}")


if __name__ == "__main__":
    os.umask(0o022)
    main()
