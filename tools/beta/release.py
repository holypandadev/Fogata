#!/usr/bin/env python3
"""Makes a pre-release from the builds of each system.

    python3 -I tools/beta/release.py --build 2 --version 0.1.0-beta.2 --min-build 1 \\
        --notes notes.txt --linux-bundle app/build/linux/x64/release/bundle \\
        --windows-zip Fogata-windows-x64.zip --macos-zip Fogata-macos-arm64.zip --out <folder>

Inputs (each optional, at least one):
  --linux-bundle  the Flutter release bundle built on Linux, or by the release workflow
                  (fetched and checked by tools/beta/ci_builds.py); the Flatpak is made from
                  it (tools/beta/flatpak.sh; --extra-libs is passed on)
  --build-linux   instead of --linux-bundle: build that bundle here, from a clean worktree of
                  the commit checked out (a target folder of its own, no changes of this
                  checkout and no library of an earlier build can get in), and use it
  --windows-zip   a zip of build\\windows\\x64\\runner\\Release (fogata.exe and what is
                  beside it), built on Windows or by the release workflow
  --macos-zip     Fogata.app (built by hand or by the release workflow, with
                  tools/beta/macos-updater.sh), signed on the Mac (tools/sign-macos.sh)
                  and zipped with `ditto -c -k --keepParent`; give --macos-arch arm64 or x64

Writes into --out (made new):
  release/   release.json, release.sig and one package per system: publish it with
             `fogata --data-dir <folder> release publish <out>/release`
  drive/     the first install for each system, README.txt, and SHA256SUMS with every
             file's SHA-256, signed by the same key (SHA256SUMS.sig), to upload
  work/      what was unpacked on the way (can go)

THIS_BUILD and THIS_VERSION in crates/fogata-update/src/lib.rs must be --build and
--version, RELEASE_KEYS must name a release key and a rescue key, or no copy would take the
release (--allow-no-keys for tests), and the checkout must have no changes to its files.
Every build must be a release build (not debug or profile) made from the commit checked out
here with no changes, holding that build and trusting exactly the keys RELEASE_KEYS names
(`fogata release pack --expect-build --expect-key --expect-rescue-key --expect-commit`): a
build made before the keys changed would never take a release signed by them. The Windows
and Linux programs and libraries must carry the hardening their builds give them, and the
Linux ones look for libraries only beside them ($ORIGIN). The key is
read from ~/.config/fogata-release/release-key unless --key says otherwise (the rescue key,
for a rescue release); `fogata release sign` asks for its passphrase, and its secret never
leaves that file. The Flatpak is signed with the key in ~/.config/fogata-flatpak-key/gnupg
(tools/beta/flatpak-key.sh) unless --gpg-homedir says otherwise, never one kept in the
folder of a release or rescue key (refused); GnuPG asks for that passphrase. Every manifest signed is noted in published.log beside the key.
"""

import argparse
import hashlib
import importlib.util
import os
import pathlib
import plistlib
import re
import shutil
import stat
import subprocess
import sys
import time
import zipfile

REPO = pathlib.Path(__file__).resolve().parents[2]
KEY_DIR = pathlib.Path.home() / ".config/fogata-release"
# The Flatpak signing key's GnuPG home, apart from the release keys.
FLATPAK_KEY_DIR = pathlib.Path.home() / ".config/fogata-flatpak-key/gnupg"
# The list of the Windows application's files, beside install.ps1 in the first install: the
# script installs those files only, each with its SHA-256.
WINDOWS_LIST = "Fogata.sha256"


