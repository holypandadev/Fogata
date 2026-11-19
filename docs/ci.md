# Checks before a push

Every commit pushed is checked first on the computer that pushes it. GitHub runs the same
checks again on its own computers (`.github/workflows/ci.yml`), for every push to `main`, every
pull request and by hand: on Linux `tools/ci.sh --full`, in two halves on two computers
(`--split app` in the job `linux`, `--split checks` in the job `linux-checks`), whose macOS and
Windows legs are the workflow's other two jobs, `tools/ci-leg.sh macos` and
`tools/ci-leg.ps1` (without the smoke test, which needs virtual sound devices), all four side
by side. The run passes only when every job passes; `tools/beta/release.py` asks GitHub for
the run's conclusion, not for any one job.

None of this is enough for a release: calls with an optimised build, the network lab, a
real call on Windows and on macOS, and a data folder of the last release are checked by
hand before each one (docs/releasing-beta.md, "Checks before a release"); the push checks
passed with a fault in each of them.

What GitHub's computers cover, and what they do not:

- Linux: everything of `tools/ci.sh --full`, split in two jobs that start together and set up
  the same system packages, sound server, Flutter and Rust (only `linux-checks` installs the
  fuzz toolchain): `linux-checks` runs `tools/check.sh` (strict) and builds the fuzz targets;
  `linux` runs every other step. Each job's own record must show every step of its half run
  and passed (the record of a half says `split: app` or `split: checks`, the other half's
  parts `elsewhere`, and never `result: pass`, so the pre-push hook refuses it). Of the
  smoke test's desktops only `x11` (Xvfb) runs there; the KDE ones (`wayland`, `xwayland`,
  `bare`: the desktop portal's screen sharing) need a GPU render node and are skipped. The
  job fails unless at least one desktop ran the smoke test and passed. The KDE desktops run
  only on the computers of the people who push, through the pre-push hook.
