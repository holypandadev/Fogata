#!/usr/bin/env python3
"""Writes THIRD-PARTY-NOTICES at the root of the repository: the third-party code that the
release packages carry, each part with its licence and, where the licence asks for it, its
licence text.

    python3 tools/third_party_notices.py          # writes THIRD-PARTY-NOTICES
    python3 tools/third_party_notices.py --check  # fails if THIRD-PARTY-NOTICES is out of date

It reads what is on this computer and downloads nothing: the Rust crates from `cargo metadata
--offline` (the crates the Fogata library, `fogata-ffi`, is built from on Linux, macOS and
Windows, without build or development dependencies) and their sources in Cargo's registry; the
Dart and Flutter packages from `app/pubspec.lock` and the pub cache (`PUB_CACHE`, or
`~/.pub-cache`), following the dependencies of `app/pubspec.yaml` (development dependencies
left out); the fonts and native libraries from the list below. Run `cargo fetch` and
`flutter pub get` first if the sources are not there yet.
"""

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "THIRD-PARTY-NOTICES"

# The platforms the release packages are built for.
TARGETS = [
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
]

# Licences in the order one is chosen when a crate offers several.
PREFERRED = [
    "MIT",
    "MIT-0",
    "BSD-3-Clause",
    "BSD-2-Clause",
    "ISC",
    "Zlib",
    "0BSD",
    "Apache-2.0",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "CC0-1.0",
    "Unlicense",
    "BSL-1.0",
]

# Words that tell a licence file's licence when its name does not.
SIGNS = {
    "MIT": ["Permission is hereby granted, free of charge"],
    "MIT-0": ["Permission is hereby granted, free of charge"],
    "Apache-2.0": ["Apache License", "TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION"],
    "BSD-3-Clause": ["Redistribution and use in source and binary forms", "Neither the name"],
    "BSD-2-Clause": ["Redistribution and use in source and binary forms"],
    "ISC": ["Permission to use, copy, modify, and/or distribute this software"],
    "Zlib": ["This software is provided 'as-is'"],
    "Unicode-3.0": ["UNICODE"],
    "Unicode-DFS-2016": ["UNICODE"],
    "CC0-1.0": ["CC0"],
    "Unlicense": ["This is free and unencumbered software"],
    "BSL-1.0": ["Boost Software License"],
    "0BSD": ["Permission to use, copy, modify, and/or distribute this software"],
}

# The WebRTC library's own licence (libwebrtc on Linux and Windows, WebRTC-SDK on macOS).
WEBRTC_LICENSE = """Copyright (c) 2011, The WebRTC project authors. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

  * Redistributions of source code must retain the above copyright
    notice, this list of conditions and the following disclaimer.

  * Redistributions in binary form must reproduce the above copyright
    notice, this list of conditions and the following disclaimer in
    the documentation and/or other materials provided with the
    distribution.

  * Neither the name of Google nor the names of its contributors may
    be used to endorse or promote products derived from this software
    without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
"""

# Fogata's own packages in app/.
OWN_PACKAGES = {"fogata", "fogata_video", "rust_lib_fogata"}

RULE = "=" * 79
LINE = "-" * 79


def read(path):
    return Path(path).read_text(encoding="utf-8", errors="replace").replace("\r\n", "\n").strip()


# The licence and notice files of libwebrtc (Linux and Windows) come in its archive, which is
# unpacked only where the app has been built for one of them (not on a Mac). A copy is kept
# here so that THIRD-PARTY-NOTICES can be made, and checked, anywhere; where the archive is
# unpacked its files must be the same as the copy, or libwebrtc changed and the copy with it.
LIBWEBRTC_COPY = ROOT / "tools/notices/libwebrtc"
LIBWEBRTC_UNPACKED = ROOT / "app/third_party/flutter_webrtc/third_party/libwebrtc"


def libwebrtc_file(name):
    """The path of libwebrtc's file `name`: the copy kept in the repository, after checking it
    against the unpacked archive when this computer has one."""
    kept = LIBWEBRTC_COPY / name
    if not kept.exists():
        sys.exit(f"{kept} is missing: copy it from a libwebrtc archive (linux-x64-release/{name})")
    unpacked = LIBWEBRTC_UNPACKED / name
    if unpacked.exists() and unpacked.read_bytes() != kept.read_bytes():
        sys.exit(
            f"{unpacked} differs from {kept.relative_to(ROOT)}: libwebrtc changed, so copy the"
            " new file over the one kept in the repository and run this again"
        )
    return kept