def load_public_release():
    """tools/release.py, for its check of debug and profile builds."""
    sys.dont_write_bytecode = True  # no __pycache__ in the repository
    spec = importlib.util.spec_from_file_location("public_release", REPO / "tools/release.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


README = """Fogata {version} (pre-release)

FIRST, BEFORE YOU UNZIP OR OPEN ANYTHING: check what you downloaded
  Fogata has no paid code signing certificate, so Windows and macOS cannot tell who made
  these files, and they warn you before Fogata first opens (see below). What shows that a
  file is the one Holy Panda made is its SHA-256 checksum, listed in SHA256SUMS. That list
  is signed by Fogata's pre-release key (SHA256SUMS.sig), whose public half is in Fogata's
  source code, in the file tools/allowed_signers. Take that file from the source code (not
  from where you downloaded these files, which whoever changed them could have changed
  too), save it as allowed_signers next to SHA256SUMS, and check the list:
      ssh-keygen -Y verify -f allowed_signers -I fogata-prerelease -n file -s SHA256SUMS.sig < SHA256SUMS
  It must say: Good "file" signature for fogata-prerelease. Then work out the checksum of the
  file you downloaded YOURSELF, with the command below, and compare it with its line in
  SHA256SUMS. Do not rely on the installer to do it: its script is inside the zip, so
  whoever changed the zip could have changed the script and the checksum it shows.
    Windows, in PowerShell:  Get-FileHash .\\Fogata-{version}-windows-x64.zip
    macOS, in Terminal:      shasum -a 256 Fogata-{version}-macos-{mac_arch}.zip
    Linux, in a terminal:    sha256sum Fogata-{version}-linux.flatpak
  If the signature is not good or the checksum differs, do not open the file: write to
  contact@holypanda.dev.

Windows
  1. Unzip Fogata-{version}-windows-x64.zip (right click, Extract All).
  2. Open the folder and double-click "Install Fogata".
  3. Windows says "Windows protected your PC": it says so for any program without a paid
     signature that it has not seen often. Only if the checksum matched, click "More info",
     then "Run anyway". The installer shows the zip's checksum again and installs only
     once you type yes to say that you worked it out yourself before unzipping and it
     matched. It checks every file of Fogata against the list
     in the download, installs them for you only (no administrator) and opens Fogata; it is
     in the Start menu.

macOS (Apple silicon{mac_note})
  1. Open Fogata-{version}-macos-{mac_arch}.zip and drag Fogata to Applications.
  2. Open Fogata. macOS says it cannot check it for malicious software: Fogata is signed
     but not notarized by Apple, which needs a paid membership. Only if the checksum
     matched, click "Done", then open System Settings, Privacy & Security, scroll down and
     click "Open Anyway" next to Fogata, and confirm. This is needed once.

Linux
  Fogata comes as a Flatpak. In a terminal, in this folder:
      flatpak install --user Fogata-{version}-linux.flatpak
  (it needs the org.freedesktop.Platform 26.08 runtime from Flathub, which flatpak offers
  to install). Then open Fogata from your applications menu.

Updates
  Fogata keeps itself up to date. It looks for new versions on its releases page and among
  your friends, takes only versions signed by Holy Panda's release key, and checks every
  byte of them. A new version installs when Fogata starts; one that arrives while it runs
  waits for you to restart it. Your profile, friends, groups and chats stay.

What to expect from this early version
  - Things will go wrong now and then. When they do, Settings, "Report a problem" shows
    you the whole report before it goes (folders, names and addresses taken out) and sends
    it to contact@holypanda.dev.
  - Fogata has no servers: computers reach each other directly. Two people whose networks
    both block that (some offices, some mobile networks) cannot reach each other unless
    someone of the group can reach both. Fogata then says what keeps you apart.
  - One account works on one computer at a time. To take it to another computer, use
    "Move account" in Settings, with both computers on.
  - Keep a recovery copy (Settings, "Storage & downloads", "Recovery copy"), and its
    recovery key apart from it: if this computer's keychain or disk is lost, it brings
    your account back.
  - Messages and calls are encrypted end to end, but the encryption has not yet been
    reviewed by an outside auditor: do not rely on Fogata for anything that would put
    someone at risk.
"""


def fail(text):
    sys.exit(f"release.py: {text}")


def run(*args, **kwargs):
    print("$", " ".join(str(a) for a in args))
    subprocess.run([str(a) for a in args], check=True, **kwargs)


def this_build():
    source = (REPO / "crates/fogata-update/src/lib.rs").read_text()
    build = re.search(r"pub const THIS_BUILD: u64 = (\d+);", source)
    version = re.search(r'pub const THIS_VERSION: &str = "([^"]+)";', source)
    if not build or not version:
        fail("cannot read THIS_BUILD and THIS_VERSION")
    return int(build.group(1)), version.group(1)


def release_keys(source=None):
    """The keys RELEASE_KEYS in the source names, as (role, hex) with role "release" or
    "rescue", in order. Stops on anything in it that is not such a key."""
    if source is None:
        source = (REPO / "crates/fogata-update/src/lib.rs").read_text()
    keys = re.search(r"pub const RELEASE_KEYS: &\[ReleaseKey\] = &\[(.*?)\];", source, re.S)
    if not keys:
        fail("cannot read RELEASE_KEYS")
    # Comments go first: a key commented out is no key.
    named = re.sub(r"//[^\n]*|/\*.*?\*/", "", keys.group(1), flags=re.S)
    found = []
    pattern = re.compile(r"\s*ReleaseKey::(release|rescue)\(\[([^\]]*)\]\)\s*(,|$)")
    at = 0
    while named[at:].strip():
        key = pattern.match(named, at)
        if not key:
            fail(f"cannot read RELEASE_KEYS from: {named[at:].strip()[:60]}")
        parts = [p.strip() for p in key.group(2).split(",") if p.strip()]
        if len(parts) != 32 or not all(re.fullmatch(r"0x[0-9a-fA-F]{1,2}", p) for p in parts):
            fail(f"a key in RELEASE_KEYS is not 32 bytes: {key.group(0).strip()[:60]}")
        found.append((key.group(1), bytes(int(p, 16) for p in parts).hex()))
        at = key.end()
    return found


def release_keys_named(source=None):
    """Whether RELEASE_KEYS in the source names a release key and a rescue key."""
    roles = {role for role, _ in release_keys(source)}
    return roles == {"release", "rescue"}


def expect_keys(keys):
    """The arguments of `fogata release pack` that make it check a build trusts `keys`."""
    args = []
    for role, key in keys:
        args += ["--expect-key" if role == "release" else "--expect-rescue-key", key]
    return args


# What a release is built from: where a file git does not keep yet still changes the build.
# The same list as fogata-update's build.rs.
SOURCES = ["crates", "vendor", "app", "Cargo.toml", "Cargo.lock", ".cargo", "rust-toolchain.toml"]


# What changes what the compilers make: a build made with them set is not the build the
# release scripts check for.
BUILD_ENV_CLEARED = ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "CFLAGS",
                     "CXXFLAGS", "LDFLAGS", "CMAKE", "CARGO_BUILD_TARGET", "CARGO_TARGET_DIR"]


