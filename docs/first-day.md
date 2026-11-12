# First day

A map for someone new to the code: what to read, what to run, and the rules that are easy to
break without noticing. Everything here is said at length elsewhere; this page only points.

## Read

1. `docs/architecture.md`, sections 1 (the concepts: groups, devices, friends, rooms) and 2
   (the folders and the layers of the crates). The rest is long (some 4,800 lines) and is
   read by section, when a change touches it: it is the specification of what Fogata does,
   kept true as the code changes.
2. The comments of the code you change: they say why, and many state a rule the rest of
   the code relies on.
3. For a forked package, its `FOGATA.md`: `vendor/<crate>/FOGATA.md`,
   `app/third_party/<package>/FOGATA.md`. Every change from the published package is listed
   there, and the weekly workflow reports any that is not (`tools/vendor_diff.py`).

## Build and run

`docs/building.md`: the Rust of `rust-toolchain.toml` (rustup takes it), the exact Flutter of
`app/pubspec.yaml`, and on Linux the development packages it names. The WebRTC library
comes as archives built by `tools/webrtc` and published in holypandadev/fogata-libwebrtc,
which the build downloads and checks by SHA-256 (`docs/native-components.md`); you do not
build it. `docs/cli.md` is the command-line program, which runs the same core as the app and
is the quickest way to try the network.

## Before a commit, before a push

- `tools/check.sh`: formatting, clippy, every Rust test, `cargo deny`, the tools' own tests,
  the generated bridge code and notices. Run it before every commit.
- `tools/install-hooks.sh` once, then `tools/ci.sh --full` before a push: the whole run of
  `docs/ci.md` (the release build, the native, integration, pair and smoke tests) in a
  clean worktree. GitHub runs the same on Linux, macOS and Windows at every push.

## Rules that are easy to break

- **Compatibility.** Copies of the last releases must keep agreeing with yours. What a device
  sends, accepts and keeps is pinned by the golden fixtures (`crates/*/tests/data`,
  `crates/fogata-proto/tests/frozen`), which never change; a change to what a device
  accepts from others goes through a capability level (architecture.md 3.15 and 5.1). The
  test against the last release (`tools/previous-release.sh`) must pass before a release.
- **Layers.** A crate uses only crates of the layers below it (`tools/crate_layers.py`).
- **Unsafe code**: of the workspace's crates, only `fogata-ffi` may have it, and
  `tools/check.sh` counts it per file.
- **Text people read** goes into `app/lib/l10n/app_en.arb` and every other language
  (`docs/translating.md`).
- **The bridge** (`crates/fogata-ffi/src/api`) is generated with flutter_rust_bridge_codegen
  2.13.0 exactly; commit what it writes.

## Where other devices' input comes in

The code that reads what other devices send, and so decides what a stranger, a friend or a
member may do. Each is a large `match` over message kinds: a new kind needs its own check of
who may ask, at the top of its arm, and a fuzz target if it decodes something new
(`docs/fuzzing.md`).

| Where | What |
|---|---|
| `crates/fogata-net/src/node/members.rs` (`on_request`, `on_response`) | Requests between members of a group |
| `crates/fogata-net/src/node/friends.rs` (`on_friend_request`) | Requests between friends and from strangers |
| `crates/fogata-core/src/node_events.rs` (`handle`) | What the node reports, turned into the app's state |
| `crates/fogata-state/src/state.rs` (`check_and_apply_at_two`) | Whether a group log entry is taken |
| `crates/fogata-core/src/direct/fetch.rs`, `crates/fogata-core/src/transfer/` | Files of direct chats, and an account moved |

## Where the tests are

| Kind | Where | Run by |
|---|---|---|
| Rust unit tests | beside the code (`#[cfg(test)]`) | `cargo test -p <crate>`, `tools/check.sh` |
| Rust integration and golden tests | `crates/*/tests` | the same |
| Two releases together | `crates/fogata-core/tests/previous_release.rs` | `tools/previous-release.sh`, weekly |
| Network scenarios through NATs | `tools/netlab` | by hand, Linux |
| Flutter widget and unit tests | `app/test` | `flutter test` |
| The app driven on a desktop | `app/integration_test` | `tools/integration-tests.sh`, `tools/integration-pairs.sh` |
| Native code of the WebRTC plugin | `app/third_party/flutter_webrtc/common/cpp/test` | its `run_tests.sh`, in `tools/ci.sh` |
| The tools and release scripts | `tools/test_*.py`, `tools/beta/test_*.py` | `tools/check.sh` |
| Fuzzing | `fuzz/fuzz_targets` | built by `tools/ci.sh`, run weekly (`tools/fuzz.sh`) |

Known gaps: `fogata-ffi` (the bridge) is tested only through the app; `fogata-voice`,
`fogata-denoise` and `fogata-crypto` have unit tests only.