def native_parts():
    """Fonts and native libraries the packages carry: (name, where, licence, text)."""
    fonts = ROOT / "app/assets/fonts"
    webrtc_sdk = libwebrtc_file("LICENSE")
    webrtc_notice = libwebrtc_file("NOTICE.md")
    flutter = os.environ.get("FLUTTER_ROOT") or shutil.which("flutter")
    if not flutter:
        sys.exit("flutter is not on the PATH (or set FLUTTER_ROOT)")
    flutter = Path(flutter)
    if flutter.is_file():
        flutter = flutter.resolve().parent.parent
    flutter_license = flutter / "LICENSE"
    parts = [
        ("Inter (font)", "https://github.com/rsms/inter", "OFL-1.1", read(fonts / "Inter-OFL.txt")),
        ("Geist Mono (font)", "https://github.com/vercel/geist-font", "OFL-1.1", read(fonts / "OFL.txt")),
        (
            "Phosphor Icons (font)",
            "https://github.com/phosphor-icons/core",
            "MIT",
            read(fonts / "Phosphor-LICENSE.txt"),
        ),
        (
            "Unicode CLDR emoji annotations (the emoji names and keywords in app/assets/emoji)",
            "https://github.com/unicode-org/cldr-json",
            "Unicode-3.0",
            read(ROOT / "app/assets/emoji/CLDR-LICENSE.txt"),
        ),
        (
            "WebRTC (in libwebrtc on Linux and Windows, WebRTC-SDK on macOS)",
            "https://webrtc.googlesource.com/src",
            "BSD-3-Clause",
            WEBRTC_LICENSE.strip()
            + "\n\nThe WebRTC library also holds third-party code of its own, each part under its"
            "\nown permissive licence. On Linux and Windows, those built into libwebrtc are"
            "\nlisted below with their licences; on Windows it also holds LLVM's compiler-rt,"
            "\nunder the LLVM licence of libc++ below. On macOS, Fogata's build of the same WebRTC"
            "\n(WebRTC-SDK, tools/webrtc/MACOS.md) holds the same libraries; its"
            "\nWebRTC.xcframework/NOTICE.md lists them all.",
        ),
    ]
    # WebRTC's own list of the libraries built into libwebrtc, after its first heading (the
    # name of the build), without the Markdown fences around each licence.
    notice = read(webrtc_notice).split("\n", 1)[1]
    notice = "\n".join(l for l in notice.split("\n") if l.strip() != "```").strip()
    parts.append(
        (
            "The libraries built into libwebrtc (Linux and Windows)",
            "tools/webrtc",
            "BSD-3-Clause and others, each named below",
            notice,
        )
    )
    parts.append(
        (
            "libwebrtc (the C++ wrapper of WebRTC by webrtc-sdk, Linux and Windows)",
            "https://github.com/webrtc-sdk/libwebrtc",
            "MIT",
            read(webrtc_sdk),
        )
    )
    opus = crate_dir("opusic-sys") / "opus/COPYING"
    parts.append(
        (
            "libopus (voice messages; built from the copy opusic-sys carries, linked statically)",
            "https://opus-codec.org",
            "BSD-3-Clause",
            read(opus),
        )
    )
    parts.append(
        (
            "Flutter (engine and framework)",
            "https://github.com/flutter/flutter",
            "BSD-3-Clause",
            read(flutter_license)
            + "\n\nThe engine's own third-party code and the packages' notices are also in each"
            "\npackage, in flutter_assets/NOTICES.Z, which Flutter writes at build time.",
        )
    )
    return parts


# ----- Rust -----


def split_top(expr, word):
    """Splits an SPDX expression at `word` (AND, OR) outside parentheses."""
    parts, depth, start = [], 0, 0
    tokens = re.finditer(r"\(|\)|\bAND\b|\bOR\b", expr)
    for t in tokens:
        if t.group() == "(":
            depth += 1
        elif t.group() == ")":
            depth -= 1
        elif t.group() == word and depth == 0:
            parts.append(expr[start : t.start()])
            start = t.end()
    parts.append(expr[start:])
    return [p.strip() for p in parts]


