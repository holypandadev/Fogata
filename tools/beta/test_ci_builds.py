#!/usr/bin/env python3
"""Tests of tools/beta/ci_builds.py: the commit markers a build must carry, release.py's
checks of each build (check-build), and the fetch of a run's builds, with a stand-in for gh,
so nothing is downloaded.

    python3 -I tools/beta/test_ci_builds.py
"""

import hashlib
import importlib.util
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile

sys.dont_write_bytecode = True  # no __pycache__ in the repository
HERE = pathlib.Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ci_builds = load("ci_builds", HERE / "ci_builds.py")
# The makers of program headers of release.py's tests (elf, pe, macho).
maker = load("test_release", HERE / "test_release.py")
COMMIT = "a" * 40
OTHER = "b" * 40
TEAM = "ABCDE12345"


def make_zip(path, files):
    """A zip of {name: bytes}, every file executable, as zip -X and ditto store them."""
    with zipfile.ZipFile(path, "w") as z:
        for name, data in files.items():
            info = zipfile.ZipInfo(name)
            info.external_attr = 0o100755 << 16
            z.writestr(info, data)


class Gh:
    """Stands in for gh: a run, the artifacts it uploaded and the attestations GitHub holds."""

    def __init__(self, root):
        self.root = root
        self.view = {"workflowName": "Release build", "headSha": COMMIT,
                     "status": "completed", "conclusion": "success"}
        self.artifacts = root / "artifacts"
        self.attested = {}  # sha256 -> the commit its attestation names
        self.calls = []

    def add(self, artifact, name, data_or_files, attest=True):
        folder = self.artifacts / artifact
        folder.mkdir(parents=True, exist_ok=True)
        path = folder / name
        if isinstance(data_or_files, dict):
            make_zip(path, data_or_files)
        else:
            path.write_bytes(data_or_files)
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if attest:
            self.attested[digest] = COMMIT
        return digest

    def sums(self, digests):
        folder = self.artifacts / ci_builds.SUMS_ARTIFACT
        folder.mkdir(parents=True, exist_ok=True)
        (folder / ci_builds.SUMS_FILE).write_text(
            "".join(f"{digest}  {name}\n" for name, digest in digests.items()))

    def __call__(self, *args):
        self.calls.append(args)
        if args[:2] == ("run", "view"):
            return json.dumps(self.view)
        if args[:2] == ("run", "download"):
            into = pathlib.Path(args[args.index("--dir") + 1])
            names = [args[i + 1] for i, a in enumerate(args) if a == "--name"]
            for name in names:
                if (self.artifacts / name).is_dir():
                    shutil.copytree(self.artifacts / name, into / name)
            return ""
        if args[:2] == ("attestation", "verify"):
            assert args[args.index("--repo") + 1] == "holypandadev/Fogata"
            assert args[args.index("--signer-workflow") + 1] == \
                "holypandadev/Fogata/.github/workflows/release-build.yml"
            assert "--deny-self-hosted-runners" in args
            digest = hashlib.sha256(pathlib.Path(args[2]).read_bytes()).hexdigest()
            commit = args[args.index("--source-digest") + 1]
            if self.attested.get(digest) != commit:
                raise SystemExit("gh: no attestation verifies")  # gh fails: so does gh()
            return json.dumps([{"verificationResult": {"statement": {
                "subject": [{"name": pathlib.Path(args[2]).name,
                             "digest": {"sha256": digest}}]}}}])
        raise AssertionError(f"unexpected gh {args}")


def helper_binary(team):
    return b"\xcf\xfa\xed\xfe..." + f"fogata-updater-team:{team};".encode() + b"..."


