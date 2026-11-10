# Contributing to Fogata

Thank you for wanting to help. Fogata is a small project with one maintainer, so a short note
before a large change (an issue saying what and why) saves work on both sides.

## Before you start

- Read [docs/architecture.md](docs/architecture.md), at least its "Start here" and section 2:
  where the code is, and which crate may use which.
- Build it as [docs/building.md](docs/building.md) says.
- A security problem goes to [SECURITY.md](SECURITY.md), not to an issue.

## A change

- **Versions must keep talking to each other.** Copies of different versions are in the same
  groups. A change to what a device accepts from others (group log entries, room events, wire
  messages) can split a group between those who updated and those who did not: such a change
  goes through the capability levels (docs/architecture.md, 3.15 and 5.1), and
  `crates/fogata-core/tests/previous_release.rs` checks it against the last release.
- **Tests.** A fix comes with a test that fails without it; a feature with tests of what it
  promises. Rust tests sit beside the code they test, as the existing ones do; Flutter widget
  tests under `app/test`.
- **Style.** English everywhere. Comments say why, not what. Keep the names and the voice of
  the code around the change. No new dependency without a reason given in the change
  (`deny.toml` says which licences can come in).
- **Unsafe code** only in `fogata-ffi`, each block with a `SAFETY` comment, and counted in
  `tools/check.sh`.
- **Translations**: [docs/translating.md](docs/translating.md).

## The checks

`tools/check.sh` must pass before every commit (formatting, clippy with warnings as errors,
every test, `cargo deny`, the checks of the scripts), and `tools/ci.sh` before a push
(`tools/install-hooks.sh` installs the hook that asks for it); [docs/ci.md](docs/ci.md) says
what each runs. Format the Dart code with `dart format`. GitHub runs the same checks on every
pull request.

## Licence

Fogata is under the [GNU Affero General Public License, version 3](LICENSE) only. By sending a
change you agree that it is released under that licence.
