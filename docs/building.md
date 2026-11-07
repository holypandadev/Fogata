# Building Fogata

## The command-line program

Requires Rust 1.91 or newer. `rust-toolchain.toml` names the Rust the application and its
releases are built with; rustup installs it the first time cargo runs here.

```
cargo build --release -p fogata
```

The binary is `target/release/fogata`.

## The desktop application

Requires Flutter 3.47 or newer and, on Linux, the GTK 3 development files, clang, cmake and
ninja. On macOS a release build is universal (Apple silicon and Intel), so the Rust that
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
CLI does; `FOGATA_DATA_DIR` chooses another folder (and `FOGATA_DOWNLOADS` where downloads
go). After changing `crates/fogata-ffi/src/api/`, run `flutter_rust_bridge_codegen
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
