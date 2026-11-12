#!/usr/bin/env python3
"""Tests of the checks before a push: tools/check.sh's check of the RustSec database,
tools/ci.sh's record, the pre-push hook that reads it, tools/install-hooks.sh, and
tools/toolchain.sh, the note of what the workflows' builds were made with. Each runs
the real script in a repository made for the test, with stand-ins for cargo, rustc and
flutter, so nothing is built.

    python3 -I tools/test_ci.py
"""

import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

sys.dont_write_bytecode = True  # no __pycache__ in the repository
TOOLS = pathlib.Path(__file__).resolve().parent
# What tools/ci-leg.sh prints at the end of a leg that checked the commit, as a leg command.
MAC_SAYS = 'echo "fogata-ci-leg: system=macos commit=$FOGATA_CI_SHA status=pass steps=check,test skipped=none"'
WINDOWS_SAYS = 'echo "fogata-ci-leg: system=windows commit=$FOGATA_CI_SHA status=pass steps=fmt,test skipped=none"'


def write(path, text, mode=0o644):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    path.chmod(mode)


class Sandbox(unittest.TestCase):
    """A folder with a git configuration of its own and a folder of stand-in commands."""

    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="fogata-ci-test-"))
        self.bin = self.tmp / "bin"
        self.bin.mkdir()
        self.calls = self.tmp / "calls"
        self.env = {
            "PATH": f"{self.bin}:{os.environ['PATH']}",
            "HOME": str(self.tmp / "home"),
            "GIT_CONFIG_GLOBAL": str(self.tmp / "gitconfig"),
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": "Bea", "GIT_AUTHOR_EMAIL": "bea@example.com",
            "GIT_COMMITTER_NAME": "Bea", "GIT_COMMITTER_EMAIL": "bea@example.com",
            "CALLS": str(self.calls),
            "LANG": "C.UTF-8",
        }
        (self.tmp / "gitconfig").write_text("")

    def tearDown(self):
        shutil.rmtree(self.tmp)

    def stand_in(self, name, script):
        """A command `name` on the PATH that runs `script` (bash) and notes its call."""
        write(self.bin / name,
              f'#!/usr/bin/env bash\necho "{name} $*" >> "$CALLS"\n{script}\n', 0o755)

    def run_in(self, cwd, *args, env=None, stdin=""):
        return subprocess.run([str(a) for a in args], cwd=cwd, env={**self.env, **(env or {})},
                              input=stdin, capture_output=True, text=True)

    def git(self, cwd, *args):
        result = self.run_in(cwd, "git", *args)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def repo(self, files):
        """A repository holding `files` (path: text, executable when the text starts with
        #!), committed."""
        repo = self.tmp / "repo"
        repo.mkdir()
        self.git(repo, "init", "-q", "-b", "main")
        for name, text in files.items():
            write(repo / name, text, 0o755 if text.startswith("#!") else 0o644)
        self.git(repo, "add", "-A")
        self.git(repo, "commit", "-q", "-m", "first")
        return repo


class AdvisoryDatabase(Sandbox):
    """tools/check.sh refuses a RustSec database too old to trust, before any long step."""

    def database(self, days_old):
        db = self.tmp / "cargo/advisory-dbs/advisory-db-0123456789abcdef"
        (db / "crates").mkdir(parents=True)
        (db / "crates/README").write_text("advisories\n")
        self.git(db, "init", "-q")
        self.git(db, "add", "-A")
        when = f"@{int(time.time()) - days_old * 86400} +0000"
        result = self.run_in(db, "git", "commit", "-q", "-m", "advisories",
                             env={"GIT_COMMITTER_DATE": when, "GIT_AUTHOR_DATE": when})
        self.assertEqual(result.returncode, 0, result.stderr)

    def check(self):
        # cargo stops the script at its first call: what matters is whether it got there.
        self.stand_in("cargo", "exit 7")
        # The Rust rust-toolchain.toml names, without rustup (HOME is the test's).
        self.stand_in("rustc", 'echo "rustc $(sed -n \'s/^channel = "\\(.*\\)"$/\\1/p\' '
                      'rust-toolchain.toml) (test)"')
        return self.run_in(TOOLS.parent, TOOLS / "check.sh", "--no-codegen",
                           env={"CARGO_HOME": str(self.tmp / "cargo")})

    def test_an_old_database_fails_before_anything_runs(self):
        self.database(days_old=40)
        result = self.check()
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("40 days old", result.stderr)
        self.assertIn("--no-advisories", result.stderr)
        self.assertFalse(self.calls.exists())

    def test_a_recent_database_is_named_and_used(self):
        self.database(days_old=3)
        result = self.check()
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertRegex(result.stdout, r"RustSec database of \d{4}-\d\d-\d\d \(3 days old\)")
        self.assertIn("cargo fmt", self.calls.read_text())

    def test_no_database_fails(self):
        result = self.check()
        self.assertEqual(result.returncode, 1)
        self.assertIn("no RustSec database", result.stderr)


PATCHES = '[patch.crates-io]\nturn = { path = "vendor/turn" }\n'


