#!/usr/bin/env python3
"""Tests of how the library is built and identified, on small stand-ins for the workspace:

- the commit marker of crates/fogata-update/build.rs says `+dirty` after a change to a native
  source (the Linux, Windows or macOS runner, the third-party folder) and does not make cargo
  rebuild the crate when only generated files change;
- app/rust_builder/linux/CMakeLists.txt bundles the library cargo built, also when
  CARGO_TARGET_DIR puts it somewhere other than target/, with an older library at the
  default path.

    python3 -I -B tools/test_native_build.py
"""

import os
import pathlib
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True  # no __pycache__ in the repository
ROOT = pathlib.Path(__file__).resolve().parent.parent


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def writable_and_again(remove, path, _):
    """Removes `path` again once it may be written: git keeps its objects read-only, and on
    Windows a read-only file cannot be removed."""
    os.chmod(path, stat.S_IWRITE | stat.S_IREAD)
    remove(path)


def remove_tree(path):
    """shutil.rmtree, read-only files included (`onexc` from Python 3.12 on, which
    deprecates `onerror`)."""
    if sys.version_info >= (3, 12):
        shutil.rmtree(path, onexc=writable_and_again)
    else:
        shutil.rmtree(path, onerror=writable_and_again)


def run(cwd, *args, env=None):
    merged = {**os.environ, **(env or {})}
    merged = {name: value for name, value in merged.items() if value is not None}
    result = subprocess.run([str(a) for a in args], cwd=cwd, capture_output=True, text=True,
                            env=merged)
    if result.returncode != 0:
        raise AssertionError(f"{args} failed:\n{result.stdout}\n{result.stderr}")
    return result


class CommitMarker(unittest.TestCase):
    """A workspace of one crate that has fogata-update's build.rs and prints what it makes."""

    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="fogata-marker-test-"))
        self.repo = self.tmp / "repo"
        self.target = self.tmp / "target"
        repo = self.repo
        write(repo / "Cargo.toml", '[workspace]\nmembers = ["crates/marker"]\nresolver = "2"\n')
        write(repo / "crates/marker/Cargo.toml",
              '[package]\nname = "marker"\nversion = "0.0.0"\nedition = "2021"\nbuild = "build.rs"\n')
        shutil.copy(ROOT / "crates/fogata-update/build.rs", repo / "crates/marker/build.rs")
        write(repo / "crates/marker/src/main.rs",
              'fn main() { println!("{}", env!("FOGATA_COMMIT")); }\n')
        shutil.copy(ROOT / "rust-toolchain.toml", repo / "rust-toolchain.toml")
        for name in ("app/lib/main.dart", "app/pubspec.yaml", "app/pubspec.lock",
                     "app/linux/CMakeLists.txt", "app/linux/runner/my_application.cc",
                     "app/linux/flutter/CMakeLists.txt", "app/windows/runner/main.cpp",
                     "app/macos/Runner/AppDelegate.swift", "app/third_party/pkg/src/x.cc",
                     "vendor/x/lib.rs", ".cargo/config.toml"):
            write(repo / name, "# original\n")
        write(repo / ".gitignore",
              "/target\napp/linux/flutter/ephemeral/\napp/third_party/pkg/downloads/\n")
        # What a Flutter build writes into the tree on every run.
        write(repo / "app/linux/flutter/ephemeral/generated_config.cmake", "one\n")
        write(repo / "app/third_party/pkg/downloads/libwebrtc.zip", "one\n")
        run(repo, "cargo", "generate-lockfile", "--offline")
        self.git("init", "-q", "-b", "main")
        self.git("-c", "user.name=Bea", "-c", "user.email=bea@example.com", "add", "-A")
        self.git("-c", "user.name=Bea", "-c", "user.email=bea@example.com", "commit", "-q",
                 "-m", "first")
        self.sha = self.git("rev-parse", "HEAD").stdout.strip()
        self.clock = 4_000_000_000  # modification times that only grow, whatever the disk's grain

    def tearDown(self):
        remove_tree(self.tmp)

    def git(self, *args):
        return run(self.repo, "git", *args)

    def touch(self, name, text):
        """Writes `text` (bytes as they are: a file put back as it was, whatever its line
        endings) to `name`, a moment after anything written before."""
        path = self.repo / name
        if isinstance(text, bytes):
            path.write_bytes(text)
        else:
            write(path, text)
        self.clock += 10
        os.utime(path, (self.clock, self.clock))

    def build(self):
        """Builds in release mode; returns (what the program prints, whether cargo compiled it)."""
        result = run(self.repo, "cargo", "build", "--release", "--offline",
                     env={"CARGO_TARGET_DIR": str(self.target)})
        marker = run(self.repo, self.target / "release" / ("marker.exe" if os.name == "nt" else "marker")).stdout.strip()
        return marker, "Compiling marker" in result.stderr

    def test_a_clean_checkout_says_its_commit(self):
        self.assertEqual(self.build(), (self.sha, True))
        self.assertEqual(self.build(), (self.sha, False))

    def test_a_change_to_a_native_source_says_dirty(self):
        self.build()
        for name in ("app/linux/runner/my_application.cc", "app/linux/CMakeLists.txt",
                     "app/linux/flutter/CMakeLists.txt", "app/windows/runner/main.cpp",
                     "app/macos/Runner/AppDelegate.swift", "app/third_party/pkg/src/x.cc",
                     "rust-toolchain.toml", "vendor/x/lib.rs", ".cargo/config.toml",
                     "app/lib/main.dart"):
            with self.subTest(name=name):
                self.touch(name, (self.repo / name).read_text() + "# changed\n")
                try:
                    self.assertEqual(self.build(), (f"{self.sha}+dirty", True))
                finally:
                    self.git("checkout", "--", name)
                    self.touch(name, (self.repo / name).read_bytes())
                self.assertEqual(self.build(), (self.sha, True))

    def test_a_new_native_file_says_dirty(self):
        self.build()
        for name in ("app/linux/runner/new.cc", "app/windows/runner/new.cpp",
                     "app/macos/Runner/New.swift", "app/third_party/pkg/src/new.cc"):
            with self.subTest(name=name):
                self.touch(name, "# new\n")
                try:
                    self.assertEqual(self.build(), (f"{self.sha}+dirty", True))
                finally:
                    (self.repo / name).unlink()
                self.assertEqual(self.build(), (self.sha, True))

    def test_files_a_flutter_build_writes_do_not_make_cargo_compile_again(self):
        self.build()
        self.touch("app/linux/flutter/ephemeral/generated_config.cmake", "two\n")
        self.touch("app/third_party/pkg/downloads/libwebrtc.zip", "two\n")
        self.touch("app/third_party/pkg/downloads/new.zip", "two\n")
        self.assertEqual(self.build(), (self.sha, False))