- macOS: `tools/check.sh --no-advisories --no-codegen`, `dart format`, `flutter analyze` and
  `flutter test`;
  no integration, pair or smoke test; then the application built as a release is
  (`flutter build macos --release`, universal: the Rust library, the native plugins and the
  `objective_c` hook for both Macs, so the Intel slice is compiled at every push, not first
  at a release; `WebRTC.xcframework` downloaded and checked, see
  `app/macos/WebRTC-SDK.podspec`), with its updater helper (`tools/beta/macos-updater.sh`,
  no team pinned), checked as `tools/beta/release.py` checks it (below). The job uses the
  Xcode `XCODE_VERSION` names (the workflow's `env`, the same in `release-build.yml`; it
  stops if GitHub's image no longer has it) and the CocoaPods version `Podfile.lock` names,
  installed for the job, rather than the image's.
- Windows: `tools/ci-leg.ps1` without the smoke test (it needs virtual sound devices), and
  the application built as a release is (`flutter build windows --release`: Control Flow
  Guard and EH continuation metadata are release-only flags of `app/windows/CMakeLists.txt`),
  which compiles the native plugins and downloads and checks the Windows libwebrtc, checked
  as `tools/beta/release.py` checks it.

Each built application, the Linux bundle of `tools/ci.sh` too, goes through
`python3 -I tools/beta/ci_builds.py check-build <linux|windows|macos> <folder>`: the checks
`release.py` runs before it packs a build, with its own code (a release build, not debug or
profile; every Linux and Windows program and library with its hardening, the Linux WebRTC
plugin linking PipeWire and PulseAudio; the Mac app universal, with its icon, its updater, its
`Info.plist` naming `THIS_BUILD`). A build `release.py` would refuse fails at the push, not
at the release. Each build job also writes `tools/toolchain.sh` to the run's summary: the
runner image, Flutter's engine revision, Rust, CMake, and clang and the dev packages (Linux),
Xcode, its SDK, Swift and CocoaPods (macOS), Visual Studio, the MSVC toolset and the Windows
SDK the build used (Windows). Run as one job, the Linux steps took some 56 minutes
(`check.sh` 23, the pair tests 8, the integration tests 7, the fuzz build 5, flutter test 4);
split, each half should take about 30 with its setup, so the run ends with the Windows job
(some 50 minutes), no longer with Linux.

A newer push to a pull request cancels the run of the one before; every push to `main` keeps
its own run. The Rust builds of the dependencies are cached (`Swatinem/rust-cache`, a cache
per job), saved only by runs of pushes to `main`, so a pull request reads them and never
writes them. That is what keeps them honest: Cargo checks each crate's source against
`Cargo.lock`, not the compiled builds it finds in the target folder. The release builds
(`release-build.yml`) use no cache at all.
Every Linux job but the release build installs its Ubuntu packages with `tools/ci-apt.sh`,
which gives the fetching a time of its own and asks another mirror when one stalls: some
evenings every mirror gives a few hundred kB/s, and the 128 MB of packages did not come in
the step's 20 minutes. So each job keeps the packages' archives (`.deb`) it fetched in a
cache of its own (`CI_APT_ARCHIVES`, `/tmp/fogata-apt`), restored before the install from
the newest of its branch or else of `main`, and saved, under a key named by the archives it
holds, whenever that is not the one restored, even when the install failed: a run fetches
only what Ubuntu updated since. The package lists are always fetched; apt takes an archive
it finds in its cache by its size alone, so `ci-apt.sh` first keeps only the plain files
whose SHA-256 the lists just fetched, which Ubuntu signs, give for that file's own name, and
deletes everything else, the versions the lists no longer give among them (a release's own
version stays in the lists beside its update, so at most one more version of a package may
be kept). Unlike the Rust builds, a branch saves these too: a branch's cache is read by that
branch's runs and by pull requests into it. The check guards against stale or damaged
archives, and only runs on the default branch can write what the default branch's runs
restore.
`.github/workflows/weekly.yml` runs once a week (and by hand) what no push waits for: see
"Every week" below. `.github/workflows/release-build.yml` builds a pre-release's three
applications, by hand, and has GitHub attest them; it signs nothing
(docs/releasing-beta.md, "A release", step 2). `.github/workflows/update-install.yml`, by
hand after a release is published, installs an earlier release on Windows and as a Flatpak
and sees it update itself to the newest one from the releases page (the same document,
step 7). `.github/workflows/netlab.yml`, by hand, runs the network lab on six of GitHub's
computers at once, about half an hour (the same document, "Checks before a release", 3),
and `.github/workflows/call-checks.yml` the Linux calls of the same list (2): see below.
`.github/workflows/flatpak-drop.yml`, by hand, drags files onto the Flatpak on KDE Plasma
Wayland: see "Dropping files on the Flatpak" below.

## The calls before a release

`.github/workflows/call-checks.yml`, started by hand on the commit to be released (`gh
workflow run call-checks.yml --ref <branch or tag>`), runs on one of GitHub's computers
what docs/releasing-beta.md asks of Linux in calls ("Checks before a release", 2), in one
job of about 40 minutes: a clean release build checked as `tools/beta/release.py` checks it
(`tools/beta/ci_builds.py check-build linux`), the video plugin's native tests, the call
test built as a profile and as a debug build, and then, in the call lab
(`tools/call-lab`, with `LOAD_MAX=9` and its own display `:172`), the microphone checks
`media devices alone late muted unplug switch broken hang noise term:1500` on the debug
build (`term`: a SIGTERM must make the copy leave the room in order and end by itself) and
`late` again with PipeWire's quantum left free (`LAB_FIXED_QUANTUM=0`, a part of its own),
`unmute:headset` and `unmute:speakers` (a part of their own, with ffmpeg making the room's
sound: the other side holds a tone while the microphone is turned off and on; with a headset
every word and the room's noise for 12.5 s after each unmute must go out within 6 dB of
the same unmute's last, and with speakers the tone must go back to it at least 20 dB under
the words, `tools/call-lab/unmute_check.py`; only a libwebrtc with WebRTC patch 0010
passes the headset part),
`rejoin` on the profile build, a shared screen's sound beside a loopback and a combined
output (`CALL_LAB_LOOPBACK=1`: a's log must say `passed on, not linked` and never link
`pipewire-pulse`), calls of 4 copies with a camera and with a shared screen for 60 s on the
debug build, and of 4 copies three times and 6 copies twice with a shared screen for 90 s
on the profile build. A call passes when every copy wrote a result, none says `failed` and
no copy's log says it was kept out. The run fails when any part failed or did not run, or
when the application crashed meanwhile (`coredumpctl`, the kernel log, a core file). The
run's summary has every part's result and the tables of `tools/call-lab/sum.py`; the run
folders, without the copies' data folders, are the run's artifact, kept 30 days. One job,
because every part needs the same three builds and the parts run one after another anyway:
each takes the computer's processors and a sound session to itself. Before the builds, a
recording in a sound session of the lab's must reach its virtual microphone, or the run
stops there. Two things of the lab's sound sessions keep them working on the PipeWire 1.0
and WirePlumber 0.4 of GitHub's Ubuntu 24.04 as on a newer desktop: the sound daemons have
a session bus of their own (`tools/call-lab/session.sh`; WirePlumber 0.4 ended at once
without one, so nothing was linked and no copy recorded or played anything, the x11 smoke
test's session included), and the graph runs at WebRTC's 10 ms from the start
(`tools/call-lab/lab.sh`, `lab_devices`; otherwise a lab microphone went silent for good
when the voice engine stopped its recording and started it again, as it did when the
microphone opened after someone connected, and `mic.sh late` failed every time). The
application no longer does that restart when the microphone opens with nothing changed
in the sound server since the call chose it (a source or sink that comes or goes in between
still has it restart once, as before); `LAB_FIXED_QUANTUM=0` leaves the quantum free, as
on a person's computer, and the part "mic late (quantum free)" checks it so.

GitHub's computers have 4 processors: the call of 4 copies with a camera takes them whole
(a load of about 18, each copy at some 93% of a processor, 20 to 22 pictures a second where
a shared screen keeps 30) and still passes; the calls with a shared screen, of 6 copies
too, stay near a load of 10.

## Dropping files on the Flatpak

`.github/workflows/flatpak-drop.yml`, started by hand (`gh workflow run flatpak-drop.yml
--ref <branch or tag>`), drags files onto Fogata running as a Flatpak with a real drag, in
about 45 minutes (docs/architecture/5d-desktop-application.md says what a drop does
there). It builds the commit's copy of `app/integration_test/drop_portal_test.dart`, a room's
drop zone that writes down what each drop handed over, took and said, and one of an
earlier commit's drop plugin and check of private places beside it (`-f before=<commit>`;
by default the last commit before the application took the FileTransfer portal; `-f
before=` for none), runs `test/private_drop_test.dart`, whose drops through the plugin's
channel only Linux runs, and makes both copies Flatpaks with the application's own
permissions (`tools/beta/flatpak.sh --unsigned --test-bundle`). Then, in a container of
Ubuntu 26.04 (GitHub's Ubuntu 24.04 has neither KDE Plasma 6 nor an xdg-desktop-portal with
the Documents portal's `GetHostPaths`, 1.19 and later), `tools/smoke-wayland/drop.sh` runs
the session of `tools/smoke-wayland/session.sh` with `/dev/fuse` for the document portal
and drags a GTK window outside the sandbox onto each copy with KWin's fake input: files
from the downloads folder, from another folder, from `~/.ssh`, a hidden one, the key
written as a page can write it (a fragment, a query, another host), two together, the same
from a source that offers only the list of files, one that is nowhere, two drags in a row,
the key from a source that offers no list, the key dragged over the copy and dropped on it
while the copy judges another drop (slowly, by `FOGATA_TEST_SLOW_JUDGE`), a source whose
portal transfer holds another number of files than its list, a link to the key and a
document of the portal named by a list. Each drop's line says what the copy took, the
folder it shows, its notice, the targets it asked for, whether the copy's plugin called
`RetrieveFiles` (by its own log) and, after the drops of the key, whether the portal holds
a document of it for the copy (`flatpak documents`). The container's image is pinned by its
digest; what apt and Flathub install in it is the newest.
The run fails when a drop of the commit's copy did not do what it should, or the earlier
copy did not do what that commit did; the run's summary has every line, and the session's
folders and logs are the run's artifact, kept 14 days. `drop.sh` needs what it names at its
start; it has only been run in that container.

## A call's memory over hours

`.github/workflows/call-memory.yml`, started by hand (`gh workflow run call-memory.yml -f
seconds=12600 -f mode=voice`), runs a call of the call lab for hours on one of GitHub's
computers, where a job may last six: copies of the call test, every copy's memory by kind of
mapping each minute (`tools/call-lab/smaps.py`, in the run's `smaps.log`), and, when asked
(`-f heap=b`), one copy under heaptrack (`MEASURE_HEAP` of `tools/call-lab/measure.sh`), from
whose notes `tools/call-lab/heap-report.sh` says what was allocated after the call settled
and never freed, by call stack. The whole run folder is the run's artifact, kept 30 days. It
checks nothing and blocks nothing: it is how a slow growth is looked for, before and after a
change. The same runs on any Linux computer with `heaptrack` and `zstd` installed.

It measures a release build unless told otherwise (`-f build=profile`), because a profile
build grows by itself while a call is on screen: the engine writes a timeline event with a
new id for every frame it is asked to draw (`Animator::RequestFrame`), and, by what
heaptrack recorded, a build that is not a release keeps a 16-byte record and a slot of a
table for each, for as long as the process lives.
At 60 frames a second that is about 17 MB an hour, in steps (the table doubles: 6 MB after
some 36 minutes, 12 after 65, 24 after two hours). In a call of two with voice measured for
three and a half hours side by side, the profile build's copies went from 552 to 578 MB and
from 516 to 551 MB, and the release build's stayed where they were (496 to 496 MB, 487 to
489 MB): a release build has no such record. The profiler's copy starts many times slower,
so the others wait longer for it (`FOGATA_TEST_PATIENCE` of the call test); with more than
two copies it can still miss the test's waits.

```sh
tools/ci.sh [--full] <commit>          # by default HEAD; --split app|checks: half of it
tools/install-hooks.sh        # once per clone: installs the pre-push hook
```

`tools/ci.sh` makes a clean temporary worktree of the commit (under `<git dir>/fogata-ci/`)
and runs there, in this order:

1. `flutter pub get --offline`;
2. a release build of the Linux application (`flutter build linux --release`), then the
   checks `tools/beta/release.py` makes of the bundle before it packs it
   (`tools/beta/ci_builds.py check-build linux`, see above);
3. the native tests of the WebRTC plugin's shared C++ code
   (`app/third_party/flutter_webrtc/common/cpp/test/run_tests.sh`): the video renderer's,
   built with ThreadSanitizer against the libwebrtc the build unpacked, and the noise
   suppression table's;
4. `tools/check.sh`, strict: formatting, clippy with warnings as errors, every test (with
   cargo-nextest, several test binaries at a time, those that start devices on the network
   at most two at once, `.config/nextest.toml`; the doc tests with `cargo test --doc`; the video
   players' check of a file's first bytes among them, on Linux built a second time with
   AddressSanitizer and UndefinedBehaviorSanitizer like the Linux player's other C++ tests,
   and on macOS the tests of its video loader, `app/packages/fogata_video/macos/test`), `cargo
   deny` (licences, bans, sources and the RustSec advisories, which fail when the computer
   has no advisory database, or one whose last advisory is more than 14 days old; a yanked
   crate version fails too, and warnings are printed, not hidden; with the advisories, also
   over `fuzz/Cargo.toml`, without its licences), the
   tests of the release and check scripts, of the commit marker (a change to a native source,
   or a new file there, makes the library say `+dirty`; a build of Flutter's own files does not
   make cargo compile again) and of the CMake file that bundles the library (it takes the one
   cargo built, whatever `CARGO_TARGET_DIR` says), `THIRD-PARTY-NOTICES` up to date, the bridge
   code generated again the same, the `[patch.crates-io]` of `fuzz/Cargo.toml` the same as the
   workspace's, the crates `fuzz/Cargo.lock` shares with `Cargo.lock` at versions `Cargo.lock`
   has (`tools/fuzz_lock.py`), the dependencies between the workspace's own crates only along
   the edges their layers allow (`tools/crate_layers.py`, docs/architecture.md), the app's
   limits of a message, a bio and the names the same as the Rust ones (`tools/limits.py`), the command-line program linted and tested also with its `lab` feature (the
   network lab's tools and test switches, which the program that ships does not have), and
   where unsafe code may be: every crate but `fogata-ffi`
   has `#![forbid(unsafe_code)]`, clippy denies an unsafe block without a SAFETY comment, and
   the lines naming `unsafe_code` and using `unsafe` in each file of `fogata-ffi` are counted
   against a list in `check.sh`, so a new block inside a function or module that already has
   an allowance changes a number. `--no-advisories`, `--no-codegen` and `--no-notices` skip
   those checks, for a computer that cannot run them, and are recorded as skipped;
5. the libFuzzer targets of `fuzz/` built (`cargo +nightly fuzz build`; they are not run
   here, `tools/fuzz.sh --libfuzzer` runs them), because they are a workspace of their own
   that no other step compiles;
6. `flutter gen-l10n`, then nothing in the worktree may differ from the commit (generated
   code, localizations, `fuzz/Cargo.lock`);
7. `dart format --set-exit-if-changed` over the app's own Dart code (`lib`, `test`,
   `integration_test`, `test_driver`, `packages/fogata_video`; not `third_party`), then
   `flutter analyze` and `flutter test`;
8. `tools/integration-tests.sh`: the integration tests that need one copy of the
   application and nothing else (`beta_test`, `chat_tools_test`, `layout_test`,
   `screens_test`, `settings_layout_test`, `video_test`), each with `flutter test -d linux`
   on an Xvfb display of its own (from `:300` up, never the desktop's), its sound to a null
   sink of its own (made with `pactl`, and taken away only once nothing plays into it), no
   session bus, and fresh folders. Without Xvfb, `pactl` or a sound server it is recorded
   as skipped, naming what is missing;
9. with `--full` only (it takes much longer): `tools/integration-pairs.sh`, the tests of two
   copies talking to each other, `call_test` (two copies enter a group call muted), `direct_test`,
   `voice_test` and `move_test` (each of its four scenarios), each built as its own debug
   bundle and run on an Xvfb display of its own with virtual sound devices only (a null sink
   to play into; for the voice test, a virtual microphone fed with a made-up recording), no
   session bus and fresh folders. Without `--full` they are recorded as `not run (--full not
   given)` and the result is `incomplete`, never `pass`: a commit that breaks a call, a
   direct chat, a voice message or an account move must not get through unnoticed. The hook
   lets such a record through only when `pairs` is named in `FOGATA_CI_ALLOW_SKIPPED`. Left to the test
   rigs: the screen and sound variants of `call_test` (a shared screen's sound needs a guard
   against a copy recording from a real source), `changes_test`, `away_test`,
   `with_friend_test`, a command-line peer or several computers, the macOS and Windows
   `screen_sound_test`, and `window_test`, whose maximizing and minimizing need a window
   manager;
10. with `--full` only: the smoke test, `tools/smoke-linux.sh` (see "The smoke test" below),
   on each Linux desktop this computer can build. A desktop it cannot build is skipped with
   the reason; when none can be built the smoke test is recorded as `skipped (...)` and does
   not change the result (it is optional, unlike the pair tests); a desktop that fails, or a
   copy that crashes, fails the run;
11. the legs for macOS and Windows: `FOGATA_CI_MAC` and `FOGATA_CI_WINDOWS`, when set, are
   shell commands run with `FOGATA_CI_SHA` (the full commit) in their environment, that get
   that commit to those systems however this computer reaches them (a `git bundle` or a push
   to a clone there), check it out, and run `tools/ci-leg.sh macos` or
   `tools/ci-leg.ps1` there (formatting, clippy with warnings as errors, every test,
   `cargo deny`, `THIRD-PARTY-NOTICES`, the tests of the release scripts, `flutter analyze`,
   `flutter test`, and on Windows the smoke test, `tools/smoke-windows.ps1`, as the step
   `smoke`; on macOS through `tools/check.sh --no-advisories --no-codegen`, which the
   first computer checks; on Windows step by step, each native command checked by its exit
   status, since PowerShell's own error handling does not cover them). A leg refuses a clone
   at another commit, with a change to any tracked file (`deny.toml`, `THIRD-PARTY-NOTICES`,
   `docs/`, `rust-toolchain.toml` too) or with a new file git does not ignore, and a `git
   status` that fails counts as a failure, not as a clean clone. The tests of the Unix
   scripts (`tools/test_ci.py`) and of the Linux CMake file only run on Linux and macOS. They live in the environment of the
   computer, never in the repository. A leg passes only when its command exits 0 **and** its
   output has the line the leg script prints at its end, `fogata-ci-leg: system=<name>
   commit=<commit> status=pass steps=<steps> skipped=none`, for this commit and this system:
   a command that exits 0 and says nothing (a bare `FOGATA_CI_MAC=true`) leaves the leg
   `incomplete`, and so does a leg that left steps out (`FOGATA_CI_LEG_SKIP`). A leg not set
   is `skipped`.

Every step runs without the variables that change what the compilers make (`RUSTFLAGS`,
`CARGO_ENCODED_RUSTFLAGS`, `RUSTC_WRAPPER`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`, `CMAKE`); the
record names those that were set and cleared.

It writes `<git dir>/fogata-ci/<commit>`: each leg's result; for each leg of another system the
command that ran, its exit status, the SHA-256 and line count of its output and the steps it
reports; `legs ran` and `legs not run` (with why), so a record never passes in silence;
whether each check ran (the advisories with the date of the RustSec database); the native,
the integration, the pair and the smoke tests on lines of their own (`pass`, `fail`, `skipped (<what is
missing>)`, `not run` after an earlier step failed, or `not run (--full not given)`), the tools that ran (`rustc -V`, `cargo -V`, `flutter --version`), the
SHA-256 of `tools/ci.sh` and of the commit's `tools/check.sh`, and `result: pass` only when
every leg ran and passed and nothing was skipped or left incomplete; `result: incomplete`
when something was skipped or left incomplete and nothing failed; `result: fail` otherwise. The whole output goes to `<commit>.log` beside it. `FOGATA_CI_SLOT` names a
command the heavy steps run under (one that waits for a free build slot on a shared
computer, say); `CARGO_BUILD_JOBS` passes through. Build outputs are kept between runs in
`<git dir>/fogata-ci/target` and the libwebrtc download in `<git dir>/fogata-ci/downloads`;
`FOGATA_CI_FRESH=1` builds without them. Cargo never forgets an old build, so a folder of
them (`target`, `fuzz-target`) past `FOGATA_CI_CACHE_GB` (40 by default) is emptied before
the run, which then builds afresh and says so in its log.

The pre-push hook (`tools/hooks/pre-push`) refuses to push a branch or tag whose commit has
no record of `result: pass` written by the commit's own `tools/ci.sh`. A commit whose run
skipped something goes through only when each part skipped is named, after it was checked
some other way:

```sh
FOGATA_CI_ALLOW_SKIPPED="macos windows" git push
```

(`integration` names the integration tests, `pairs` the pair tests (not run without `--full`
as much as skipped), and `macos` or `windows` a leg that came out `incomplete` as well as one
skipped.) A record of half a run (`--split`) is refused whatever is named: run it whole.

The record is a plain file any program of the user can write, and a leg is a command the user
chooses: this is a barrier against oversights (a leg forgotten, a check skipped without anyone
saying so, a commit pushed that was not the one checked), not against someone who means to get
past it, who can write the record, write the line a leg prints, or use `--no-verify`. The line
`fogata-ci-leg: ... status=pass` in particular says nothing that an `echo` cannot say: the
script tests do exactly that (`FOGATA_CI_MAC='echo "fogata-ci-leg: system=macos commit=$FOGATA_CI_SHA
status=pass ..."'` is recorded as a pass), and the digest of the output only shows that the
output was not changed afterwards. Nothing in a plain file on the user's own computer can do
better, so none of it is meant to; a leg that must be trusted is one a maintainer ran and read. It
proves nothing to a third person. What it does give is a trail: the command, its exit status
and a digest of its output are in the record, and the output is in the log beside it. `git push --no-verify` skips the hook: only when the
commit cannot be checked here.

## Every week

`.github/workflows/weekly.yml` runs on Mondays (and by hand, from the Actions tab), with a
token that can only read; a failure there is for the maintainers to look at, it blocks no push:

- `advisories`: `cargo deny check advisories` online over `Cargo.lock` and
  `fuzz/Cargo.lock` (a fresh RustSec database, yanked versions read from crates.io), so a new
  advisory against a crate already locked shows even in a week with no push.
- `fuzz`: every libFuzzer target (`tools/fuzz.sh --libfuzzer`, 8 minutes each by default,
  four at a time: about 50 minutes, plus the build), with the dated nightly of `ci.yml`. The
  corpus is kept from week to week in the Actions cache (written only by this workflow's
  runs); findings and the logs are kept as the run's `fuzz-findings` artifact. A finding fails
  the job: replay it as docs/fuzzing.md says.
- `previous-release`: the command line built at the last release tag before the commit (or
  the tag given), against this commit's core:
  `crates/fogata-core/tests/previous_release.rs` (what this version writes to a group, the
  last release takes in whole, and both meet in its voice room). The tests of
  `crates/fogata-core/tests/old_copy.rs` are not run there: each needs a copy from before a
  given feature (banners, MLS, picture records...), older than every published tag, so they
  run only where such a build is at hand. `tools/previous-release.sh` runs the same test on
  your computer and records it; `tools/beta/release.py` takes a release only of a commit
  for which this job, or that record, passed (docs/releasing-beta.md, step 1).
- `vendor`: `tools/vendor_diff.py`, each vendored crate against the one published on
  crates.io at the version it records, and each forked Dart package of `app/third_party`
  (flutter_webrtc, tray_manager, desktop_drop) against pub.dev at the version its
  `pubspec.yaml` records: the files changed, and the files of the package's code left out
  (for a Dart plugin, its native code for Linux, Windows and macOS too, and flutter_webrtc's
  `third_party/`), which `FOGATA.md` must name, as a whole (a file left out by its whole
  path), and whether a newer version is out (a Dart fork behind also shows as a warning
  on the run). A fork whose
  `FOGATA.md` has a line "Reviewed against <name> <version>" was read against that newer
  version, what it brings taken or left out there: while that is the newest, the fork shows
  as reviewed, without a warning. The whole diff is the `vendor-diff` artifact. Report only.
- `webrtc`: `tools/webrtc/check_stable.py`, `tools/webrtc/pins.sh` against the milestone on
  Chrome's Stable channel (chromiumdash): a warning one milestone behind or with newer merges
  on the same branch-head, a failure two milestones behind.
- `dates`: `tools/beta/deadlines.py --online`, the dates after which something of the
  pre-releases breaks (the newest release's expiry, read from its `release.json` on GitHub;
  the Mac signing certificate's, kept in the script): a warning 75 days before, a failure 30
  days before (docs/releasing-beta.md, "Dates that break things").

Dependencies are updated by hand, not by pull requests a bot opens. The repository's
Dependabot alerts name the crates and packages with a known vulnerability, and the weekly
`advisories` job checks the crates against RustSec; secret scanning with push protection and
private vulnerability reporting are on too. These are settings of the repository, not files
in it.

## The smoke test

The smoke test is the main things a person does, done by two copies of the application the
way a person does them, on each system and each kind of desktop people run it in:
`app/integration_test/smoke_test.dart`. Ana and Bea make profiles, become friends, write to
each other in a direct chat, make a group, enter its voice room (microphone off), turn the
camera on (a made-up one on Linux) and see each other's, then Ana shares her screen **with the
room's button**, as a click does: first she presses it twice in a row and cancels what opens,
then she presses it again and shares (where the list is already open when the second press
comes, that press closes it, as a double click does; with one screen and no window there is
no list and the button shares at once); Bea watches the screen and frames must keep coming. Ana
sends Bea a file, Bea downloads it, and both leave. A copy that ends by itself is named as a
crash, with its signal or exception code.

The screen goes through each system's own path: the app's list of screens and windows (filled
by libwebrtc's desktop media list) on X11, Windows and macOS, and under Wayland the desktop
portal's dialog (xdg-desktop-portal, PipeWire).

| System | Script | Desktop | Unattended |
| --- | --- | --- | --- |
| Linux | `tools/smoke-linux.sh [wayland] [xwayland] [bare] [x11]` | see below | yes, in `tools/ci.sh --full` |
| Windows | `tools/smoke-windows.ps1 [-NoBuild] [-Out <folder>]` | the computer's own | on a test machine with virtual sound devices only, in its CI leg |
| macOS | `tools/smoke-macos.sh build`, then `run` | the computer's own | no: see below |

On Linux, each desktop is one of its own; nothing reaches the computer's screen, sound,
camera, session bus or portal:

- `wayland`: `tools/smoke-wayland/session.sh` builds a KDE Plasma Wayland session in
  unprivileged user, mount and PID namespaces (as `tools/call-lab/session.sh` does for sound):
  a `/dev` with no sound, camera or display device (only the GPU's render nodes, which cannot
  show anything on a screen: KWin shares a screen only when it composites with OpenGL, and on
  a virtual output only on a render node), the user's runtime folder, the system bus and the X
  sockets hidden, a home folder of its own, its own `dbus-daemon` session bus (with the
  system's service files, so the portal starts on demand as in Plasma), PipeWire, WirePlumber,
  pipewire-pulse, KDE's application index (`kbuildsycoca6`, which KWin reads to let the
  portal use its screen casting), `kwin_wayland --virtual --xwayland` and a notification
  server that shows nothing (`notifications.py`: the portal tells of every share through
  one, and with none the bus tried to start one for 25 s each time; two shares in thirty
  timed out then, none since). The copies start with the environment Plasma gives an application. The portal's dialog is answered by
  `tools/smoke-wayland/picker.py`, which watches the accessibility bus for it and presses
  the screen's button (Share) or closes the dialog's window through KWin (Cancel), as the
  test asks in `$SMOKE_PICKER/answer`; before the copies run, `portal_check.py` shares the
  screen once through the portal, and the session is skipped if it cannot.
- `xwayland`: the same session, with the copies on X11 through Xwayland (as with
  `GDK_BACKEND=x11` under Plasma): KWin manages their windows, so the system lists the
  screen and the windows and the app shows its own list.
- `bare`: the same session, with the copies started as a service or a remote shell starts
  them there: only `WAYLAND_DISPLAY` and the session bus, no `XDG_SESSION_TYPE` or `DISPLAY`.
  GTK still draws on Wayland there, so the screen is still picked in the portal's dialog;
  the test fails, named, if the app would take its own list instead (which would be empty).
- `x11`: Xvfb inside `tools/call-lab/session.sh`, no session bus, as the other integration
  tests. With no window manager the system lists only the screen, which the button shares at
  once: there is no list to cancel there.

It needs `unshare` with unprivileged user namespaces, `kwin_wayland`, `Xwayland`,
`dbus-daemon`, PipeWire, WirePlumber, `xdg-desktop-portal` and `xdg-desktop-portal-kde`,
`kbuildsycoca6`, `python3` with `dbus` and `gi` (Atspi), a GPU render node, and `Xvfb`; a
desktop missing one of them is skipped, naming it.

On macOS the copies are an application of their own, `Fogata Smoke.app`
(`dev.holypanda.fogata.smoke`, never the person's `dev.holypanda.fogata`), signed for that
Mac only, each copy with its own data folder. The first run of each build needs a person:
macOS asks to let it record the screen and maybe to accept incoming connections, and the two
windows show on the screen while it runs. Mute the Mac's sound first.

### A still shared screen under Wayland

Under Wayland the portal's capture (PipeWire) gives a picture only when something on the
shared screen changes, and a screen is sent to someone only once they open it: the first
picture they get has to be made from what the capture already had (the plugin keeps its
last frame and gives it again when someone starts watching: the fork's "The last frame of
a shared screen again"). `tools/smoke-wayland/still.sh [calls] [opens] [still|moved|both]`
checks it in the same KDE Plasma Wayland session as the smoke test's `wayland` desktop,
with two virtual outputs (`SMOKE_OUTPUTS=2`): copy a shares the first one through the
portal's dialog, both copies' windows are moved to the second as they appear, and nothing
on the shared one moves (KWin draws no panel, wallpaper or clock there, and the pointer
stays where it is). Copy b opens the screen only once PipeWire has given copy a no buffer
for 5 s, which the plugin's own log proves (`FOGATA_WEBRTC_LOG`: each buffer and each
frame given again, with its time), and logs how long the first picture took, decoded and
shown (the spinner gone). Before an open the screen has stayed still since the share
started (`still`), or a window was moved onto it and back first (`moved`); with
`both` (the default) they take turns. A picture that does not come in 20 s is waited for
30 s more after a change on the screen, so the trial says whether it ever comes without
one. By default, 30 calls each made anew with one open (the first open of a share);
`still.sh 1 30` opens and closes the screen 30 times in one call instead. It passes when
every picture came within 3 s with no buffer from PipeWire in between; 30 calls take
about 15 minutes. Built with the plugin's `Refresh` giving nothing, every open waited the 20 s,
and its picture came only with the change: the trial tells.

Like the smoke test's KDE desktops it needs a GPU's render node (KWin shares a screen
only with one), so it cannot run on GitHub's computers: it runs on a Linux computer with
a GPU, and is skipped where the session cannot be built. `STILL_WINDOWS=shared` leaves the
windows on the shared output, as on a desktop with one screen, to see what reaches the
capture then.