class Fetch(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="fogata-ci-builds-test-"))
        self.gh = Gh(self.tmp)
        self.out = self.tmp / "out"
        marker = f"fogata-commit-marker:{COMMIT};".encode()
        self.digests = {
            "Fogata-linux-x64-bundle.zip": self.gh.add(
                "fogata-linux-x64", "Fogata-linux-x64-bundle.zip",
                {"bundle/fogata": b"\x7fELF", "bundle/lib/libfogata_ffi.so": marker}),
            "Fogata-windows-x64.zip": self.gh.add(
                "fogata-windows-x64", "Fogata-windows-x64.zip",
                {"fogata.exe": b"MZ", "fogata_ffi.dll": marker}),
            "Fogata-macos-unsigned.zip": self.gh.add(
                "fogata-macos-unsigned", "Fogata-macos-unsigned.zip",
                {"Fogata.app/Contents/Info.plist": b"<plist/>",
                 "Fogata.app/Contents/Library/Helpers/Fogata Updater.app/Contents/MacOS/"
                 "Fogata Updater": helper_binary(TEAM)}),
        }
        self.gh.sums(self.digests)

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def fetch(self, only=("linux", "windows", "macos"), team=None):
        return ci_builds.fetch(7, self.out, COMMIT, list(only), team, gh=self.gh)

    def test_verified_builds_are_laid_out_for_release_py(self):
        laid = self.fetch(team=TEAM)
        self.assertEqual(laid["linux"], self.out / "linux/bundle")
        self.assertTrue((laid["linux"] / "fogata").is_file())
        self.assertEqual(laid["windows"].read_bytes(),
                         (self.gh.artifacts / "fogata-windows-x64/Fogata-windows-x64.zip").read_bytes())
        self.assertTrue((laid["macos"] / "Contents/Info.plist").is_file())
        self.assertEqual(ci_builds.listed_sums(self.out / "SHA256SUMS"), self.digests)
        self.assertFalse((self.out / "download").exists())
        verified = [call[2] for call in self.gh.calls if call[:2] == ("attestation", "verify")]
        self.assertEqual(len(verified), 3)
        # The attested zips themselves stay, as release.py --ci-builds reads them.
        release = ci_builds.load_release()
        self.assertEqual(release.CI_ATTESTED, ci_builds.ATTESTED)
        for system, (artifact, name) in ci_builds.BUILDS.items():
            self.assertEqual(release.CI_BUILDS[system], name)
            self.assertEqual((self.out / ci_builds.ATTESTED / name).read_bytes(),
                             (self.gh.artifacts / artifact / name).read_bytes())
        self.assertEqual(set(release.read_ci_builds(self.out, ci_builds.SYSTEMS)),
                         set(ci_builds.SYSTEMS))

    def test_the_toolchain_records_come_along(self):
        for system, (artifact, _) in ci_builds.BUILDS.items():
            if system != "windows":  # a run from before the records were kept has none
                self.gh.add(artifact, ci_builds.toolchain_file(system),
                            f"image: {system}\n".encode(), attest=False)
        self.fetch()
        self.assertEqual((self.out / "toolchain-linux.txt").read_text(), "image: linux\n")
        self.assertEqual((self.out / "toolchain-macos.txt").read_text(), "image: macos\n")
        self.assertFalse((self.out / "toolchain-windows.txt").exists())
        # Outside the zips: what was attested is unchanged.
        self.assertEqual(ci_builds.listed_sums(self.out / "SHA256SUMS"), self.digests)

    def test_only_the_systems_asked_for_are_fetched(self):
        laid = self.fetch(only=["macos"])
        self.assertEqual(set(laid), {"macos"})
        [download] = [call for call in self.gh.calls if call[:2] == ("run", "download")]
        self.assertNotIn("fogata-linux-x64", download)

    def test_a_build_without_attestation_is_refused(self):
        self.gh.add("fogata-windows-x64", "Fogata-windows-x64.zip", {"fogata.exe": b"other"},
                    attest=False)
        digests = dict(self.digests)
        digests["Fogata-windows-x64.zip"] = hashlib.sha256(
            (self.gh.artifacts / "fogata-windows-x64/Fogata-windows-x64.zip").read_bytes()).hexdigest()
        self.gh.sums(digests)
        with self.assertRaises(SystemExit):
            self.fetch(only=["windows"])
        self.assertFalse((self.out / "Fogata-windows-x64.zip").exists())

    def test_an_attestation_of_another_commit_is_refused(self):
        for digest in self.gh.attested:
            self.gh.attested[digest] = OTHER
        with self.assertRaises(SystemExit):
            self.fetch(only=["linux"])

    def test_a_build_whose_sha256_the_run_did_not_list_is_refused(self):
        digests = dict(self.digests)
        digests["Fogata-linux-x64-bundle.zip"] = "0" * 64
        self.gh.sums(digests)
        with self.assertRaises(SystemExit) as stop:
            self.fetch(only=["linux"])
        self.assertIn("the run listed", str(stop.exception))

    def test_a_run_of_another_commit_or_that_failed_is_refused(self):
        for change in [{"headSha": OTHER}, {"conclusion": "failure"},
                       {"status": "in_progress", "conclusion": ""},
                       {"workflowName": "CI"}]:
            with self.subTest(change=change):
                self.gh.view.update(change)
                with self.assertRaises(SystemExit):
                    self.fetch()
                self.gh.view = Gh(self.tmp).view
                self.assertFalse(any(call[:2] == ("run", "download") for call in self.gh.calls))

    def test_a_macos_updater_of_another_team_is_refused(self):
        with self.assertRaises(SystemExit) as stop:
            self.fetch(only=["macos"], team="ZZZZZ99999")
        self.assertIn(f"built for team {TEAM}", str(stop.exception))

    def test_an_existing_folder_is_not_written(self):
        self.out.mkdir()
        with self.assertRaises(SystemExit):
            self.fetch()
        self.assertEqual(self.gh.calls, [])