@unittest.skipUnless(sys.platform.startswith("linux") and shutil.which("cmake"),
                     "the Linux CMake file needs Linux and cmake")
class LinuxCMake(unittest.TestCase):
    """app/rust_builder/linux/CMakeLists.txt on a workspace whose fogata-ffi is a library that
    says which version of itself it is."""

    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="fogata-cmake-test-"))
        self.work = self.tmp / "work"
        write(self.work / "Cargo.toml", '[workspace]\nmembers = ["crates/fogata-ffi"]\nresolver = "2"\n')
        write(self.work / "crates/fogata-ffi/Cargo.toml",
              '[package]\nname = "fogata-ffi"\nversion = "0.0.0"\nedition = "2021"\n'
              '[lib]\ncrate-type = ["cdylib"]\n')
        (self.work / "app/rust_builder/linux").mkdir(parents=True)
        shutil.copy(ROOT / "app/rust_builder/linux/CMakeLists.txt",
                    self.work / "app/rust_builder/linux/CMakeLists.txt")
        shutil.copy(ROOT / "app/rust_builder/cargo_build.cmake",
                    self.work / "app/rust_builder/cargo_build.cmake")
        self.version(1)
        run(self.work, "cargo", "generate-lockfile", "--offline")
        self.build_dir = self.tmp / "cmake"

    def tearDown(self):
        remove_tree(self.tmp)

    def version(self, number):
        write(self.work / "crates/fogata-ffi/src/lib.rs",
              f'#[no_mangle]\npub extern "C" fn fogata_version() -> u32 {{ {number} }}\n')

    def bundled_version(self):
        # In a process of its own: a library loaded once stays loaded under its path.
        code = "import ctypes, sys; print(ctypes.CDLL(sys.argv[1]).fogata_version())"
        out = run(self.tmp, sys.executable, "-I", "-c", code, self.build_dir / "libfogata_ffi.so")
        return int(out.stdout)

    def bundle(self, env=None):
        env = {"CARGO_TARGET_DIR": None, **(env or {})}
        run(self.tmp, "cmake", "-S", self.work / "app/rust_builder/linux", "-B", self.build_dir,
            "-DCMAKE_BUILD_TYPE=Release", env=env)
        run(self.tmp, "cmake", "--build", self.build_dir, env=env)

    def test_the_library_of_the_default_target_folder_is_bundled(self):
        self.bundle()
        self.assertEqual(self.bundled_version(), 1)
        self.version(2)
        self.bundle()
        self.assertEqual(self.bundled_version(), 2)

    def test_the_library_cargo_built_is_bundled_when_it_builds_elsewhere(self):
        # An older library at the default path, as an earlier build left it.
        self.bundle()
        self.assertEqual(self.bundled_version(), 1)
        self.version(2)
        for target in (self.tmp / "elsewhere", "relative-target"):
            with self.subTest(target=str(target)):
                self.bundle({"CARGO_TARGET_DIR": str(target)})
                self.assertEqual(self.bundled_version(), 2)
        self.assertTrue((self.work / "relative-target/release/libfogata_ffi.so").is_file())
        # And once more with nothing changed: the library is still found.
        self.bundle({"CARGO_TARGET_DIR": str(self.tmp / "elsewhere")})
        self.assertEqual(self.bundled_version(), 2)


def main():
    """unittest.main, except that on Linux, where every test here runs, a skipped one fails
    the run: a Linux computer without cmake does not pass the CMake tests in silence."""
    result = unittest.main(exit=False).result
    if sys.platform.startswith("linux") and result.skipped:
        for test, reason in result.skipped:
            print(f"skipped where it must run: {test.id()} ({reason})", file=sys.stderr)
        sys.exit(1)
    sys.exit(0 if result.wasSuccessful() else 1)


if __name__ == "__main__":
    main()