def build_linux(commit, work, repo=REPO):
    """Builds the Linux release bundle from a clean worktree of `commit` in `work`, with a
    target folder of its own and without the variables that change what the compilers make;
    returns the bundle's folder (a copy that outlives the worktree). The libwebrtc download is
    taken from `repo`, which has it; the build is offline."""
    tree = pathlib.Path(work) / "linux-tree"
    run("git", "-C", repo, "worktree", "add", "--detach", tree, commit)
    try:
        webrtc = pathlib.PurePosixPath("app/third_party/flutter_webrtc/third_party/downloads")
        downloads = pathlib.Path(repo) / webrtc
        if downloads.is_dir():
            (tree / webrtc).mkdir(parents=True, exist_ok=True)
            for zipped in downloads.glob("*.zip"):
                shutil.copy(zipped, tree / webrtc / zipped.name)
        env = {name: value for name, value in os.environ.items() if name not in BUILD_ENV_CLEARED}
        env["CARGO_TARGET_DIR"] = str(pathlib.Path(work) / "linux-target")
        run("flutter", "pub", "get", "--offline", cwd=tree / "app", env=env)
        run("flutter", "build", "linux", "--release", cwd=tree / "app", env=env)
        built = tree / "app/build/linux/x64/release/bundle"
        if not built.is_dir():
            fail(f"the Linux build left no bundle at {built}")
        bundle = pathlib.Path(work) / "linux-bundle"
        shutil.copytree(built, bundle, symlinks=True)
    finally:
        subprocess.run(["git", "-C", str(repo), "worktree", "remove", "--force", str(tree)],
                       capture_output=True)
        subprocess.run(["git", "-C", str(repo), "worktree", "prune"], capture_output=True)
        shutil.rmtree(pathlib.Path(work) / "linux-target", ignore_errors=True)
    return bundle


def checked_out(repo=REPO):
    """The commit checked out in `repo`; stops when a file git keeps there has changes, or a
    file it does not keep yet is among the sources, since the builds must be made from the
    commit itself."""
    def git(*args):
        result = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)
        if result.returncode != 0:
            fail(f"git {' '.join(args)}: {result.stderr.strip()}")
        return result.stdout.strip()
    changed = git("status", "--porcelain", "--untracked-files=no")
    if not changed:
        existing = [name for name in SOURCES if (pathlib.Path(repo) / name).exists()]
        changed = git("status", "--porcelain", "--untracked-files=normal", "--", *existing) \
            if existing else ""
    if changed:
        fail("the checkout has changes not committed; commit them and build every system "
             f"from that commit:\n{changed}")
    return git("rev-parse", "--verify", "HEAD^{commit}")


def check_cli_commit(fogata, commit):
    """Stops unless the command-line program `fogata` was built from `commit`, with nothing
    changed: it packs, signs and checks the release, so one left from another commit (an old
    target/release/fogata) must not. Its commit marker (fogata_update::COMMIT_MARKER, which a
    release build holds) says which commit it was built from."""
    data = pathlib.Path(fogata).read_bytes()
    found = sorted({m.decode() for m in re.findall(
        rb"fogata-commit-marker:([0-9a-f]{40}(?:\+dirty)?|unknown);", data)})
    if found != [commit]:
        fail(f"{fogata} was built from {', '.join(found) or 'no known commit'}, not from "
             f"{commit}: build it again here (cargo build --release -p fogata)")