class Marker(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="fogata-ci-marker-test-"))
        (self.tmp / "lib").mkdir()
        (self.tmp / "fogata").write_bytes(b"\x7fELF no marker")

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def library(self, marker):
        (self.tmp / "lib/libfogata_ffi.so").write_bytes(
            b"\0\0" + f"fogata-commit-marker:{marker};".encode() + b"\0")

    def test_the_commit_with_no_changes_passes(self):
        self.library(COMMIT)
        ci_builds.check_marker(self.tmp, COMMIT)

    def test_a_dirty_build_another_commit_or_none_fails(self):
        for marker in [f"{COMMIT}+dirty", OTHER, "unknown", None]:
            with self.subTest(marker=marker):
                if marker is None:
                    (self.tmp / "lib/libfogata_ffi.so").unlink()
                else:
                    self.library(marker)
                with self.assertRaises(SystemExit):
                    ci_builds.check_marker(self.tmp, COMMIT)

    def test_two_markers_fail(self):
        self.library(COMMIT)
        (self.tmp / "lib/old.so").write_bytes(f"fogata-commit-marker:{OTHER};".encode())
        with self.assertRaises(SystemExit):
            ci_builds.check_marker(self.tmp, COMMIT)


class CheckBuild(unittest.TestCase):
    """check-build: release.py's checks of each built application, run where it was built.
    The files are made with test_release.py's makers of ELF, PE and Mach-O headers."""

    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="fogata-ci-check-build-test-"))
        self.release = ci_builds.load_release()

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def files(self, folder, files):
        for name, data in files.items():
            path = folder / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        return folder

    def refused(self, system, folder, says):
        with self.assertRaises(SystemExit) as stop:
            ci_builds.check_build(system, folder)
        self.assertIn(says, str(stop.exception))

    def linux(self):
        elf = maker.elf
        return self.files(self.tmp / "bundle", {
            "fogata": elf(runpath="$ORIGIN/lib"),
            "lib/libflutter_linux_gtk.so": elf(runpath=None, protected=False, stripped=True),
            "lib/libflutter_webrtc_plugin.so": elf(needed=[
                "libwebrtc.so", "libpipewire-0.3.so.0", "libpulse.so.0"]),
            "lib/libfogata_ffi.so": elf(runpath=None),
            "lib/libapp.so": elf(relro=False, bind_now=None, protected=False),
            "data/icudtl.dat": b"data",
        })

    def windows(self):
        pe, release = maker.pe, self.release
        full = (release.PE_HIGH_ENTROPY_VA | release.PE_DYNAMIC_BASE | release.PE_NX_COMPAT
                | release.PE_GUARD_CF)
        ehcont = release.PE_GUARD_EH_CONTINUATION
        return self.files(self.tmp / "Release", {
            "fogata.exe": pe(full, cet=False, guard=ehcont),
            "fogata_ffi.dll": pe(full),
            "flutter_windows.dll": pe(full & ~release.PE_GUARD_CF),
            "tray_manager_plugin.dll": pe(full, guard=ehcont),
            "data/app.so": b"data",
        })

    def macos(self, build=None, archs=("arm64", "x86_64")):
        import plistlib
        build = self.release.this_build()[0] if build is None else build
        program = maker.macho(*archs) + bytes(100)
        return self.files(self.tmp / "Fogata.app", {
            "Contents/Info.plist": plistlib.dumps({
                "CFBundleIdentifier": "dev.holypanda.fogata", "CFBundleVersion": str(build),
                "CFBundleIconFile": "Fogata"}),
            "Contents/Resources/Fogata.icns": b"icns" + bytes(64),
            "Contents/MacOS/Fogata": program,
            "Contents/Library/Helpers/Fogata Updater.app/Contents/MacOS/Fogata Updater": program,
            "Contents/Frameworks/FlutterMacOS.framework/Versions/A/FlutterMacOS": program,
            "Contents/Frameworks/App.framework/Versions/A/App": program,
        })

    def test_release_builds_that_release_py_takes_pass(self):
        ci_builds.check_build("linux", self.linux())
        ci_builds.check_build("windows", self.windows())
        ci_builds.check_build("macos", self.macos())

    def test_a_mac_application_without_its_icon_is_refused(self):
        app = self.macos()
        (app / "Contents/Resources/Fogata.icns").unlink()
        self.refused("macos", app, "has no icon")

    def test_debug_builds_are_refused_on_every_system(self):
        for system, folder, blob in [
                ("linux", self.linux(), "data/flutter_assets/kernel_blob.bin"),
                ("windows", self.windows(), "data/flutter_assets/kernel_blob.bin"),
                ("macos", self.macos(),
                 "Contents/Frameworks/App.framework/Resources/flutter_assets/kernel_blob.bin")]:
            with self.subTest(system=system):
                self.files(folder, {blob: b""})
                self.refused(system, folder, "debug build")

    def test_a_linux_bundle_without_its_hardening_or_pipewire_is_refused(self):
        bundle = self.linux()
        (bundle / "lib/libflutter_webrtc_plugin.so").write_bytes(
            maker.elf(needed=["libwebrtc.so", "libpulse.so.0"]))
        self.refused("linux", bundle, "does not link libpipewire-0.3.so.0")
        bundle = self.linux()
        # The jni plugin built against a JDK: a RUNPATH into it (beta.8's bundle).
        self.files(bundle, {"lib/libdartjni.so": maker.elf(runpath="/usr/lib/jvm/lib/server")})
        self.refused("linux", bundle, "outside the application")

    def test_a_windows_library_without_its_mitigations_is_refused(self):
        release = self.release
        folder = self.windows()
        # As beta.8's dartjni.dll: built by the jni plugin without the runner's flags.
        self.files(folder, {"dartjni.dll": maker.pe(
            release.PE_HIGH_ENTROPY_VA | release.PE_DYNAMIC_BASE | release.PE_NX_COMPAT)})
        self.refused("windows", folder, "dartjni.dll was built without")

    def test_a_mac_app_not_universal_or_of_another_build_or_without_updater_is_refused(self):
        self.refused("macos", self.macos(archs=("arm64",)), "not for both arm64 and x86_64")
        shutil.rmtree(self.tmp / "Fogata.app")
        build = self.release.this_build()[0]
        self.refused("macos", self.macos(build=build + 1), f"not {build}")
        shutil.rmtree(self.tmp / "Fogata.app/Contents/Library")
        self.refused("macos", self.tmp / "Fogata.app", "no updater")
        self.refused("macos", self.tmp / "nothing", "no ")

    def test_the_command_line_says_so_by_its_exit_status(self):
        script = HERE / "ci_builds.py"
        run = [sys.executable, "-I", "-B", str(script), "check-build"]
        passed = subprocess.run(run + ["windows", str(self.windows())], capture_output=True,
                                text=True)
        self.assertEqual(passed.returncode, 0, passed.stderr)
        self.assertIn("release.py would take", passed.stdout)
        refused = subprocess.run(run + ["linux", str(self.windows())], capture_output=True,
                                 text=True)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("no lib/libflutter_webrtc_plugin.so", refused.stderr)
        other = subprocess.run(run + ["android", str(self.tmp)], capture_output=True, text=True)
        self.assertEqual(other.returncode, 2)


if __name__ == "__main__":
    unittest.main()