def strip_parens(expr):
    expr = expr.strip()
    while expr.startswith("(") and expr.endswith(")"):
        inner = expr[1:-1]
        depth = 0
        for ch in inner:
            depth += ch == "("
            depth -= ch == ")"
            if depth < 0:
                return expr
        expr = inner.strip()
    return expr


def options(expr):
    """The ways to follow an SPDX expression, the preferred first: each a list of licences
    (one of each `OR`, all of each `AND`)."""
    expr = strip_parens(expr.replace("/", " OR "))
    ands = split_top(expr, "AND")
    if len(ands) > 1:
        ways = [[]]
        for part in ands:
            ways = [w + o for w in ways for o in options(part)]
        return ways
    ors = [strip_parens(p) for p in split_top(expr, "OR")]
    if len(ors) > 1:
        ways = [o for p in ors for o in options(p)]
        ways.sort(key=lambda o: max(PREFERRED.index(x) if x in PREFERRED else 99 for x in o))
        return ways
    return [[re.sub(r"\s+WITH\s+.*", "", expr)]]


# The MIT licence's text, for a crate under MIT that does not carry it.
MIT_TEXT = """Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""


def licence_files(folder):
    names = []
    for p in sorted(Path(folder).iterdir()):
        n = p.name.lower()
        if p.is_file() and re.match(r"(un)?licen[cs]e|copying|copyright|notice|authors", n):
            names.append(p)
    return names


def text_for(licence, files):
    """The text of `licence` among a package's licence files, or None."""
    if licence not in SIGNS:
        return None
    named = [f for f in files if licence.lower().split("-")[0] in f.name.lower()]
    for f in named + files:
        if f.name.lower().startswith(("notice", "authors")):
            continue
        text = read(f)
        if all(sign in text for sign in SIGNS.get(licence, [])):
            return text
    return None


def crate_dir(name):
    """Where the source of crate `name`, as the workspace uses it, is on this computer."""
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--offline", "--quiet", "--format-version", "1",
             "--manifest-path", str(ROOT / "Cargo.toml")],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    for p in meta["packages"]:
        if p["name"] == name:
            return Path(p["manifest_path"]).parent
    sys.exit(f"crate {name} is not in the workspace's dependencies")