def safe_member(name):
    """A zip member's path as plain parts, or None for what must not be unpacked."""
    parts = name.replace("\\", "/").split("/")
    parts = [p for p in parts if p not in ("", ".")]
    if not parts or name.startswith("/") or any(p == ".." for p in parts):
        return None
    if parts[0] == "__MACOSX" or parts[-1].startswith("._"):
        return None
    return parts


def inside(path, root):
    """Whether `path` is `root` or inside it, both resolved."""
    path, root = os.path.realpath(path), os.path.realpath(root)
    return path == root or path.startswith(root + os.sep)


def link_climbs_out(link_dir, target, root):
    """Whether a link in folder `link_dir` (resolved, inside `root`) to `target` would lead
    out of `root`: the rule of `fogata release pack`. Steps up (`..`) come only first, and no
    higher than the link's own folder; after them come plain names, each a file, a folder or
    another link that keeps to the same rule."""
    if not target or target.startswith("/") or "\\" in target or "\0" in target:
        return True
    relative = os.path.relpath(os.path.realpath(link_dir), os.path.realpath(root))
    depth = 0 if relative == "." else len(relative.split(os.sep))
    named = False
    for part in target.split("/"):
        if part == ".." and not named:
            depth -= 1
            if depth < 0:
                return True
        elif part in ("", ".", ".."):
            return True
        else:
            named = True
    return False


def unzip(archive, dest):
    """Unpacks `archive` into `dest`, made new, keeping links (as ditto stores them) and
    executable bits. Files and folders come first and links last, so nothing is written
    through a link; each file goes into a folder that is really inside `dest`, and each link
    must lead inside it."""
    dest.mkdir(parents=True)
    links = []
    with zipfile.ZipFile(archive) as z:
        for info in z.infolist():
            parts = safe_member(info.filename)
            if parts is None:
                continue
            path = dest.joinpath(*parts)
            mode = info.external_attr >> 16
            if stat.S_ISLNK(mode):
                links.append((info.filename, parts, z.read(info).decode()))
                continue
            # A folder ends with / or, as PowerShell 5's Compress-Archive writes it, with \;
            # or it has the MS-DOS folder attribute.
            if info.filename.endswith(("/", "\\")) or info.external_attr & 0x10:
                path.mkdir(parents=True, exist_ok=True)
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            if not inside(path.parent, dest) or os.path.lexists(path):
                fail(f"{info.filename} cannot be unpacked inside {dest}")
            path.write_bytes(z.read(info))
            os.chmod(path, 0o755 if mode & 0o111 else 0o644)
    for name, parts, target in links:
        path = dest.joinpath(*parts)
        path.parent.mkdir(parents=True, exist_ok=True)
        if not inside(path.parent, dest) or os.path.lexists(path):
            fail(f"{name} cannot be unpacked inside {dest}")
        if link_climbs_out(path.parent, target, dest):
            fail(f"{name} links to {target}, out of the app")
        os.symlink(target, path)


RELEASE_KEY_FILES = ("release-key", "release-key.pub", "rescue-key", "rescue-key.pub")


def check_flatpak_key_apart(gpg_homedir, key):
    """Stops when the Flatpak signing key's GnuPG home is inside the folder of the release key
    that signs, or of any release or rescue key: on Linux that key alone can make copies run
    code, so whoever takes one key must not have the other with it."""
    home = gpg_homedir.resolve()
    if key.resolve().parent in (home, *home.parents):
        fail(f"{gpg_homedir} is inside the folder of {key}: keep the Flatpak signing key in a "
             "folder of its own (docs/releasing-beta.md)")
    for folder in (home, *home.parents):
        if any((folder / name).exists() for name in RELEASE_KEY_FILES):
            fail(f"{gpg_homedir} is inside {folder}, which holds a release or rescue key: keep "
                 "the Flatpak signing key in a folder of its own (docs/releasing-beta.md)")


def check_macos_info(app, build):
    """Stops unless the Info.plist of the macOS `app` names Fogata and this build: the updater
    inside installed copies takes a new version only when the build its signature seals there
    is newer than theirs (app/macos/Updater/Checks.swift)."""
    try:
        info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    except (OSError, plistlib.InvalidFileException, ValueError) as error:
        fail(f"cannot read Fogata.app's Info.plist: {error}")
    if info.get("CFBundleIdentifier") != "dev.holypanda.fogata":
        fail(f"Fogata.app is {info.get('CFBundleIdentifier')!r}, not dev.holypanda.fogata")
    if info.get("CFBundleVersion") != str(build):
        fail(f"Fogata.app is build {info.get('CFBundleVersion')!r}, not {build}: set version in "
             "app/pubspec.yaml and build it again")


