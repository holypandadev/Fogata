# Checks before a push

Every commit pushed is checked first on the computer that pushes it. GitHub runs the same
checks again on its own computers (`.github/workflows/ci.yml`), for every push to `main`, every
pull request and by hand: on Linux `tools/ci.sh --full`, whose macOS and Windows legs are the
workflow's other two jobs, `tools/ci-leg.sh macos` and `tools/ci-leg.ps1` (without the smoke
test, which needs virtual sound devices), side by side.

```sh
tools/ci.sh [--full] <commit>          # by default HEAD
tools/install-hooks.sh        # once per clone: installs the pre-push hook
```

`tools/ci.sh` makes a clean temporary worktree of the commit (under `<git dir>/fogata-ci/`)
and runs there, in this order:

1. `flutter pub get --offline`;
2. a release build of the Linux application (`flutter build linux --release`);
3. the native tests of the WebRTC plugin's shared C++ code
   (`app/third_party/flutter_webrtc/common/cpp/test/run_tests.sh`): the video renderer's,
   built with ThreadSanitizer against the libwebrtc the build unpacked, and the noise
   suppression table's;
4. `tools/check.sh`, strict: formatting, clippy with warnings as errors, every test (the video
   players' check of a file's first bytes among them, and on macOS the tests of its video
   loader, `app/packages/fogata_video/macos/test`), `cargo
   deny` (licences, bans, sources and the RustSec advisories, which fail when the computer
   has no advisory database, or one whose last advisory is more than 14 days old), the
   tests of the release and check scripts, of the commit marker (a change to a native source,
   or a new file there, makes the library say `+dirty`; a build of Flutter's own files does not
   make cargo compile again) and of the CMake file that bundles the library (it takes the one
   cargo built, whatever `CARGO_TARGET_DIR` says), `THIRD-PARTY-NOTICES` up to date, the bridge
   code generated again the same, the `[patch.crates-io]` of `fuzz/Cargo.toml` the same as the
   workspace's, the command-line program linted and tested also with its `lab` feature (the
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
7. `flutter analyze` and `flutter test`;
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
skipped.)

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
  portal use its screen casting) and `kwin_wayland --virtual --xwayland`. The copies start
  with the environment Plasma gives an application. The portal's dialog is answered by
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