def rust_crates():
    """The crates built into the Fogata library for the release platforms, as `cargo tree`
    has them (only the features that are on), with what `cargo metadata` says of each."""
    cargo = ["cargo", "--offline", "--quiet"]
    manifest = ["--manifest-path", str(ROOT / "Cargo.toml")]
    meta = json.loads(
        subprocess.run(
            cargo[:1] + ["metadata"] + cargo[1:] + ["--format-version", "1"] + manifest,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    packages = {(p["name"], p["version"]): p for p in meta["packages"]}
    members = {(p["name"], p["version"]) for p in meta["packages"] if p["id"] in meta["workspace_members"]}
    built = set()
    for target in TARGETS:
        tree = subprocess.run(
            cargo[:1] + ["tree"] + cargo[1:] + manifest
            + ["-p", "fogata-ffi", "-e", "normal", "--target", target, "--prefix", "none", "-f", "{p}"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        for line in tree.splitlines():
            m = re.match(r"(\S+) v(\S+)", line)
            if m:
                built.add((m.group(1), m.group(2)))
    return [packages[k] for k in sorted(built - members)]


def rust_parts():
    parts = []
    for p in rust_crates():
        folder = Path(p["manifest_path"]).parent
        files = licence_files(folder)
        expr = p.get("license") or ""
        if not expr and p.get("license_file"):
            expr = "see its licence file"
            texts = [read(folder / p["license_file"])]
        else:
            ways = options(expr)
            found = [[text_for(lic, files) for lic in way] for way in ways]
            whole = [i for i, texts in enumerate(found) if None not in texts]
            if whole:
                way, texts = ways[whole[0]], found[whole[0]]
            elif "MIT" in [w[0] for w in ways if len(w) == 1]:
                # The crate says MIT but carries no text: the licence with its copyright
                # lines, or its authors.
                way = ["MIT"]
                lines = [
                    line.strip()
                    for f in files
                    for line in read(f).splitlines()
                    if line.strip().lower().startswith("copyright")
                ]
                authors = ", ".join(re.sub(r"\s*<[^>]*>", "", a) for a in p["authors"])
                owner = "\n".join(dict.fromkeys(lines)) or (
                    f"Copyright (c) {authors or 'the ' + p['name'] + ' authors'}"
                )
                texts = [f"{owner}\n\n{MIT_TEXT}"]
            else:
                sys.exit(f"{p['name']} {p['version']}: no text for {expr} in {folder}")
            if "Apache-2.0" in way:
                texts += [read(f) for f in files if f.name.lower().startswith("notice")]
            way = list(dict.fromkeys(way))
            expr = expr if len(ways) == 1 else f"{expr} (here: {' AND '.join(way)})"
        texts = list(dict.fromkeys(texts))
        where = p.get("repository") or "https://crates.io/crates/" + p["name"]
        parts.append((f"{p['name']} {p['version']}", where, expr, "\n\n".join(texts)))
    return parts


# ----- Dart -----


def pub_cache():
    return Path(os.environ.get("PUB_CACHE", Path.home() / ".pub-cache"))


def dart_parts():
    app = ROOT / "app"
    lock = read(app / "pubspec.lock")
    locked = {}
    for m in re.finditer(
        r"^  (\S+):\n((?:    .*\n|      .*\n)+)", lock + "\n", re.MULTILINE
    ):
        body = m.group(2)
        source = re.search(r"source: (\S+)", body).group(1)
        version = re.search(r'version: "([^"]+)"', body).group(1)
        path = re.search(r'path: "?([^"\n]+)"?', body)
        locked[m.group(1)] = (source, version, path.group(1) if path else None)

    def folder(name):
        source, version, path = locked[name]
        if source == "hosted":
            return pub_cache() / "hosted/pub.dev" / f"{name}-{version}"
        if source == "path":
            return (app / path).resolve()
        return None

    def deps(pubspec):
        text = read(pubspec)
        block = re.search(r"^dependencies:\n((?:[ #].*\n|\n)*)", text + "\n", re.MULTILINE)
        if not block:
            return []
        return re.findall(r"^  ([a-z0-9_]+):", block.group(1), re.MULTILINE)

    todo = deps(app / "pubspec.yaml")
    reached = set()
    while todo:
        name = todo.pop()
        if name in reached or name not in locked:
            continue
        reached.add(name)
        where = folder(name)
        if where is not None:
            todo += deps(where / "pubspec.yaml")
    parts = []
    for name in sorted(reached):
        source, version, _ = locked[name]
        if source == "sdk" or name in OWN_PACKAGES:
            # The Flutter SDK's own packages (Flutter's licence, above), and Fogata's.
            continue
        where = folder(name)
        files = [f for f in licence_files(where) if not f.name.lower().startswith("authors")]
        if not files:
            sys.exit(f"{name} {version}: no licence file in {where}")
        url = f"https://pub.dev/packages/{name}" if source == "hosted" else f"app/{locked[name][2]}"
        parts.append((f"{name} {version}", url, "", "\n\n".join(read(f) for f in files)))
    return parts


# ----- Output -----


def section(title, parts):
    out = [RULE, title, RULE, ""]
    first = {}
    for name, where, licence, text in parts:
        out.append(name)
        out.append(where)
        if licence:
            out.append(f"Licence: {licence}")
        if text in first:
            out.append(f"Licence text: the same as {first[text]}'s, above.")
            out.append("")
        else:
            first[text] = name
            out += ["", text, ""]
        out.append(LINE)
        out.append("")
    return out


def notices():
    out = [
        "THIRD-PARTY NOTICES",
        "",
        "Fogata is made with the third-party software listed here, which its packages carry.",
        "Each entry names the software, where it comes from and its licence, followed by the",
        "licence's text. When several packages share the very same text, it is written once.",
        "",
        "Made by tools/third_party_notices.py; do not edit by hand.",
        "",
    ]
    out += section("Fonts and native libraries", native_parts())
    out += section("Rust crates (in the Fogata library)", rust_parts())
    out += section("Dart and Flutter packages", dart_parts())
    return "\n".join(out).rstrip() + "\n"


def main():
    text = notices()
    if "--check" in sys.argv[1:]:
        if not OUT.exists() or OUT.read_text(encoding="utf-8") != text:
            sys.exit("THIRD-PARTY-NOTICES is out of date: run python3 tools/third_party_notices.py")
        return
    OUT.write_text(text, encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)} ({len(text) // 1024} KiB)")


if __name__ == "__main__":
    main()
