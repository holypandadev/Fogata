# Building Fogata

## The command-line program

`rust-toolchain.toml` names the Rust the application and its releases are built with
(1.99.1); rustup installs it the first time cargo runs here. Only that Rust is tested: the
workspace's `rust-version` (1.91, in `Cargo.toml`) is the oldest no crate in `Cargo.lock`
asks more than, not a Rust anything builds with.

```
cargo build --release -p fogata
```

The binary is `target/release/fogata`.

## The desktop application

Requires the exact Flutter the workflows build with (`FLUTTER_VERSION` in
`.github/workflows`, now 3.47.8): `app/pubspec.yaml` names it under `environment:`, and
`flutter pub get` refuses any other, since another Flutter builds another engine into the
application. A new Flutter is taken by changing both together (`tools/test_repo_checks.py`
checks that they agree). On Linux it needs clang, cmake, ninja and the
development files of GTK 3, PulseAudio, PipeWire, ALSA and liblzma (on Ubuntu:
`libgtk-3-dev libpulse-dev libpipewire-0.3-dev libasound2-dev liblzma-dev`). A release or
profile build stops without PulseAudio's or PipeWire's: it would have no watch on the sound
server, or no screen capture under Wayland, no screen sound and no silent input. On macOS a release build is universal (Apple silicon and Intel), so the Rust that
`rust-toolchain.toml` names (1.99.1) needs both Mac targets, added once: its `minimal`
profile brings only the Mac's own.

```
rustup target add aarch64-apple-darwin x86_64-apple-darwin --toolchain 1.99.1
```

Without them the build stops with "the Rust target x86_64-apple-darwin is not installed".

```
cd app
flutter build linux --release
```

The application is `app/build/linux/x64/release/bundle/fogata`. It keeps its data where the
CLI does; `FOGATA_DATA_DIR` chooses another folder (and, in a debug build, `FOGATA_DOWNLOADS`
where downloads go). After changing `crates/fogata-ffi/src/api/`, run `flutter_rust_bridge_codegen
generate` in `app/`.

Walk through every screen with a fresh folder:

```
FOGATA_DATA_DIR=/tmp/fogata-a FOGATA_SHOTS=/tmp/shots flutter test integration_test/screens_test.dart -d linux
```

`integration_test/with_friend_test.dart` does a whole session with another device; see the
comment at its top (`tool/peer.sh` runs the other device; on Windows, `tool/peer.ps1`).

On macOS run `flutter clean` after changing the bridge plugin's podspec, so CocoaPods applies
it. After `flutter test` the debug build holds the test's entry point: build again before
opening the app by hand.

The widget tests (`flutter test` in `app/`) see Linux whichever computer runs them
(`test/flutter_test_config.dart` pins the system the interface follows), so they measure the
same text and title bar, and give the same result, on Linux, macOS and Windows.

## Checks before a commit

```
tools/check.sh
```

runs before every commit: formatting, lints (warnings are errors), tests, `cargo-deny` (the
licence policy, crates from crates.io only, no wildcard versions, and known vulnerabilities
from the RustSec database, fetched by hand with `cargo deny fetch db` so the checks stay
offline; it fails when the database is missing or more than 14 days old, unless
`--no-advisories` skips that check), the tests of the release and check scripts,
`THIRD-PARTY-NOTICES` and the bridge code up to date. Native components no Rust check sees are
listed in `docs/native-components.md`.

Before a push, `tools/ci.sh` runs those checks and the Flutter ones on a clean copy of the
commit, builds the Linux application, runs the native tests and the integration tests that
need no second copy (`tools/integration-tests.sh`, each on a display and a null sound sink of
its own), runs the macOS and Windows legs when they are set up, and records the result; the pre-push hook (`tools/install-hooks.sh` installs it) refuses a
commit without a passing record. See `docs/ci.md`.

## Network lab

```
tools/netlab/netlab.sh
```

simulates home and carrier NATs in unprivileged Linux network namespaces and runs the
connection scenarios. Needs `unshare`, `ip`, `nft` and `python3`; no root. It builds the CLI
with its `lab` feature (every `lab-*` command and `--lab-*` flag: the lab's attack tools and
test switches, which the program that ships does not have) into `target/netlab`, apart from the
release build.