class FuzzPatches(Sandbox):
    """tools/check.sh makes fuzz/'s [patch.crates-io] name what the workspace's does."""

    def check(self, fuzz):
        self.stand_in("cargo", "exit 7")
        self.stand_in("rustc", 'echo "rustc $(sed -n \'s/^channel = "\\(.*\\)"$/\\1/p\' '
                      'rust-toolchain.toml) (test)"')
        repo = self.repo({
            "tools/check.sh": (TOOLS / "check.sh").read_text(),
            "rust-toolchain.toml": (TOOLS.parent / "rust-toolchain.toml").read_text(),
            "Cargo.toml": '[workspace]\n[patch.crates-io]\nturn = { path = "vendor/turn" }\n'
                          'spake2 = { path = "vendor/spake2" }\n',
            "fuzz/Cargo.toml": fuzz,
        })
        return self.run_in(repo, "tools/check.sh", "--no-advisories", "--no-codegen")

    def test_a_patch_the_fuzz_workspace_lacks_fails(self):
        result = self.check('[workspace]\n[patch.crates-io]\nturn = { path = "../vendor/turn" }\n')
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("differs", result.stderr)
        self.assertIn("spake2", result.stderr)
        self.assertNotIn("cargo", self.calls.read_text())

    def test_the_same_patches_in_another_order_pass(self):
        result = self.check('[workspace]\n[patch.crates-io]\nspake2 = { path = "../vendor/spake2" }\n'
                            'turn = { path = "../vendor/turn" }\n')
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertIn("cargo fmt", self.calls.read_text())


class UnsafeCode(Sandbox):
    """tools/check.sh pins where unsafe code may be, before clippy runs."""

    def check(self, lib, other=""):
        # cargo fmt passes; the first thing after the check of unsafe code stops the script.
        self.stand_in("cargo", 'if [[ $1 == fmt ]]; then exit 0; fi; exit 7')
        self.stand_in("rustc", 'echo "rustc $(sed -n \'s/^channel = "\\(.*\\)"$/\\1/p\' '
                      'rust-toolchain.toml) (test)"')
        files = {
            "tools/check.sh": (TOOLS / "check.sh").read_text(),
            "rust-toolchain.toml": (TOOLS.parent / "rust-toolchain.toml").read_text(),
            "Cargo.toml": PATCHES, "fuzz/Cargo.toml": PATCHES,
            "crates/fogata-x/Cargo.toml": "[package]\nname = 'x'\n",
            "crates/fogata-x/src/lib.rs": lib,
        }
        if other:
            files["crates/fogata-x/src/other.rs"] = other
        repo = self.repo(files)
        return self.run_in(repo, "tools/check.sh", "--no-advisories", "--no-codegen")

    def test_a_crate_must_forbid_unsafe_code(self):
        result = self.check("pub fn f() {}\n")
        self.assertEqual(result.returncode, 1)
        self.assertIn("crates/fogata-x/src/lib.rs does not forbid unsafe code", result.stderr)

    def test_an_expectation_is_noticed(self):
        result = self.check("#![forbid(unsafe_code)]\n#[expect(unsafe_code)]\nfn f() {}\n")
        self.assertEqual(result.returncode, 1)
        self.assertIn("unsafe_code 1 crates/fogata-x/src/lib.rs", result.stderr)

    def test_a_new_block_in_a_file_is_noticed(self):
        result = self.check("#![forbid(unsafe_code)]\npub mod other;\n",
                            other="fn g() { unsafe { } }\n")
        self.assertEqual(result.returncode, 1)
        self.assertIn("unsafe 1 crates/fogata-x/src/other.rs", result.stderr)

    def test_comments_and_the_forbid_itself_are_not_counted(self):
        result = self.check("#![forbid(unsafe_code)]\n\n// unsafe_code is forbidden, and unsafe too.\n")
        # The pins of fogata-ffi are for the real tree, so only this crate's part is under test.
        self.assertNotIn("crates/fogata-x", result.stderr)


