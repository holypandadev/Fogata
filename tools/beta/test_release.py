#!/usr/bin/env python3
"""Tests of tools/beta/release.py: unpacking the zips of other systems, the check of debug and
profile builds, the Windows list of files, the hardening of the Windows and Linux binaries,
the keys a release needs.

    python3 -I tools/beta/test_release.py
"""

import hashlib
import importlib.util
import os
import pathlib
import shutil
import stat
import subprocess
import tempfile
import unittest
import warnings
import sys
import zipfile

sys.dont_write_bytecode = True  # no __pycache__ in the repository
HERE = pathlib.Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


release = load("beta_release", HERE / "release.py")


def make_zip(path, entries):
    """Writes a zip of `entries`: (name, bytes) for files, (name, None) for folders and
    (name, "->target") for links, as ditto stores them."""
    with zipfile.ZipFile(path, "w") as z, warnings.catch_warnings():
        warnings.simplefilter("ignore")  # a repeated name, on purpose
        for name, data in entries:
            info = zipfile.ZipInfo(name)
            if data is None:
                info.external_attr = (stat.S_IFDIR | 0o755) << 16
                z.writestr(info, b"")
            elif isinstance(data, str):
                info.external_attr = (stat.S_IFLNK | 0o777) << 16
                z.writestr(info, data[2:].encode())
            else:
                info.external_attr = (stat.S_IFREG | 0o644) << 16
                z.writestr(info, data)


def elf(relro=True, bind_now="flags", runpath="$ORIGIN", rpath=None, protected=True,
        stripped=False):
    """A 64-bit ELF file with what the Linux checks read: a RELRO segment or not, symbols
    bound at start (by DT_FLAGS, DT_FLAGS_1, DT_BIND_NOW or not at all), its RUNPATH and
    RPATH in the dynamic segment, and the stack protector's symbol among its dynamic ones
    or not; with its sections, or stripped of them. Loaded at an address other than its
    place in the file, as libraries are."""
    base = 0x400000
    strings = bytearray(b"\0")

    def string(text):
        at = len(strings)
        strings.extend(text.encode() + b"\0")
        return at

    if protected:
        string("__stack_chk_fail")
    string("memcpy")
    dynamic = []
    if runpath is not None:
        dynamic.append((29, string(runpath)))
    if rpath is not None:
        dynamic.append((15, string(rpath)))
    if bind_now == "flags":
        dynamic.append((30, 0x8))
    elif bind_now == "flags_1":
        dynamic.append((0x6FFFFFFB, 0x1))
    elif bind_now == "bind_now":
        dynamic.append((24, 0))
    symbols = bytes(24)
    kinds = [1, 2] + ([0x6474E552] if relro else [])
    phoff = 64
    strings_at = phoff + 56 * len(kinds)
    symbols_at = strings_at + len(strings)
    dynamic_at = symbols_at + len(symbols)
    dynamic = [(5, base + strings_at), (10, len(strings))] + dynamic + [(0, 0)]
    dynamic_bytes = b"".join(t.to_bytes(8, "little") + v.to_bytes(8, "little")
                             for t, v in dynamic)
    shoff = dynamic_at + len(dynamic_bytes)
    size = shoff + (0 if stripped else 4 * 64)

    def segment(kind):
        offset, length = (0, size) if kind == 1 else (dynamic_at, len(dynamic_bytes))
        if kind == 0x6474E552:
            offset, length = dynamic_at, len(dynamic_bytes)
        return (kind.to_bytes(4, "little") + bytes(4) + offset.to_bytes(8, "little")
                + (base + offset).to_bytes(8, "little") + bytes(8)
                + length.to_bytes(8, "little") + length.to_bytes(8, "little") + bytes(8))

    def section(kind, at, length, link):
        return ((0).to_bytes(4, "little") + kind.to_bytes(4, "little") + bytes(16)
                + at.to_bytes(8, "little") + length.to_bytes(8, "little")
                + link.to_bytes(4, "little") + bytes(20))

    header = (b"\x7fELF" + bytes([2, 1, 1]) + bytes(9) + (3).to_bytes(2, "little")
              + (62).to_bytes(2, "little") + (1).to_bytes(4, "little") + bytes(8)
              + phoff.to_bytes(8, "little") + (0 if stripped else shoff).to_bytes(8, "little")
              + bytes(4) + (64).to_bytes(2, "little") + (56).to_bytes(2, "little")
              + len(kinds).to_bytes(2, "little") + (64).to_bytes(2, "little")
              + (0 if stripped else 4).to_bytes(2, "little") + bytes(2))
    program = b"".join(segment(kind) for kind in kinds)
    sections = b"" if stripped else (
        bytes(64) + section(3, strings_at, len(strings), 0)
        + section(11, symbols_at, len(symbols), 1)
        + section(6, dynamic_at, len(dynamic_bytes), 1))
    return header + program + bytes(strings) + symbols + dynamic_bytes + sections