def refuse_debug_folder(folder):
    """Stops if the app in `folder` is a debug or profile build, or holds no Flutter engine."""
    public = load_public_release()
    names = sorted(p.relative_to(folder).as_posix() for p in folder.rglob("*")
                   if p.is_file() and not p.is_symlink())
    kind, shown_by = public.build_kind(names, lambda n: (folder / n).read_bytes())
    if kind is not None:
        fail(f"{folder} is a {kind} build (see {shown_by}): build it with --release")
    if shown_by == "":
        fail(f"{folder} holds no Flutter engine: is it the application?")


def write_windows_list(app, path):
    """Lists every file of the Windows application folder `app` with its SHA-256."""
    lines = []
    for file in sorted(app.rglob("*")):
        if file.is_symlink():
            fail(f"{file} is a link: the Windows application holds none")
        if file.is_file():
            digest = hashlib.sha256(file.read_bytes()).hexdigest()
            lines.append(f"{digest}  {file.relative_to(app).as_posix()}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


# What a Windows program or library of ours must have. Every one: high-entropy ASLR, ASLR
# and DEP (DllCharacteristics). CFG: Control Flow Guard (DllCharacteristics). EHCONT: EH
# continuation metadata (the load configuration's GuardFlags).
PE_HIGH_ENTROPY_VA, PE_DYNAMIC_BASE, PE_NX_COMPAT, PE_GUARD_CF = 0x20, 0x40, 0x100, 0x4000
PE_CET_COMPAT = 0x1
PE_GUARD_EH_CONTINUATION = 0x400000
CFG, EHCONT = "Control Flow Guard", "EH continuation metadata"
# Every .exe and .dll of the application is checked: those named here for what they can
# have, any other (the runner, the plugins, built with app/windows/CMakeLists.txt) for both.
# fogata_ffi.dll: no stable rustc emits EH continuation metadata. libwebrtc.dll
# (tools/webrtc): built with WebRTC's own toolchain, without C++ exceptions, so without it.
# flutter_windows.dll: Flutter's engine as its upstream release ships it, with neither.
MITIGATIONS = {"fogata_ffi.dll": {CFG}, "libwebrtc.dll": {CFG}, "flutter_windows.dll": set()}
# CET shadow stack compatibility (the extended characteristics, in the debug directory) is
# off on purpose, and the program must not have it: Windows turns shadow stacks on for a
# process when its program is compatible, which was never tried on a processor that has
# them (app/windows/CMakeLists.txt). What a library says does not count while the program
# is not compatible, so libwebrtc.dll, compatible from its own build, is left as it is.
NOT_CET = "fogata.exe"
REQUIRED = ("fogata.exe", "fogata_ffi.dll")


def pe_characteristics(data):
    """The DllCharacteristics of the PE file in `data`, its extended ones and its guard
    flags (0 when it has none), or None when it is not a PE file."""
    def u16(at):
        return int.from_bytes(data[at:at + 2], "little")

    def u32(at):
        return int.from_bytes(data[at:at + 4], "little")

    if len(data) < 0x40 or data[:2] != b"MZ":
        return None
    pe = u32(0x3C)
    if data[pe:pe + 4] != b"PE\0\0":
        return None
    sections, optional_size = u16(pe + 6), u16(pe + 20)
    optional = pe + 24
    magic = u16(optional)
    if magic not in (0x10B, 0x20B):
        return None
    characteristics = u16(optional + 70)
    directories = optional + (112 if magic == 0x20B else 96)
    count = u32(directories - 4)
    table = optional + optional_size

    def offset(rva):
        for i in range(sections):
            section = table + 40 * i
            size = max(u32(section + 8), u32(section + 16))
            start = u32(section + 12)
            if start <= rva < start + size:
                return rva - start + u32(section + 20)
        return None

    extended = 0
    if count > 6:
        debug_rva, debug_size = u32(directories + 6 * 8), u32(directories + 6 * 8 + 4)
        at = offset(debug_rva) if debug_size else None
        for entry in range(debug_size // 28 if at is not None else 0):
            entry_at = at + 28 * entry
            if u32(entry_at + 12) == 20:  # IMAGE_DEBUG_TYPE_EX_DLLCHARACTERISTICS
                extended = u32(u32(entry_at + 24))
    guard = 0
    if count > 10:
        config_rva, config_size = u32(directories + 10 * 8), u32(directories + 10 * 8 + 4)
        at = offset(config_rva) if config_size else None
        # GuardFlags, in IMAGE_LOAD_CONFIG_DIRECTORY64 or 32; the structure says its size.
        flags_at = 0x90 if magic == 0x20B else 0x58
        if at is not None and u32(at) >= flags_at + 4:
            guard = u32(at + flags_at)
    return characteristics, extended, guard


def check_windows_hardening(app):
    """Stops unless every program and library in `app` was built with its mitigations (see
    MITIGATIONS): a variable such as RUSTFLAGS in the build's environment drops them
    silently."""
    for name in REQUIRED:
        if not (app / name).is_file():
            fail(f"no {name} in the Windows application")
    for path in sorted(app.rglob("*")):
        if path.suffix.lower() not in (".exe", ".dll") or not path.is_file():
            continue
        name = path.relative_to(app).as_posix()
        found = pe_characteristics(path.read_bytes())
        if found is None:
            fail(f"{name} is not a Windows program")
        characteristics, extended, guard = found
        expected = MITIGATIONS.get(name.lower(), {CFG, EHCONT})
        missing = [what for flag, what in [(PE_HIGH_ENTROPY_VA, "high-entropy ASLR"),
                                           (PE_DYNAMIC_BASE, "ASLR"), (PE_NX_COMPAT, "DEP")]
                   if not characteristics & flag]
        if CFG in expected and not characteristics & PE_GUARD_CF:
            missing.append(CFG)
        if EHCONT in expected and not guard & PE_GUARD_EH_CONTINUATION:
            missing.append(EHCONT)
        if missing:
            fail(f"{name} was built without {', '.join(missing)}: build it again with "
                 "nothing like RUSTFLAGS, CL or LINK in the environment")
        if name.lower() == NOT_CET and extended & PE_CET_COMPAT:
            fail(f"{name} is CET shadow stack compatible, which Fogata is not yet (see "
                 "app/windows/CMakeLists.txt): build it again with nothing like LINK in "
                 "the environment")


# The Linux bundle's code made with a compiler, and what the build gives it
# (app/linux/CMakeLists.txt and the plugins'): relocations read-only once loaded (RELRO,
# with every symbol bound at start), and libraries found only next to the program
# ($ORIGIN), never in a folder of the computer that built it. Dart's compiled code
# (libapp.so) is data loaded by the engine, with no dynamic section of its own.
LINUX_DATA_ONLY = {"lib/libapp.so"}
# The runner and the plugins' libraries are built with the stack protector; libraries
# built elsewhere (the Flutter engine, libwebrtc, the Rust library) keep their own flags.
# Each of ours has functions with local arrays or objects whose address is taken (strings,
# buffers, GLib and GStreamer structures), which -fstack-protector-strong always guards:
# built with it, each calls __stack_chk_fail, so its absence means the flag was lost. A
# plugin of ours with none of those would have to be named apart here.
LINUX_OURS = re.compile(r"^(fogata|lib/lib\w+_plugin\.so)$")

ELF_MAGIC = b"\x7fELF"


def elf_dynamic(data):
    """What the release checks of the ELF file in `data` (64-bit, little-endian), read as
    the loader reads it, from the program headers: whether it has a RELRO segment, whether
    it binds every symbol at start, and its RPATH and RUNPATH folders; and from its sections
    the names its dynamic symbols use (none when they are stripped). None when it is no such
    file, or one that cannot be read."""
    try:
        return _elf_dynamic(data)
    except (ValueError, IndexError):
        return None


def _elf_dynamic(data):
    def u16(at):
        return int.from_bytes(data[at:at + 2], "little")

    def u32(at):
        return int.from_bytes(data[at:at + 4], "little")

    def u64(at):
        if at + 8 > len(data):
            raise IndexError(at)
        return int.from_bytes(data[at:at + 8], "little")

    def string(at):
        return data[at:data.index(b"\0", at)].decode(errors="replace")

    if len(data) < 64 or data[:4] != ELF_MAGIC or data[4] != 2 or data[5] != 1:
        return None
    phoff, shoff = u64(0x20), u64(0x28)
    phentsize, phnum, shentsize, shnum = u16(0x36), u16(0x38), u16(0x3A), u16(0x3C)
    # Each segment: its type, where it is in the file, its address and its size there.
    segments = [(u32(at), u64(at + 0x08), u64(at + 0x10), u64(at + 0x20))
                for at in (phoff + i * phentsize for i in range(phnum))]
    relro = any(kind == 0x6474E552 for kind, _, _, _ in segments)  # PT_GNU_RELRO

    def offset_of(address):
        """Where in the file the loader puts `address`, by the PT_LOAD segments."""
        for kind, offset, vaddr, size in segments:
            if kind == 1 and vaddr <= address < vaddr + size:
                return address - vaddr + offset
        raise ValueError(f"address {address:#x} outside the file")

    bind_now, paths = False, []
    for kind, offset, _, size in segments:
        if kind != 2:  # PT_DYNAMIC
            continue
        entries = []
        for at in range(offset, offset + size - 15, 16):
            tag, value = u64(at), u64(at + 8)
            if tag == 0:  # DT_NULL
                break
            entries.append((tag, value))
        table = next((value for tag, value in entries if tag == 5), None)  # DT_STRTAB
        for tag, value in entries:
            # DT_BIND_NOW, DT_FLAGS with DF_BIND_NOW, DT_FLAGS_1 with DF_1_NOW.
            if tag == 24 or (tag == 30 and value & 0x8) or (tag == 0x6FFFFFFB and value & 0x1):
                bind_now = True
            if tag in (15, 29):  # DT_RPATH, DT_RUNPATH
                if table is None:
                    raise ValueError("RUNPATH without a string table")
                paths += string(offset_of(table) + value).split(":")

    names = b""
    sections = [shoff + i * shentsize for i in range(shnum)] if shoff else []
    for section in sections:
        if u32(section + 4) == 11:  # SHT_DYNSYM, whose strings are in its linked section
            linked = sections[u32(section + 0x28)]
            at, size = u64(linked + 0x18), u64(linked + 0x20)
            names = data[at:at + size]
    return relro, bind_now, paths, set(names.decode(errors="replace").split("\0"))


def in_application(folder):
    """Whether a RUNPATH or RPATH `folder` is inside the application: $ORIGIN, written
    either way, or a folder under it."""
    return any(folder == origin or folder.startswith(origin + "/")
               for origin in ("$ORIGIN", "${ORIGIN}"))


def check_linux_hardening(bundle):
    """Stops unless every program and library in the Linux `bundle` was built as
    LINUX_DATA_ONLY and LINUX_OURS say: a variable such as LDFLAGS or CFLAGS in the build's
    environment, or a plugin built without the flags, drops them silently."""
    for path in sorted(bundle.rglob("*")):
        if not path.is_file() or path.is_symlink():
            continue
        name = path.relative_to(bundle).as_posix()
        # Only what starts as an ELF file is read whole: the bundle's data can be large.
        with path.open("rb") as file:
            if file.read(4) != ELF_MAGIC or name in LINUX_DATA_ONLY:
                continue
        found = elf_dynamic(path.read_bytes())
        if found is None:
            fail(f"{name} is not a 64-bit Linux program that can be read")
        relro, bind_now, paths, names = found
        missing = [what for ok, what in [(relro, "RELRO"), (bind_now, "BIND_NOW")] if not ok]
        if LINUX_OURS.match(name) and "__stack_chk_fail" not in names:
            missing.append("the stack protector")
        if missing:
            fail(f"{name} was built without {', '.join(missing)}: build it again with "
                 "nothing like CFLAGS or LDFLAGS in the environment")
        outside = [p for p in paths if p and not in_application(p)]
        if outside:
            fail(f"{name} looks for libraries in {', '.join(outside)}, outside the "
                 "application: only $ORIGIN may be in its RUNPATH")


def find_dir_with(root, name):
    for path in sorted(root.rglob(name)):
        return path.parent
    fail(f"no {name} in {root}")


def zip_dir(folder, archive, arcroot, extra=()):
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as z:
        for path in sorted(folder.rglob("*")):
            if path.is_file():
                z.write(path, f"{arcroot}/{path.relative_to(folder).as_posix()}")
        for path in extra:
            z.write(path, path.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--build", type=int, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--min-build", type=int, required=True)
    parser.add_argument("--notes", type=pathlib.Path)
    parser.add_argument("--days", type=int, default=180)
    parser.add_argument("--key", type=pathlib.Path, default=KEY_DIR / "release-key")
    parser.add_argument("--gpg-homedir", type=pathlib.Path, default=FLATPAK_KEY_DIR)
    parser.add_argument("--allow-no-keys", action="store_true",
                        help="make the release although RELEASE_KEYS is empty (tests only)")
    parser.add_argument("--fogata", type=pathlib.Path, default=REPO / "target/release/fogata")
    parser.add_argument("--linux-bundle", type=pathlib.Path)
    parser.add_argument("--build-linux", action="store_true",
                        help="build the Linux bundle here from a clean worktree of the commit")
    parser.add_argument("--extra-libs", type=pathlib.Path)
    parser.add_argument("--windows-zip", type=pathlib.Path)
    parser.add_argument("--macos-zip", type=pathlib.Path)
    parser.add_argument("--macos-arch", choices=["arm64", "x64"], default="arm64")
    parser.add_argument("--out", type=pathlib.Path, required=True)
    args = parser.parse_args()

    commit = checked_out()
    build, version = this_build()
    if (build, version) != (args.build, args.version):
        fail(f"the source says build {build} ({version}); set THIS_BUILD, THIS_VERSION and "
             "BUILD_MARKER first and build every system from that commit")
    if not release_keys_named() and not args.allow_no_keys:
        fail("RELEASE_KEYS in crates/fogata-update/src/lib.rs names no key: no build takes "
             "releases. Add the key (fogata release keygen prints the line) and build again")
    # What each build must hold: this build, these keys, this commit.
    expect = ["--expect-build", build, *expect_keys(release_keys()), "--expect-commit", commit]
    if args.build_linux and args.linux_bundle:
        fail("give --linux-bundle or --build-linux, not both")
    if not (args.build_linux or args.linux_bundle or args.windows_zip or args.macos_zip):
        fail("give at least one build")
    if args.build_linux or args.linux_bundle:
        check_flatpak_key_apart(args.gpg_homedir, args.key)
    if not args.fogata.is_file():
        fail(f"no {args.fogata}: cargo build --release -p fogata")
    check_cli_commit(args.fogata, commit)
    if args.out.exists():
        fail(f"{args.out} is there already")
    release, drive, work = args.out / "release", args.out / "drive", args.out / "work"
    for folder in (release, drive, work):
        folder.mkdir(parents=True)
    fogata = args.fogata
    packages = []
    if args.build_linux:
        args.linux_bundle = build_linux(commit, work)

    if args.linux_bundle:
        refuse_debug_folder(args.linux_bundle)
        check_linux_hardening(args.linux_bundle)
        run(fogata, "release", "pack", *expect, args.linux_bundle,
            "-o", work / "bundle-check.pack")
        (work / "bundle-check.pack").unlink()
        flatpak = [REPO / "tools/beta/flatpak.sh", args.linux_bundle, work / "flatpak",
                   "--gpg-homedir", args.gpg_homedir]
        if args.extra_libs:
            flatpak += ["--extra-libs", args.extra_libs]
        run(*flatpak)
        pack = work / f"Fogata-{version}-linux.pack"
        run(fogata, "release", "pack", work / "flatpak/repo", "-o", pack)
        packages.append(f"linux-flatpak={pack}")
        shutil.copy(work / "flatpak/Fogata.flatpak", drive / f"Fogata-{version}-linux.flatpak")

    if args.windows_zip:
        unzip(args.windows_zip, work / "windows")
        app = find_dir_with(work / "windows", "fogata.exe")
        refuse_debug_folder(app)
        check_windows_hardening(app)
        pack = work / f"Fogata-{version}-windows-x64.pack"
        run(fogata, "release", "pack", *expect, app, "-o", pack)
        packages.append(f"windows-x64={pack}")
        scripts = REPO / "tools/beta/windows"
        write_windows_list(app, work / WINDOWS_LIST)
        zip_dir(app, drive / f"Fogata-{version}-windows-x64.zip", "Fogata",
                extra=[scripts / "Install Fogata.cmd", scripts / "install.ps1",
                       work / WINDOWS_LIST])

    if args.macos_zip:
        unzip(args.macos_zip, work / "macos")
        app = work / "macos/Fogata.app"
        if not (app / "Contents/Info.plist").is_file():
            fail("the macOS zip holds no Fogata.app at its top")
        if not (app / "Contents/Library/Helpers/Fogata Updater.app").is_dir():
            fail("Fogata.app has no updater: run tools/beta/macos-updater.sh before signing")
        refuse_debug_folder(app)
        check_macos_info(app, build)
        pack = work / f"Fogata-{version}-macos-{args.macos_arch}.pack"
        run(fogata, "release", "pack", *expect, app, "-o", pack)
        packages.append(f"macos-{args.macos_arch}={pack}")
        shutil.copy(args.macos_zip, drive / f"Fogata-{version}-macos-{args.macos_arch}.zip")

    (drive / "README.txt").write_text(README.format(
        version=version,
        mac_arch=args.macos_arch,
        mac_note="" if args.macos_arch == "arm64" else "; this one is for Intel Macs",
    ))
    # One passphrase for both: the manifest, and SHA256SUMS of the first installs.
    sign = [fogata, "release", "sign", "--key", args.key, "--build", build, "--version", version,
            "--min-build", args.min_build, "--days", args.days, "--out", release,
            "--sums-of", drive]
    if args.notes:
        sign += ["--notes", args.notes]
    run(*sign, *packages)
    digest = hashlib.sha256((release / "release.json").read_bytes()).hexdigest()
    with open(args.key.parent / "published.log", "a") as log:
        log.write(f"{time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())} build {build} "
                  f"{version} release.json sha256 {digest}\n")
    print(f"\nRelease in {release}; first installs in {drive}.")
    print("Upload drive/ to the pre-release; the pre-release keys' fogata-prerelease lines must be in tools/allowed_signers.")
    print(f"Publish: {fogata} --data-dir <your data folder> release publish {release}")


if __name__ == "__main__":
    main()