class Record(Sandbox):
    """tools/ci.sh's record, and the pre-push hook that reads it."""

    CHECK = """#!/usr/bin/env bash
echo "check.sh $* RUSTFLAGS=${RUSTFLAGS-unset} CFLAGS=${CFLAGS-unset}" >> "$CALLS"
echo "RustSec database of 2026-10-30 (2 days old)"
"""

    # Says what it needs when INTEGRATION_EXIT is 3, as the real one does.
    INTEGRATION = """#!/usr/bin/env bash
echo "integration tests CXXFLAGS=${CXXFLAGS-unset}" >> "$CALLS"
if [[ ${INTEGRATION_EXIT:-0} == 3 ]]; then
    echo "integration tests not run: missing Xvfb" >&2
fi
exit "${INTEGRATION_EXIT:-0}"
"""

    # Says what it needs when PAIRS_EXIT is 3, as the real one does.
    PAIRS = """#!/usr/bin/env bash
echo "integration pairs CXXFLAGS=${CXXFLAGS-unset}" >> "$CALLS"
if [[ ${PAIRS_EXIT:-0} == 3 ]]; then
    echo "integration pairs not run: missing Xvfb" >&2
elif [[ ${PAIRS_EXIT:-0} != 0 ]]; then
    echo "integration pairs failed: voice" >&2
else
    echo "integration pairs passed: call direct voice move"
fi
exit "${PAIRS_EXIT:-0}"
"""

    # Says what passed, failed and was skipped, as the real one does.
    SMOKE = """#!/usr/bin/env bash
echo "smoke CXXFLAGS=${CXXFLAGS-unset}" >> "$CALLS"
case ${SMOKE_EXIT:-0} in
    0) echo "smoke: passed wayland bare x11; failed none; skipped none" ;;
    3) echo "smoke session: missing kwin_wayland" >&2
       echo "smoke: passed none; failed none; skipped wayland bare x11" ;;
    *) echo "smoke: passed wayland x11; failed bare; skipped none" ;;
esac
exit "${SMOKE_EXIT:-0}"
"""

    # Notes its call and passes unless CHECK_BUILD_EXIT says otherwise.
    CHECK_BUILD = """import os, sys
with open(os.environ["CALLS"], "a") as calls:
    calls.write("ci_builds.py " + " ".join(sys.argv[1:]) + "\\n")
sys.exit(int(os.environ.get("CHECK_BUILD_EXIT", "0")))
"""

    def setUp(self):
        super().setUp()
        self.stand_in("flutter", 'if [[ $1 == --version ]]; then echo "Flutter 9.9.9 • channel x"; fi')
        self.stand_in("dart", 'exit "${DART_EXIT:-0}"')
        self.stand_in("rustc", 'echo "rustc 9.9.9 (test)"')
        self.stand_in("cargo", 'echo "cargo 9.9.9 (test)"; [[ "$*" == *fuzz* ]] && exit "${FUZZ_EXIT:-0}"; exit 0')
        self.repo_dir = self.repo({
            "tools/ci.sh": (TOOLS / "ci.sh").read_text(),
            "tools/check.sh": self.CHECK,
            "app/third_party/flutter_webrtc/common/cpp/test/run_tests.sh":
                '#!/usr/bin/env bash\necho "native tests" >> "$CALLS"\nexit "${NATIVE_EXIT:-0}"\n',
            "tools/integration-tests.sh": self.INTEGRATION,
            "tools/integration-pairs.sh": self.PAIRS,
            "tools/beta/ci_builds.py": self.CHECK_BUILD,
            "tools/smoke-linux.sh": self.SMOKE,
            "tools/hooks/pre-push": (TOOLS / "hooks/pre-push").read_text(),
            "app/pubspec.yaml": "name: app\n",
            "fuzz/Cargo.toml": "[package]\n",
        })
        self.sha = self.git(self.repo_dir, "rev-parse", "HEAD")
        self.record = self.repo_dir / ".git/fogata-ci" / self.sha

    def ci(self, *args, env=None):
        return self.run_in(self.repo_dir, "tools/ci.sh", *args, env=env)

    def fields(self):
        return dict(line.split(": ", 1) for line in self.record.read_text().splitlines())

    def push(self, env=None):
        line = f"refs/heads/main {self.sha} refs/heads/main {'0' * 40}\n"
        return self.run_in(self.repo_dir, self.repo_dir / "tools/hooks/pre-push", "origin",
                           "url", env=env, stdin=line)

    def test_every_leg_must_run_for_a_pass(self):
        result = self.ci("--full")
        self.assertNotEqual(result.returncode, 0)
        record = self.fields()
        self.assertEqual((record["linux"], record["macos"], record["windows"]),
                         ("pass", "skipped", "skipped"))
        self.assertEqual(record["result"], "incomplete")
        # The hook blocks it, unless each part skipped is named.
        self.assertEqual(self.push().returncode, 1)
        self.assertIn('FOGATA_CI_ALLOW_SKIPPED="macos windows"', self.push().stderr)
        self.assertEqual(self.push({"FOGATA_CI_ALLOW_SKIPPED": "macos"}).returncode, 1)
        self.assertEqual(self.push({"FOGATA_CI_ALLOW_SKIPPED": "macos windows"}).returncode, 0)
        self.assertEqual(self.push({"FOGATA_CI_ALLOW_SKIPPED": "windows,macos"}).returncode, 0)

        result = self.ci("--full", env={"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.fields()["result"], "pass")
        self.assertEqual(self.push().returncode, 0)

        result = self.ci("--full", env={"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": "false"})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.fields()["result"], "fail")
        self.assertEqual(self.push({"FOGATA_CI_ALLOW_SKIPPED": "windows"}).returncode, 1)

    def test_a_leg_that_says_nothing_is_incomplete_not_a_pass(self):
        # A bare `true` exits 0 and checked nothing.
        result = self.ci("--full", env={"FOGATA_CI_MAC": "true", "FOGATA_CI_WINDOWS": WINDOWS_SAYS})
        self.assertNotEqual(result.returncode, 0)
        record = self.fields()
        self.assertEqual((record["macos"], record["windows"]), ("incomplete", "pass"))
        self.assertEqual(record["macos exit"], "0")
        self.assertEqual(record["result"], "incomplete")
        push = self.push()
        self.assertEqual(push.returncode, 1)
        self.assertIn('FOGATA_CI_ALLOW_SKIPPED="macos"', push.stderr)
        self.assertEqual(self.push({"FOGATA_CI_ALLOW_SKIPPED": "macos"}).returncode, 0)
        # A line for another commit, or another system, says nothing about this one.
        for line in ("fogata-ci-leg: system=macos commit=" + "0" * 40 + " status=pass steps=x skipped=none",
                     "fogata-ci-leg: system=windows commit=$FOGATA_CI_SHA status=pass steps=x skipped=none",
                     "fogata-ci-leg: system=macos commit=$FOGATA_CI_SHA status=fail steps=x skipped=none"):
            result = self.ci("--full", env={"FOGATA_CI_MAC": f'echo "{line}"', "FOGATA_CI_WINDOWS": WINDOWS_SAYS})
            self.assertEqual(self.fields()["macos"], "incomplete", line)
        # Steps left out make it incomplete too, and the record names them.
        left_out = MAC_SAYS.replace("skipped=none", "skipped=test")
        self.ci("--full", env={"FOGATA_CI_MAC": left_out, "FOGATA_CI_WINDOWS": WINDOWS_SAYS})
        self.assertEqual(self.fields()["macos"], "incomplete")
        # Exit status 1 with the line is still a failure.
        self.ci("--full", env={"FOGATA_CI_MAC": MAC_SAYS + "; exit 1", "FOGATA_CI_WINDOWS": WINDOWS_SAYS})
        self.assertEqual(self.fields()["macos"], "fail")

    def test_a_leg_records_its_command_exit_status_and_output_digest(self):
        import hashlib
        legs = {"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS}
        self.assertEqual(self.ci("--full", env=legs).returncode, 0)
        record = self.fields()
        output = "fogata-ci-leg: system=macos commit=%s status=pass steps=check,test skipped=none\n" % self.sha
        self.assertEqual(record["macos command"], MAC_SAYS)
        self.assertEqual(record["macos exit"], "0")
        self.assertEqual(record["macos output"], f"{hashlib.sha256(output.encode()).hexdigest()} (1 lines)")
        self.assertEqual(record["macos steps"], "check,test")
        self.assertEqual(record["legs ran"], "linux macos windows native integration pairs smoke")
        self.assertEqual(record["legs not run"], "nothing")
        # A skipped leg says so, with nothing made up.
        self.ci("--full")
        record = self.fields()
        self.assertEqual((record["macos command"], record["macos exit"], record["macos output"]),
                         ("none", "none", "none"))
        self.assertIn("macos (skipped)", record["legs not run"])
        # A hand-written record that says macos passed without its output is refused.
        self.assertEqual(self.ci("--full", env=legs).returncode, 0)
        self.record.write_text(self.record.read_text().replace(
            f"macos output: {self.fields()['macos output']}", "macos output: none"))
        self.assertEqual(self.push().returncode, 1)

    def test_the_pair_tests_run_only_with_full_and_the_record_says_which_legs_ran(self):
        legs = {"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS}
        # Every leg ran and passed, but the pair tests did not run: incomplete, not a pass.
        self.assertNotEqual(self.ci(env=legs).returncode, 0)
        self.assertNotIn("integration pairs", self.calls.read_text())
        record = self.fields()
        self.assertEqual((record["full"], record["pairs"], record["result"]),
                         ("no", "not run (--full not given)", "incomplete"))
        self.assertIn("pairs (not run (--full not given))", record["legs not run"])
        push = self.push()
        self.assertEqual(push.returncode, 1)
        self.assertIn('FOGATA_CI_ALLOW_SKIPPED="pairs"', push.stderr)
        self.assertEqual(self.push({"FOGATA_CI_ALLOW_SKIPPED": "pairs"}).returncode, 0)
        self.assertEqual(self.push({"FOGATA_CI_ALLOW_SKIPPED": "macos windows"}).returncode, 1)

        result = self.ci("--full", env=legs)
        self.assertEqual(result.returncode, 0, result.stderr)
        record = self.fields()
        self.assertEqual((record["full"], record["pairs"], record["result"]),
                         ("yes", "pass (call direct voice move)", "pass"))
        self.assertEqual(record["legs ran"], "linux macos windows native integration pairs smoke")
        self.assertEqual(record["legs not run"], "nothing")
        self.assertEqual(record["smoke"], "pass (wayland bare x11; failed none; skipped none)")
        calls = self.calls.read_text()
        self.assertIn("integration pairs", calls)
        self.assertLess(calls.index("integration tests"), calls.index("integration pairs"))
        self.assertLess(calls.index("integration pairs"), calls.index("smoke"))

        # A desktop that fails, or a copy that crashes, fails the run.
        result = self.ci("--full", env={**legs, "SMOKE_EXIT": "1"})
        self.assertNotEqual(result.returncode, 0)
        record = self.fields()
        self.assertEqual((record["linux"], record["smoke"], record["result"]),
                         ("fail", "fail (wayland x11; failed bare; skipped none)", "fail"))

        # No desktop could be built here: recorded, and the result still passes.
        result = self.ci("--full", env={**legs, "SMOKE_EXIT": "3"})
        self.assertEqual(result.returncode, 0, result.stderr)
        record = self.fields()
        self.assertEqual((record["smoke"], record["result"]),
                         ("skipped (none; failed none; skipped wayland bare x11; "
                          "smoke session: missing kwin_wayland)", "pass"))

        result = self.ci("--full", env={**legs, "PAIRS_EXIT": "1"})
        self.assertNotEqual(result.returncode, 0)
        record = self.fields()
        self.assertEqual((record["linux"], record["pairs"], record["result"]),
                         ("fail", "fail (voice)", "fail"))

        result = self.ci("--full", env={**legs, "PAIRS_EXIT": "3"})
        self.assertNotEqual(result.returncode, 0)
        record = self.fields()
        self.assertEqual((record["pairs"], record["result"]), ("skipped (missing Xvfb)", "incomplete"))
        self.assertIn('FOGATA_CI_ALLOW_SKIPPED="pairs"', self.push().stderr)

    def test_a_check_skipped_is_recorded_and_blocks(self):
        legs = {"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS}
        result = self.ci("--full", "--no-codegen", "--no-advisories", env=legs)
        self.assertNotEqual(result.returncode, 0)
        record = self.fields()
        self.assertEqual(record["codegen"], "skipped")
        self.assertEqual(record["advisories"], "skipped")
        self.assertEqual(record["notices"], "checked")
        self.assertEqual(record["result"], "incomplete")
        self.assertIn("check.sh --no-codegen --no-advisories", self.calls.read_text())
        self.assertEqual(self.push({"FOGATA_CI_ALLOW_SKIPPED": "codegen"}).returncode, 1)
        self.assertEqual(
            self.push({"FOGATA_CI_ALLOW_SKIPPED": "codegen advisories"}).returncode, 0)

    def test_the_record_names_the_tools_and_clears_what_changes_the_build(self):
        legs = {"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS,
                "RUSTFLAGS": "-C opt-level=0", "CFLAGS": "-O0"}
        result = self.ci("--full", env=legs)
        self.assertEqual(result.returncode, 0, result.stderr)
        record = self.fields()
        self.assertEqual(record["advisories"], "checked (RustSec database of 2026-10-30)")
        self.assertEqual(record["rustc"], "rustc 9.9.9 (test)")
        self.assertEqual(record["cargo"], "cargo 9.9.9 (test)")
        self.assertEqual(record["flutter"], "Flutter 9.9.9 • channel x")
        self.assertEqual(record["cleared"], "RUSTFLAGS CFLAGS")
        script = self.git(self.repo_dir, "show", "HEAD:tools/ci.sh") + "\n"
        import hashlib
        self.assertEqual(record["ci.sh"], hashlib.sha256(script.encode()).hexdigest())
        self.assertIn("RUSTFLAGS=unset CFLAGS=unset", self.calls.read_text())

    def test_the_native_and_integration_tests_are_recorded(self):
        legs = {"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS}
        result = self.ci("--full", env=legs)
        self.assertEqual(result.returncode, 0, result.stderr)
        record = self.fields()
        self.assertEqual((record["native"], record["integration"]), ("pass", "pass"))
        calls = self.calls.read_text()
        self.assertIn("native tests", calls)
        # Built like the application, after flutter test.
        self.assertIn("integration tests", calls)
        self.assertLess(calls.index("flutter test"), calls.index("integration tests"))
        # The Dart code must be formatted, checked before flutter analyze.
        self.assertLess(calls.index("dart format --output=none --set-exit-if-changed"),
                        calls.index("flutter analyze"))
        result = self.ci("--full", env={**legs, "DART_EXIT": "1"})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.fields()["linux"], self.fields()["result"]), ("fail", "fail"))

        # A failing integration test fails the run.
        result = self.ci("--full", env={**legs, "INTEGRATION_EXIT": "1"})
        self.assertNotEqual(result.returncode, 0)
        record = self.fields()
        self.assertEqual((record["linux"], record["integration"]), ("fail", "fail"))
        self.assertEqual(record["result"], "fail")
        self.assertEqual(self.push({"FOGATA_CI_ALLOW_SKIPPED": "integration"}).returncode, 1)

        # A failing native test fails it too, and what comes after does not run.
        result = self.ci("--full", env={**legs, "NATIVE_EXIT": "1"})
        self.assertNotEqual(result.returncode, 0)
        record = self.fields()
        self.assertEqual((record["native"], record["integration"]), ("fail", "not run"))
        self.assertEqual(record["result"], "fail")

    def test_the_bundle_must_pass_the_checks_of_release_py(self):
        legs = {"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS}
        self.assertEqual(self.ci("--full", env=legs).returncode, 0)
        calls = self.calls.read_text()
        self.assertIn("ci_builds.py check-build linux app/build/linux/x64/release/bundle", calls)
        # Right after the build it checks, before the tests.
        self.assertLess(calls.index("flutter build linux --release"), calls.index("check-build"))
        self.assertLess(calls.index("check-build"), calls.index("native tests"))
        result = self.ci("--full", env={**legs, "CHECK_BUILD_EXIT": "1"})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("release checks of the bundle FAILED", result.stderr)
        record = self.fields()
        self.assertEqual((record["linux"], record["native"], record["result"]),
                         ("fail", "not run", "fail"))

    def test_the_fuzz_targets_must_build(self):
        legs = {"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS}
        self.assertEqual(self.ci("--full", env=legs).returncode, 0)
        calls = self.calls.read_text()
        self.assertIn("cargo +nightly fuzz build --fuzz-dir .", calls)
        self.assertLess(calls.index("check.sh"), calls.index("fuzz build"))
        self.assertLess(calls.index("fuzz build"), calls.index("flutter test"))
        # Targets that do not compile fail the run, and nothing after them runs.
        result = self.ci("--full", env={**legs, "FUZZ_EXIT": "1"})
        self.assertNotEqual(result.returncode, 0)
        record = self.fields()
        self.assertEqual((record["linux"], record["native"], record["integration"], record["result"]),
                         ("fail", "pass", "not run", "fail"))
        self.assertEqual(self.push({"FOGATA_CI_ALLOW_SKIPPED": "linux"}).returncode, 1)

    def test_kept_builds_past_the_limit_are_emptied(self):
        kept = self.repo_dir / ".git/fogata-ci"
        for folder in ("target", "fuzz-target"):
            (kept / folder).mkdir(parents=True, exist_ok=True)
            (kept / folder / "old.rlib").write_bytes(b"x" * 4096)
        legs = {"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS}
        # Under the limit they stay.
        self.assertEqual(self.ci("--full", env=legs).returncode, 0)
        self.assertTrue((kept / "target/old.rlib").exists())
        self.assertTrue((kept / "fuzz-target/old.rlib").exists())
        # Past it (0 GB here) they go, the run says so, and it still passes.
        result = self.ci("--full", env={**legs, "FOGATA_CI_CACHE_GB": "0"})
        self.assertEqual(result.returncode, 0)
        self.assertFalse((kept / "target/old.rlib").exists())
        self.assertFalse((kept / "fuzz-target/old.rlib").exists())
        self.assertIn("target held 0 GB of old builds, past 0 GB: emptied", result.stderr)
        # A limit that is not a number is refused.
        self.assertEqual(self.ci("--full", env={**legs, "FOGATA_CI_CACHE_GB": "lots"}).returncode, 2)

    def test_integration_tests_that_cannot_run_are_skipped_and_block(self):
        legs = {"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS, "INTEGRATION_EXIT": "3"}
        result = self.ci("--full", env=legs)
        self.assertNotEqual(result.returncode, 0)
        record = self.fields()
        self.assertEqual(record["linux"], "pass")
        self.assertEqual(record["integration"], "skipped (missing Xvfb)")
        self.assertEqual(record["result"], "incomplete")
        push = self.push()
        self.assertEqual(push.returncode, 1)
        self.assertIn('FOGATA_CI_ALLOW_SKIPPED="integration"', push.stderr)
        self.assertEqual(self.push({"FOGATA_CI_ALLOW_SKIPPED": "integration"}).returncode, 0)

    def test_a_record_of_another_ci_script_is_refused(self):
        legs = {"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS}
        self.assertEqual(self.ci("--full", env=legs).returncode, 0)
        text = self.record.read_text()
        old = self.fields()["ci.sh"]
        self.record.write_text(text.replace(old, "0" * 64))
        self.assertEqual(self.push().returncode, 1)
        self.record.unlink()
        self.assertEqual(self.push().returncode, 1)

    def test_each_half_runs_its_own_steps_and_is_never_a_pass(self):
        legs = {"FOGATA_CI_MAC": MAC_SAYS, "FOGATA_CI_WINDOWS": WINDOWS_SAYS}
        app_steps = ["flutter build linux", "check-build", "native tests", "gen-l10n",
                     "flutter analyze", "flutter test", "integration tests",
                     "integration pairs", "smoke"]
        check_steps = ["check.sh", "fuzz build"]
        # What both halves run between them is what one whole run runs.
        self.assertEqual(self.ci("--full", env=legs).returncode, 0)
        whole = self.calls.read_text()
        for name in app_steps + check_steps:
            self.assertIn(name, whole)

        self.calls.unlink()
        result = self.ci("--full", "--split", "checks", env=legs)
        self.assertNotEqual(result.returncode, 0)
        calls = self.calls.read_text()
        self.assertIn("flutter pub get --offline", calls)
        for name in check_steps:
            self.assertIn(name, calls)
        for name in app_steps:
            self.assertNotIn(name, calls)
        record = self.fields()
        self.assertEqual((record["split"], record["linux"], record["result"]),
                         ("checks", "pass", "incomplete"))
        self.assertEqual(record["advisories"], "checked (RustSec database of 2026-10-30)")
        self.assertEqual((record["codegen"], record["notices"]), ("checked", "checked"))
        for part in ("native", "integration", "pairs", "smoke"):
            self.assertEqual(record[part], "elsewhere (--split checks)", part)
        # The hook never takes half a run, whatever is named.
        push = self.push({"FOGATA_CI_ALLOW_SKIPPED": "linux macos windows native integration "
                                                     "pairs smoke"})
        self.assertEqual(push.returncode, 1)
        self.assertIn("without --split", push.stderr)

        self.calls.unlink()
        result = self.ci("--full", "--split", "app", env=legs)
        self.assertNotEqual(result.returncode, 0)
        calls = self.calls.read_text()
        for name in app_steps:
            self.assertIn(name, calls)
        for name in check_steps:
            self.assertNotIn(name, calls)
        record = self.fields()
        self.assertEqual((record["split"], record["linux"], record["native"], record["integration"],
                          record["result"]), ("app", "pass", "pass", "pass", "incomplete"))
        self.assertEqual(record["pairs"], "pass (call direct voice move)")
        for check in ("advisories", "codegen", "notices"):
            self.assertEqual(record[check], "elsewhere (--split app)", check)
        self.assertEqual(self.push().returncode, 1)

        # A failure in either half fails it.
        self.assertNotEqual(self.ci("--full", "--split", "checks", env={**legs, "FUZZ_EXIT": "1"}).returncode, 0)
        self.assertEqual((self.fields()["linux"], self.fields()["result"]), ("fail", "fail"))
        self.assertNotEqual(self.ci("--full", "--split", "app", env={**legs, "SMOKE_EXIT": "1"}).returncode, 0)
        self.assertEqual((self.fields()["linux"], self.fields()["result"]), ("fail", "fail"))
        # A half that is neither is refused.
        self.assertEqual(self.ci("--split", "other").returncode, 2)
        self.assertEqual(self.ci("--split").returncode, 2)
        # A whole run says so.
        self.assertEqual(self.ci("--full", env=legs).returncode, 0)
        self.assertEqual(self.fields()["split"], "none")


class Leg(Sandbox):
    """tools/ci-leg.sh, what a leg of another system runs there."""

    # Says what passed, failed and was skipped, as the real one does.
    SMOKE = """#!/usr/bin/env bash
echo "smoke CXXFLAGS=${CXXFLAGS-unset}" >> "$CALLS"
case ${SMOKE_EXIT:-0} in
    0) echo "smoke: passed wayland bare x11; failed none; skipped none" ;;
    3) echo "smoke session: missing kwin_wayland" >&2
       echo "smoke: passed none; failed none; skipped wayland bare x11" ;;
    *) echo "smoke: passed wayland x11; failed bare; skipped none" ;;
esac
exit "${SMOKE_EXIT:-0}"
"""

    def setUp(self):
        super().setUp()
        self.stand_in("flutter", "exit ${FLUTTER_EXIT:-0}")
        self.stand_in("dart", 'exit "${DART_EXIT:-0}"')
        self.repo_dir = self.repo({
            "tools/ci-leg.sh": (TOOLS / "ci-leg.sh").read_text(),
            "tools/check.sh": '#!/usr/bin/env bash\necho "check.sh $*" >> "$CALLS"\nexit "${CHECK_EXIT:-0}"\n',
            "app/pubspec.yaml": "name: app\n",
            "deny.toml": "deny\n", "THIRD-PARTY-NOTICES": "notices\n", "docs/ci.md": "ci\n",
            "rust-toolchain.toml": "[toolchain]\n",
        })
        self.sha = self.git(self.repo_dir, "rev-parse", "HEAD")

    def leg(self, env=None, sha=None):
        return self.run_in(self.repo_dir, "tools/ci-leg.sh", "macos",
                           env={"FOGATA_CI_SHA": sha or self.sha, **(env or {})})

    def test_it_says_what_it_checked_only_when_every_step_passed(self):
        result = self.leg()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout.splitlines()[-1],
            f"fogata-ci-leg: system=macos commit={self.sha} status=pass steps=check,analyze,test skipped=none")
        self.assertIn("check.sh --no-advisories --no-codegen", self.calls.read_text())
        self.assertIn("dart format --output=none --set-exit-if-changed lib test", self.calls.read_text())
        # Dart code not formatted fails the leg like a failed check.
        for env in ({"CHECK_EXIT": "1"}, {"FLUTTER_EXIT": "1"}, {"DART_EXIT": "1"}):
            result = self.leg(env)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn("fogata-ci-leg:", result.stdout)

    def test_steps_left_out_are_named(self):
        result = self.leg({"FOGATA_CI_LEG_SKIP": "test,analyze"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.splitlines()[-1].endswith("steps=check skipped=analyze,test"),
                        result.stdout)

    def test_a_clone_at_another_commit_or_with_new_source_files_is_refused(self):
        result = self.leg(sha="0" * 40)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not at", result.stderr)
        (self.repo_dir / "app/lib").mkdir()
        (self.repo_dir / "app/lib/new.dart").write_text("new\n")
        result = self.leg()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("the clone has changes", result.stderr)
        self.assertNotIn("fogata-ci-leg:", result.stdout)

    def test_a_change_to_any_tracked_file_is_refused(self):
        for name in ("deny.toml", "THIRD-PARTY-NOTICES", "docs/ci.md", "rust-toolchain.toml",
                     "tools/check.sh"):
            with self.subTest(name=name):
                path = self.repo_dir / name
                path.write_text(path.read_text() + "changed\n")
                result = self.leg()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(f" M {name}", result.stderr)
                self.assertNotIn("fogata-ci-leg:", result.stdout)
                self.git(self.repo_dir, "checkout", "--", name)
        self.assertEqual(self.leg().returncode, 0)

    def test_a_git_status_that_fails_is_a_failure_not_a_clean_clone(self):
        real = shutil.which("git")
        self.stand_in("git", f'if [[ $1 == status ]]; then exit 128; fi; exec {real} "$@"')
        result = self.leg()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("git status failed", result.stderr)
        self.assertNotIn("fogata-ci-leg:", result.stdout)


class Toolchain(Sandbox):
    """tools/toolchain.sh, the note of what a build was made with."""

    def test_it_names_each_tool_and_what_cannot_be_asked(self):
        self.stand_in("flutter", 'echo \'{\n  "frameworkVersion": "9.1.2",\n'
                      '  "frameworkRevision": "f00d",\n  "engineRevision": "e11e",\n'
                      '  "dartSdkVersion": "3.9.9"\n}\'')
        self.stand_in("rustc", 'echo "rustc 9.9.9 (test)"')
        self.stand_in("cmake", 'echo "cmake version 9.0.0"; echo "more"')
        self.stand_in("clang", 'echo "clang version 99"')
        self.stand_in("dpkg-query", '[[ ${@: -1} == libpulse-dev ]] && exit 1; echo "1.${@: -1}"')
        result = self.run_in(self.tmp, TOOLS / "toolchain.sh", "linux",
                             env={"ImageOS": "ubuntu24", "ImageVersion": "20261101.360"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), [
            "image: ubuntu24 20261101.360",
            "flutter: 9.1.2 (framework f00d, engine e11e, dart 3.9.9)",
            "rustc: rustc 9.9.9 (test)",
            "cmake: cmake version 9.0.0",
            "clang: clang version 99",
            "libgtk-3-dev: 1.libgtk-3-dev",
            "libpipewire-0.3-dev: 1.libpipewire-0.3-dev",
            "libpulse-dev: missing",
        ])

    def test_the_windows_toolset_and_sdk_are_read_from_the_build(self):
        build = self.tmp / "build"
        write(build / "runner/fogata.vcxproj",
              "<Project>\r\n  <PlatformToolset>v145</PlatformToolset>\r\n"
              "  <WindowsTargetPlatformVersion>10.0.26100.0</WindowsTargetPlatformVersion>\r\n")
        write(build / "CMakeFiles/4.1.2/CMakeCXXCompiler.cmake",
              'set(CMAKE_CXX_COMPILER_VERSION "19.50.35717.0")\n')
        result = self.run_in(self.tmp, TOOLS / "toolchain.sh", "windows", build)
        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()
        self.assertIn("msvc of the build: 19.50.35717.0", lines)
        self.assertIn("platform toolset of the build: v145", lines)
        self.assertIn("windows sdk of the build: 10.0.26100.0", lines)
        self.assertNotEqual(self.run_in(self.tmp, TOOLS / "toolchain.sh", "beos").returncode, 0)


class PreviousRelease(Sandbox):
    """tools/previous-release.sh and the record tools/beta/release.py reads."""

    def setUp(self):
        super().setUp()
        # Notes its call with where it ran and FOGATA_OLD_CLI; the test says what
        # TEST_SAYS says and exits with TEST_EXIT.
        self.stand_in("cargo", 'echo "cargo $* in $(basename "$PWD") old=${FOGATA_OLD_CLI-}" >> "$CALLS"\n'
                      'if [[ $1 == test ]]; then echo "${TEST_SAYS-test result: ok. 1 passed; 0 failed}"; '
                      'exit "${TEST_EXIT:-0}"; fi')
        self.repo_dir = self.repo({
            "tools/previous-release.sh": (TOOLS / "previous-release.sh").read_text(),
            "Cargo.toml": "[workspace]\n",
        })
        self.old = self.git(self.repo_dir, "rev-parse", "HEAD")
        self.git(self.repo_dir, "tag", "v0.1.0-beta.1")
        write(self.repo_dir / "Cargo.toml", "[workspace]\n# newer\n")
        self.git(self.repo_dir, "commit", "-q", "-am", "second")
        self.sha = self.git(self.repo_dir, "rev-parse", "HEAD")
        self.record = self.repo_dir / ".git/fogata-ci" / f"{self.sha}.previous-release"

    def run_it(self, *args, env=None):
        return self.run_in(self.repo_dir, "tools/previous-release.sh", *args,
                           env={"FOGATA_PREVIOUS_TARGET": str(self.tmp / "target"),
                                **(env or {})})

    def fields(self):
        return dict(line.split(": ", 1) for line in self.record.read_text().splitlines())

    def test_a_pass_is_recorded_for_the_commit_against_the_last_tag(self):
        result = self.run_it()
        self.assertEqual(result.returncode, 0, result.stderr)
        record = self.fields()
        self.assertEqual(record["commit"], self.sha)
        self.assertEqual(record["tag"], f"v0.1.0-beta.1 ({self.old})")
        self.assertEqual(record["result"], "pass")
        calls = [line for line in self.calls.read_text().splitlines() if " in " in line]
        # The old command line built in a worktree of the tag, the test run here against it.
        self.assertTrue(calls[0].startswith("cargo build --release --locked -p fogata in old"))
        self.assertEqual(calls[1], "cargo test --locked -p fogata-core --test previous_release "
                         f"-- --ignored in repo old={self.tmp / 'target'}/release/fogata")
        self.assertEqual(self.git(self.repo_dir, "worktree", "list").count("\n"), 0)

    def test_a_failure_or_no_test_run_records_no_pass(self):
        for env in ({"TEST_EXIT": "101"}, {"TEST_SAYS": "test result: ok. 0 passed; 0 failed"}):
            with self.subTest(env=env):
                result = self.run_it(env=env)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(self.fields()["result"], "fail")

    def test_a_checkout_with_changes_is_refused(self):
        write(self.repo_dir / "Cargo.toml", "[workspace]\n# changed\n")
        result = self.run_it()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("has changes", result.stderr)
        self.assertFalse(self.record.exists())


class InstallHooks(Sandbox):
    """tools/install-hooks.sh installs into this clone only."""

    def setUp(self):
        super().setUp()
        self.repo_dir = self.repo({
            "tools/install-hooks.sh": (TOOLS / "install-hooks.sh").read_text(),
            "tools/hooks/pre-push": "#!/bin/sh\nexit 0\n",
        })

    def install(self):
        return self.run_in(self.repo_dir, "tools/install-hooks.sh")

    def test_a_hooks_folder_for_every_repository_is_left_alone(self):
        shared = self.tmp / "shared-hooks"
        self.git(self.repo_dir, "config", "--global", "core.hooksPath", str(shared))
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((shared / "pre-push").exists())
        self.assertTrue((self.repo_dir / ".git/hooks/pre-push").is_file())
        # Git runs this clone's hooks now.
        self.assertEqual(self.git(self.repo_dir, "config", "--local", "core.hooksPath"),
                         str((self.repo_dir / ".git/hooks").resolve()))

    def test_a_relative_hooks_folder_is_read_from_the_top(self):
        self.git(self.repo_dir, "config", "--local", "core.hooksPath", ".githooks")
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.repo_dir / ".githooks/pre-push").is_file())
        self.assertFalse((self.repo_dir / "tools/.githooks").exists())

    def test_a_hooks_folder_outside_the_clone_is_refused(self):
        self.git(self.repo_dir, "config", "--local", "core.hooksPath", str(self.tmp / "out"))
        result = self.install()
        self.assertEqual(result.returncode, 1)
        self.assertFalse((self.tmp / "out/pre-push").exists())


if __name__ == "__main__":
    unittest.main()