class Unzip(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="fogata-release-test-"))
        self.dest = self.tmp / "work" / "macos"
        self.outside = self.tmp / "outside"
        self.outside.mkdir()

    def tearDown(self):
        for root, dirs, files in os.walk(self.tmp, topdown=False):
            for name in files + dirs:
                path = os.path.join(root, name)
                if os.path.islink(path) or not os.path.isdir(path):
                    os.unlink(path)
                else:
                    os.rmdir(path)
        os.rmdir(self.tmp)

    def unzip(self, entries):
        archive = self.tmp / "app.zip"
        make_zip(archive, entries)
        release.unzip(archive, self.dest)

    def nothing_outside(self):
        self.assertEqual(list(self.outside.iterdir()), [])
        self.assertEqual(sorted(p.name for p in self.tmp.iterdir()),
                         ["app.zip", "outside", "work"])

    @unittest.skipIf(os.name == "nt", "the release is made on Linux or macOS; links a Mac "
                     "bundle holds do not resolve on Windows as they do there")
    def test_an_app_bundle_keeps_its_links(self):
        self.unzip([
            ("Fogata.app/", None),
            ("Fogata.app/Contents/Frameworks/F.framework/Versions/A/F", b"engine"),
            ("Fogata.app/Contents/Frameworks/F.framework/Versions/Current", "->A"),
            ("Fogata.app/Contents/Frameworks/F.framework/F", "->Versions/Current/F"),
            ("Fogata.app/Contents/Frameworks/G.framework/G", "->../F.framework/F"),
        ])
        frameworks = self.dest / "Fogata.app/Contents/Frameworks"
        self.assertEqual((frameworks / "F.framework/F").read_bytes(), b"engine")
        self.assertEqual((frameworks / "G.framework/G").read_bytes(), b"engine")
        self.assertTrue((frameworks / "F.framework/Versions/Current").is_symlink())

    def test_a_file_is_never_written_through_a_link(self):
        # The link comes first and leads out; the file after it would land outside.
        with self.assertRaises(SystemExit):
            self.unzip([
                ("app/out", "->../../../outside"),
                ("app/out/evil", b"evil"),
            ])
        self.nothing_outside()

    def test_a_link_out_is_refused(self):
        for target in ["../../../outside", "/tmp", "a/../../../..", "", "a//b", "./a",
                       "..\\..\\outside"]:
            with self.subTest(target=target):
                with self.assertRaises(SystemExit):
                    self.unzip([("app/x/link", "->" + target)])
                self.nothing_outside()
                self.tearDown()
                self.setUp()

    @unittest.skipIf(os.name == "nt", "the release is made on Linux or macOS; links a Mac "
                     "bundle holds do not resolve on Windows as they do there")
    def test_a_link_cannot_climb_out_through_another_link(self):
        # "deep" leads one folder down; a link inside it climbs from where it really is.
        with self.assertRaises(SystemExit):
            self.unzip([
                ("app/sub/", None),
                ("app/deep", "->sub"),
                ("app/deep/up", "->../../.."),
            ])
        self.nothing_outside()
        self.tearDown()
        self.setUp()
        self.unzip([
            ("app/sub/", None),
            ("app/deep", "->sub"),
            ("app/deep/up", "->../.."),
        ])
        self.assertTrue(os.path.samefile(self.dest / "app/sub/up", self.dest))

    def test_a_zip_with_backslashes_unpacks_its_folders(self):
        # As PowerShell 5's Compress-Archive writes them: backslashes between names, folders
        # ending with one, some marked as folders only by their MS-DOS attribute.
        archive = self.tmp / "app.zip"
        with zipfile.ZipFile(archive, "w") as z:
            for name, data, attr in [
                ("data\\", b"", 0x10),
                ("data\\flutter_assets\\", b"", 0x10),
                ("data\\flutter_assets\\fonts", b"", 0x10),
                ("data\\flutter_assets\\fonts\\a.ttf", b"font", 0x20),
                ("data\\app.so", b"so", 0x20),
                ("fogata.exe", b"exe", 0x20),
            ]:
                info = zipfile.ZipInfo(name)
                info.create_system = 0  # MS-DOS
                info.external_attr = attr
                z.writestr(info, data)
        release.unzip(archive, self.dest)
        self.assertEqual((self.dest / "data/app.so").read_bytes(), b"so")
        self.assertEqual((self.dest / "data/flutter_assets/fonts/a.ttf").read_bytes(), b"font")
        self.assertEqual((self.dest / "fogata.exe").read_bytes(), b"exe")
        self.nothing_outside()

    def test_names_that_climb_or_repeat_are_not_unpacked(self):
        self.unzip([("../evil", b"x"), ("/abs", b"x"), ("app/ok", b"ok")])
        self.assertEqual((self.dest / "app/ok").read_bytes(), b"ok")
        self.nothing_outside()
        with self.assertRaises(SystemExit):
            self.tearDown()
            self.setUp()
            self.unzip([("app/l", "->a"), ("app/l", "->b")])


class Builds(unittest.TestCase):
    def test_debug_and_profile_builds_are_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = pathlib.Path(tmp) / "bundle"
            (app / "lib").mkdir(parents=True)
            engine = app / "lib/libflutter_linux_gtk.so"
            engine.write_bytes(b"engine")
            release.refuse_debug_folder(app)
            engine.write_bytes(b"... Dart VM service ...")
            with self.assertRaises(SystemExit):
                release.refuse_debug_folder(app)
            engine.write_bytes(b"engine")
            (app / "data/flutter_assets").mkdir(parents=True)
            (app / "data/flutter_assets/kernel_blob.bin").write_bytes(b"")
            with self.assertRaises(SystemExit):
                release.refuse_debug_folder(app)
            # A Windows profile build too.
            win = pathlib.Path(tmp) / "Release"
            win.mkdir()
            (win / "flutter_windows.dll").write_bytes(b"x Dart VM service x")
            with self.assertRaises(SystemExit):
                release.refuse_debug_folder(win)
            (win / "flutter_windows.dll").write_bytes(b"x")
            release.refuse_debug_folder(win)
            # No engine at all.
            empty = pathlib.Path(tmp) / "empty"
            empty.mkdir()
            with self.assertRaises(SystemExit):
                release.refuse_debug_folder(empty)

    def test_a_debug_build_in_a_zip_with_backslashes_is_seen(self):
        public = release.load_public_release()
        names = ["fogata.exe", "flutter_windows.dll", "data\\flutter_assets\\kernel_blob.bin"]
        self.assertEqual(public.build_kind(names, lambda n: b"x"),
                         ("debug", "data\\flutter_assets\\kernel_blob.bin"))
        self.assertEqual(public.build_kind(["data\\app.so", "x\\flutter_windows.dll"],
                                           lambda n: b"Dart VM service"),
                         ("profile", "x\\flutter_windows.dll"))

    def test_the_windows_list_names_every_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = pathlib.Path(tmp) / "Release"
            (app / "data").mkdir(parents=True)
            (app / "fogata.exe").write_bytes(b"exe")
            (app / "data/app.so").write_bytes(b"so")
            listing = pathlib.Path(tmp) / "Fogata.sha256"
            release.write_windows_list(app, listing)
            self.assertEqual(listing.read_text(encoding="utf-8"), (
                f"{hashlib.sha256(b'so').hexdigest()}  data/app.so\n"
                f"{hashlib.sha256(b'exe').hexdigest()}  fogata.exe\n"
            ))

    def test_windows_binaries_must_carry_their_mitigations(self):
        base = release.PE_HIGH_ENTROPY_VA | release.PE_DYNAMIC_BASE | release.PE_NX_COMPAT
        full = base | release.PE_GUARD_CF
        ehcont = release.PE_GUARD_EH_CONTINUATION | 0x100
        self.assertEqual(release.pe_characteristics(pe(full, cet=True, guard=ehcont)),
                         (full, 1, ehcont))
        self.assertEqual(release.pe_characteristics(pe(full, cet=False)), (full, 0, 0))
        self.assertEqual(release.pe_characteristics(pe(full, pe32=True, guard=ehcont)),
                         (full, 0, ehcont))
        self.assertIsNone(release.pe_characteristics(b"MZ not a program" + bytes(100)))
        with tempfile.TemporaryDirectory() as tmp:
            app = pathlib.Path(tmp)
            good = {
                "fogata.exe": pe(full, cet=False, guard=ehcont),
                "fogata_ffi.dll": pe(full),
                # Compatible from its own build: allowed, it does not count.
                "libwebrtc.dll": pe(full, cet=True),
                "flutter_windows.dll": pe(base),
                "tray_manager_plugin.dll": pe(full, cet=False, guard=ehcont),
            }
            for name, data in good.items():
                (app / name).write_bytes(data)
            (app / "data").mkdir()
            (app / "data/app.so").write_bytes(b"not a Windows program")
            release.check_windows_hardening(app)
            # No debug directory at all says not compatible too.
            (app / "fogata.exe").write_bytes(pe(full, guard=ehcont))
            release.check_windows_hardening(app)
            (app / "fogata.exe").write_bytes(good["fogata.exe"])
            bad = [
                # Built with RUSTFLAGS in the environment: no Control Flow Guard.
                ("fogata_ffi.dll", pe(full & ~release.PE_GUARD_CF)),
                # The program made CET compatible: the decision is not lost silently.
                ("fogata.exe", pe(full, cet=True, guard=ehcont)),
                ("fogata.exe", pe(full, cet=False)),
                # A plugin is checked like the runner, whatever its name.
                ("tray_manager_plugin.dll", pe(full, cet=False)),
                ("tray_manager_plugin.dll", pe(full & ~release.PE_GUARD_CF, guard=ehcont)),
                ("libwebrtc.dll", pe(full & ~release.PE_DYNAMIC_BASE, cet=True)),
                ("flutter_windows.dll", pe(base & ~release.PE_NX_COMPAT)),
                # A library nobody named is held to everything.
                ("Other.DLL", pe(full)),
                ("data/other.exe", pe(base, guard=ehcont)),
                ("fake.dll", b"MZ not a program" + bytes(100)),
            ]
            for name, data in bad:
                with self.subTest(name=name):
                    path = app / name
                    path.write_bytes(data)
                    with self.assertRaises(SystemExit):
                        release.check_windows_hardening(app)
                    if name in good:
                        path.write_bytes(good[name])
                    else:
                        path.unlink()
            release.check_windows_hardening(app)
            for name in ("fogata.exe", "fogata_ffi.dll"):
                with self.subTest(missing=name):
                    (app / name).unlink()
                    with self.assertRaises(SystemExit):
                        release.check_windows_hardening(app)
                    (app / name).write_bytes(good[name])

    def test_linux_binaries_must_carry_their_hardening(self):
        self.assertEqual(release.elf_dynamic(elf()),
                         (True, True, ["$ORIGIN"], {"", "__stack_chk_fail", "memcpy", "$ORIGIN"}))
        # Read from the dynamic segment, as the loader does: the sections may be gone.
        self.assertEqual(release.elf_dynamic(elf(stripped=True, runpath="${ORIGIN}/lib")),
                         (True, True, ["${ORIGIN}/lib"], {""}))
        for flag in ("flags", "flags_1", "bind_now"):
            self.assertTrue(release.elf_dynamic(elf(bind_now=flag))[1], flag)
        self.assertFalse(release.elf_dynamic(elf(bind_now=None))[1])
        self.assertIsNone(release.elf_dynamic(b"\x7fELF" + bytes([1, 1]) + bytes(100)))
        self.assertIsNone(release.elf_dynamic(elf()[:200]))
        with tempfile.TemporaryDirectory() as tmp:
            bundle = pathlib.Path(tmp)
            (bundle / "lib").mkdir()
            (bundle / "data").mkdir()
            good = {
                "fogata": elf(runpath="$ORIGIN/lib"),
                "lib/libfogata_video_plugin.so": elf(bind_now="flags_1"),
                "lib/liburl_launcher_linux_plugin.so": elf(bind_now="bind_now",
                                                           runpath="${ORIGIN}"),
                # Built elsewhere and stripped of its sections.
                "lib/libflutter_linux_gtk.so": elf(runpath=None, protected=False,
                                                   stripped=True),
                # Built elsewhere, without the stack protector's symbol: allowed.
                "lib/libwebrtc.so": elf(runpath=None, protected=False),
                "lib/libfogata_ffi.so": elf(runpath=None),
                # Dart's compiled code: data, not checked.
                "lib/libapp.so": elf(relro=False, bind_now=None, protected=False),
            }
            for name, data in good.items():
                (bundle / name).write_bytes(data)
            (bundle / "data/icudtl.dat").write_bytes(b"not a program")
            release.check_linux_hardening(bundle)
            bad = [
                # Built on a computer whose plugins kept their build folder.
                ("lib/libdesktop_drop_plugin.so", elf(runpath="/home/someone/app/build/plugins")),
                ("lib/libdesktop_drop_plugin.so", elf(runpath="$ORIGIN:/usr/local/lib")),
                ("lib/libdesktop_drop_plugin.so", elf(runpath="$ORIGINAL/lib")),
                # Ours, stripped: whether it was protected cannot be told, so it is refused.
                ("lib/libfogata_video_plugin.so", elf(stripped=True)),
                ("lib/libwebrtc.so", elf(runpath=None, rpath="/opt/webrtc", protected=False)),
                # Built with LDFLAGS or CFLAGS in the environment.
                ("fogata", elf(relro=False, runpath="$ORIGIN/lib")),
                ("fogata", elf(bind_now=None, runpath="$ORIGIN/lib")),
                ("fogata", elf(protected=False, runpath="$ORIGIN/lib")),
                ("lib/libfogata_video_plugin.so", elf(protected=False)),
                ("lib/libflutter_linux_gtk.so", elf(relro=False)),
                ("lib/fake.so", b"\x7fELF" + bytes([1, 1]) + bytes(100)),
            ]
            for name, data in bad:
                with self.subTest(name=name):
                    path = bundle / name
                    path.write_bytes(data)
                    with self.assertRaises(SystemExit):
                        release.check_linux_hardening(bundle)
                    if name in good:
                        path.write_bytes(good[name])
                    else:
                        path.unlink()
            release.check_linux_hardening(bundle)

    def test_a_file_not_in_the_signed_list_fails_the_public_check(self):
        public = release.load_public_release()
        with tempfile.TemporaryDirectory() as tmp:
            folder = pathlib.Path(tmp)
            (folder / "Fogata-1-linux-x64.tar.gz").write_bytes(b"pkg")
            digest = hashlib.sha256(b"pkg").hexdigest()
            (folder / "SHA256SUMS").write_text(f"{digest}  Fogata-1-linux-x64.tar.gz\n")
            (folder / "SHA256SUMS.sig").write_text("signature")
            args = type(
                "Args", (), {"folder": folder, "signers": folder / "allowed", "prerelease": False}
            )()
            (folder / "allowed").write_text("fogata-release ssh-ed25519 AAAA\n")
            # The signature is taken as good here: what is checked is the rest.
            holds = public.signature_holds
            public.signature_holds = lambda path, signers, principal: True
            try:
                with self.assertRaises(SystemExit):  # "allowed" itself is not listed
                    public.verify_command(args)
                (folder / "allowed").rename(folder.parent / f"{folder.name}-allowed")
                args.signers = folder.parent / f"{folder.name}-allowed"
                public.verify_command(args)
                (folder / "Fogata-1-windows-x64.zip").write_bytes(b"not signed")
                with self.assertRaises(SystemExit):
                    public.verify_command(args)
            finally:
                public.signature_holds = holds
                (folder.parent / f"{folder.name}-allowed").unlink(missing_ok=True)

    @unittest.skipUnless(shutil.which("ssh-keygen"), "ssh-keygen is not installed")
    def test_a_key_of_the_beta_never_vouches_for_a_public_release(self):
        public = release.load_public_release()
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            keys = {}
            for name in ("public", "beta"):
                key = tmp / f"{name}-key"
                subprocess.run(
                    ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", name, "-f", str(key)],
                    check=True,
                )
                keys[name] = key
            line = lambda principal, namespaces, name: (
                f'{principal} namespaces="{namespaces}" '
                + " ".join((tmp / f"{name}-key.pub").read_text().split()[:2])
                + "\n"
            )
            signers = tmp / "allowed_signers"
            signers.write_text(
                line(public.PRINCIPAL, "file,git", "public")
                + line(public.PRERELEASE_PRINCIPAL, "file", "beta")
            )
            folder = tmp / "drive"
            folder.mkdir()
            (folder / "Fogata-1-linux-x64.tar.gz").write_bytes(b"pkg")
            sums = folder / "SHA256SUMS"
            sums.write_text(f"{hashlib.sha256(b'pkg').hexdigest()}  Fogata-1-linux-x64.tar.gz\n")

            def args(prerelease):
                return type(
                    "Args", (), {"folder": folder, "signers": signers, "prerelease": prerelease}
                )()

            def sign_with(name):
                (folder / "SHA256SUMS.sig").unlink(missing_ok=True)
                subprocess.run(
                    ["ssh-keygen", "-q", "-Y", "sign", "-f", str(keys[name]), "-n", "file",
                     str(sums)],
                    check=True,
                    capture_output=True,
                )

            # Signed by the beta's key: a list of the beta's first installs, not a release.
            sign_with("beta")
            public.verify_command(args(prerelease=True))
            with self.assertRaises(SystemExit):
                public.verify_command(args(prerelease=False))
            self.assertFalse(public.signature_holds(sums, signers))
            # Signed by the key of public releases: a release, not a list of the beta.
            sign_with("public")
            public.verify_command(args(prerelease=False))
            with self.assertRaises(SystemExit):
                public.verify_command(args(prerelease=True))
            # A file with only the beta's line has no key to check a release with.
            signers.write_text(line(public.PRERELEASE_PRINCIPAL, "file", "beta"))
            with self.assertRaises(SystemExit):
                public.verify_command(args(prerelease=False))

    def test_the_readme_checks_the_list_as_signed_by_the_beta(self):
        public = release.load_public_release()
        command = (
            f"-I {public.PRERELEASE_PRINCIPAL} -n {public.NAMESPACE} -s SHA256SUMS.sig"
        )
        self.assertIn(command, release.README)
        self.assertNotIn(f"-I {public.PRINCIPAL} ", release.README)

    def test_the_macos_bundle_must_carry_this_build(self):
        import plistlib
        with tempfile.TemporaryDirectory() as tmp:
            app = pathlib.Path(tmp) / "Fogata.app"
            (app / "Contents").mkdir(parents=True)
            plist = app / "Contents/Info.plist"
            for info, good in [
                ({"CFBundleIdentifier": "dev.holypanda.fogata", "CFBundleVersion": "7"}, True),
                ({"CFBundleIdentifier": "dev.holypanda.fogata", "CFBundleVersion": "6"}, False),
                ({"CFBundleIdentifier": "dev.holypanda.fogata", "CFBundleVersion": "7.0"}, False),
                ({"CFBundleIdentifier": "dev.holypanda.fogata"}, False),
                ({"CFBundleIdentifier": "dev.holypanda.other", "CFBundleVersion": "7"}, False),
            ]:
                with self.subTest(info=info):
                    plist.write_bytes(plistlib.dumps(info))
                    if good:
                        release.check_macos_info(app, 7)
                    else:
                        with self.assertRaises(SystemExit):
                            release.check_macos_info(app, 7)
            plist.write_bytes(b"not a plist")
            with self.assertRaises(SystemExit):
                release.check_macos_info(app, 7)

    def test_the_flatpak_key_is_kept_apart_from_the_release_keys(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            release_dir = tmp / "fogata-release"
            release_dir.mkdir()
            (release_dir / "release-key").write_text("sealed")
            rescue_dir = tmp / "stick/fogata-rescue"
            rescue_dir.mkdir(parents=True)
            (rescue_dir / "rescue-key.pub").write_text("public")
            apart = tmp / "fogata-flatpak-key/gnupg"
            apart.mkdir(parents=True)
            key = release_dir / "release-key"
            release.check_flatpak_key_apart(apart, key)
            for beside in (release_dir / "gnupg", release_dir, rescue_dir / "gnupg"):
                with self.subTest(gpg_homedir=beside):
                    beside.mkdir(parents=True, exist_ok=True)
                    with self.assertRaises(SystemExit):
                        release.check_flatpak_key_apart(beside, key)
            # The release key given elsewhere, with nothing else beside the GnuPG home.
            other = tmp / "elsewhere/gnupg"
            other.mkdir(parents=True)
            with self.assertRaises(SystemExit):
                release.check_flatpak_key_apart(other, other / "release-key")

    def test_a_release_needs_both_keys(self):
        head = "pub const RELEASE_KEYS: &[ReleaseKey] = &["
        release_line = key_line("release", 1)
        rescue_line = key_line("rescue", 3)
        self.assertFalse(release.release_keys_named(head + "];"))
        self.assertFalse(release.release_keys_named(head + release_line + "];"))
        self.assertFalse(release.release_keys_named(head + rescue_line + "];"))
        self.assertTrue(release.release_keys_named(head + release_line + rescue_line + "];"))
        with self.assertRaises(SystemExit):
            release.release_keys_named("pub const RELEASE_KEYS: &[[u8; 32]] = &[];")
        # The source as it is now reads.
        release.release_keys_named()

    def test_the_keys_of_the_source_are_what_each_build_must_trust(self):
        head = "pub const RELEASE_KEYS: &[ReleaseKey] = &[\n"
        source = (head + "    " + key_line("release", 1) + "\n"
                  + "    // " + key_line("release", 9) + " (the old one)\n"
                  + "    /* " + key_line("rescue", 8) + " */\n"
                  + "    " + key_line("rescue", 3) + "\n];")
        keys = release.release_keys(source)
        self.assertEqual(keys, [("release", bytes(range(1, 33)).hex()),
                                ("rescue", bytes(range(3, 35)).hex())])
        self.assertEqual(release.expect_keys(keys), [
            "--expect-key", bytes(range(1, 33)).hex(),
            "--expect-rescue-key", bytes(range(3, 35)).hex(),
        ])
        # A key of the wrong length, or anything that is not a key, stops the release
        # rather than be left out of the check.
        for bad in ["ReleaseKey::release([0x01, 0x02]),",
                    key_line("release", 1).replace("0x01", "1"),
                    "ReleaseKey::release(KEY),",
                    key_line("release", 1) + " junk"]:
            with self.subTest(bad=bad), self.assertRaises(SystemExit):
                release.release_keys(head + bad + "\n];")
        self.assertEqual(release.release_keys(head + "];"), [])


def pe(characteristics, cet=None, pe32=False, guard=None):
    """A minimal PE file with these DllCharacteristics; when `cet` is given, a debug directory
    whose extended characteristics say CET compatible or not; when `guard` is, a load
    configuration with these GuardFlags."""
    def u16(v):
        return v.to_bytes(2, "little")

    def u32(v):
        return v.to_bytes(4, "little")

    optional_size = 96 + 16 * 8 if pe32 else 112 + 16 * 8
    header = 0x80
    table = header + 24 + optional_size
    raw = 0x400  # the one section, at RVA 0x1000
    directories = [(0, 0)] * 16
    if cet is not None:
        directories[6] = (0x1000, 28)
    flags_at = 0x58 if pe32 else 0x90
    if guard is not None:
        directories[10] = (0x1140, flags_at + 4)
    optional = bytearray(optional_size)
    optional[0:2] = u16(0x10B if pe32 else 0x20B)
    optional[70:72] = u16(characteristics)
    start = 96 if pe32 else 112
    optional[start - 4:start] = u32(16)
    for i, (rva, size) in enumerate(directories):
        optional[start + 8 * i:start + 8 * i + 8] = u32(rva) + u32(size)
    data = bytearray(raw + 0x200)
    data[0:2] = b"MZ"
    data[0x3C:0x40] = u32(header)
    data[header:header + 4] = b"PE\0\0"
    data[header + 6:header + 8] = u16(1)
    data[header + 20:header + 22] = u16(optional_size)
    data[header + 24:table] = optional
    section = u32(0x200) + u32(0x1000) + u32(0x200) + u32(raw)
    data[table + 8:table + 24] = section
    if cet is not None:
        entry = u32(0) * 3 + u32(20) + u32(4) + u32(0x1100) + u32(raw + 0x100)
        data[raw:raw + 28] = entry
        data[raw + 0x100:raw + 0x104] = u32(1 if cet else 0)
    if guard is not None:
        config = raw + 0x140
        data[config:config + 4] = u32(flags_at + 4)
        data[config + flags_at:config + flags_at + 4] = u32(guard)
    return bytes(data)


def key_line(role, first):
    """A line of RELEASE_KEYS for a key of `role` whose bytes count up from `first`."""
    return (f"ReleaseKey::{role}(["
            + ", ".join(f"0x{b:02x}" for b in range(first, first + 32)) + "]),")


@unittest.skipIf(os.name == "nt", "the Linux build runs on Linux")
class LinuxBuild(unittest.TestCase):
    """build_linux: the bundle comes from the commit, built in a worktree of its own."""

    def test_the_bundle_is_built_from_the_commit_and_not_from_the_checkout(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            repo, work, bin = tmp / "repo", tmp / "work", tmp / "bin"
            for folder in (repo, work, bin):
                folder.mkdir()
            env = dict(os.environ, GIT_AUTHOR_NAME="Bea", GIT_AUTHOR_EMAIL="bea@example.com",
                       GIT_COMMITTER_NAME="Bea", GIT_COMMITTER_EMAIL="bea@example.com",
                       GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")

            def git(*args):
                return subprocess.run(["git", "-C", str(repo), *args], env=env, check=True,
                                      capture_output=True, text=True).stdout.strip()

            git("init", "-q")
            (repo / "app").mkdir()
            (repo / "app/source.txt").write_text("committed\n")
            git("add", "-A")
            git("commit", "-q", "-m", "one")
            commit = git("rev-parse", "HEAD")
            # What this checkout holds on top of the commit, and an old build's leftovers.
            (repo / "app/source.txt").write_text("changed, not committed\n")
            (repo / "app/build/linux/x64/release/bundle").mkdir(parents=True)
            (repo / "app/build/linux/x64/release/bundle/old.so").write_text("old build\n")
            downloads = repo / "app/third_party/flutter_webrtc/third_party/downloads"
            downloads.mkdir(parents=True)
            (downloads / "libwebrtc.zip").write_text("zip\n")
            # A flutter that records how it was run and builds a bundle out of the app folder.
            flutter = bin / "flutter"
            flutter.write_text("#!/bin/sh\n"
                               "echo \"$* | $PWD | RUSTFLAGS=${RUSTFLAGS-unset} "
                               "CARGO_TARGET_DIR=${CARGO_TARGET_DIR-unset} CXXFLAGS=${CXXFLAGS-unset} "
                               "libwebrtc=$(cat third_party/flutter_webrtc/third_party/downloads/"
                               "libwebrtc.zip)\" >> \"$CALLS\"\n"
                               'if [ "$1" = build ]; then\n'
                               "  mkdir -p build/linux/x64/release/bundle/lib\n"
                               "  cp source.txt build/linux/x64/release/bundle/lib/lib.so\n"
                               "fi\n")
            flutter.chmod(0o755)
            calls = tmp / "calls"
            old = dict(os.environ)
            os.environ.update(PATH=f"{bin}:{old['PATH']}", CALLS=str(calls), RUSTFLAGS="-C opt-level=0",
                              CXXFLAGS="-O0", CARGO_TARGET_DIR=str(tmp / "mine"),
                              GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")
            try:
                bundle = release.build_linux(commit, work, repo)
            finally:
                os.environ.clear()
                os.environ.update(old)
            self.assertEqual((bundle / "lib/lib.so").read_text(), "committed\n")
            self.assertFalse((bundle / "old.so").exists())
            lines = calls.read_text().splitlines()
            self.assertEqual(len(lines), 2)
            self.assertIn("pub get --offline", lines[0])
            self.assertIn("build linux --release", lines[1])
            for line in lines:
                self.assertIn(f"{work}/linux-tree/app", line)
                self.assertIn("RUSTFLAGS=unset", line)
                self.assertIn(f"CARGO_TARGET_DIR={work}/linux-target", line)
                self.assertIn("libwebrtc=zip", line)
            # Nothing is left behind but the bundle: no worktree, no target folder.
            self.assertEqual(sorted(p.name for p in work.iterdir()), ["linux-bundle"])
            self.assertNotIn("linux-tree", git("worktree", "list"))
            self.assertEqual((repo / "app/source.txt").read_text(), "changed, not committed\n")

    def test_a_build_that_fails_leaves_no_worktree(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = pathlib.Path(tmp)
            repo, work, bin = tmp / "repo", tmp / "work", tmp / "bin"
            for folder in (repo, work, bin):
                folder.mkdir()
            env = dict(os.environ, GIT_AUTHOR_NAME="Bea", GIT_AUTHOR_EMAIL="bea@example.com",
                       GIT_COMMITTER_NAME="Bea", GIT_COMMITTER_EMAIL="bea@example.com",
                       GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")

            def git(*args):
                return subprocess.run(["git", "-C", str(repo), *args], env=env, check=True,
                                      capture_output=True, text=True).stdout.strip()

            git("init", "-q")
            (repo / "app").mkdir()
            (repo / "app/source.txt").write_text("x\n")
            git("add", "-A")
            git("commit", "-q", "-m", "one")
            (bin / "flutter").write_text("#!/bin/sh\nexit 1\n")
            (bin / "flutter").chmod(0o755)
            old = dict(os.environ)
            os.environ.update(PATH=f"{bin}:{old['PATH']}", GIT_CONFIG_GLOBAL=os.devnull,
                              GIT_CONFIG_NOSYSTEM="1")
            try:
                with self.assertRaises(subprocess.CalledProcessError):
                    release.build_linux(git("rev-parse", "HEAD"), work, repo)
            finally:
                os.environ.clear()
                os.environ.update(old)
            self.assertNotIn("linux-tree", git("worktree", "list"))
            self.assertEqual(list(work.iterdir()), [])


class Checkout(unittest.TestCase):
    def test_a_release_is_made_from_a_commit_with_no_changes(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp)
            env = dict(os.environ, GIT_AUTHOR_NAME="Bea", GIT_AUTHOR_EMAIL="bea@example.com",
                       GIT_COMMITTER_NAME="Bea", GIT_COMMITTER_EMAIL="bea@example.com",
                       GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")

            def git(*args):
                return subprocess.run(["git", "-C", str(repo), *args], env=env, check=True,
                                      capture_output=True, text=True).stdout.strip()

            git("init", "-q")
            (repo / "lib.rs").write_text("one\n")
            git("add", "lib.rs")
            git("commit", "-q", "-m", "one")
            self.assertEqual(release.checked_out(repo), git("rev-parse", "HEAD"))
            # A file not kept by git does not count unless it is among the sources; a change
            # to one kept does.
            (repo / "notes.txt").write_text("notes\n")
            release.checked_out(repo)
            (repo / "app/lib").mkdir(parents=True)
            (repo / "app/lib/new.dart").write_text("new\n")
            with self.assertRaises(SystemExit):
                release.checked_out(repo)
            (repo / "app/lib/new.dart").unlink()
            (repo / "crates").mkdir()
            (repo / "crates/new.rs").write_text("new\n")
            with self.assertRaises(SystemExit):
                release.checked_out(repo)
            (repo / "crates/new.rs").unlink()
            release.checked_out(repo)
            (repo / "lib.rs").write_text("two\n")
            with self.assertRaises(SystemExit):
                release.checked_out(repo)
            git("add", "lib.rs")
            with self.assertRaises(SystemExit):
                release.checked_out(repo)


if __name__ == "__main__":
    unittest.main()
