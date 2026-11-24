Branch: lab/lean-offers at 6503a73, on main 51913bfa. Needs no other branch. No merges.

Before: offering a group to a friend, or letting a device in, sends every entry of the log, spare ones included (entries past their author's run or the ceiling, and entries of kinds nothing applies). A log that a flood filled with spare entries can grow past the 4,096-entry and 4 MiB limit of an offer, and then the group can no longer be offered or joined.

After: an offer and a join carry only what the new device needs to judge the group the same way: the needed entries and their ancestors, and every `Level` entry that names a level, because a judged `Level` orders the log even when it is spare. Entries waiting on a missing parent come with the next sync, as they always did. A sync between members is unchanged.

## How

- `GroupLog::hashes_to_offer` (`fogata-state`, `log.rs`): walks the order from its end, keeps needed entries, `Level` entries that name a level, and every parent they name, and returns them in the order.
- `encoded_parts` (`fogata-net`, `group.rs`): encodes `hashes_to_offer()` instead of every entry.
- Docs: `docs/architecture/5-group-management.md`, what a new device is sent.

## Older releases

An older device receiving a trimmed offer judges the same group: what is left out applies nothing on any copy, and every ancestor of what is sent is sent. The wire format does not change.

## Proof

- fogata-state: `a_raise_past_its_run_is_offered_with_the_log`. A raise to level 3 written past its author's run is spare but still orders the log; without the `Level` rule the device offered the log judges it differently. Fails without the change.
- fogata-net: `a_device_new_to_the_log_gets_only_what_it_needs` (an entry of unknown kind, a waiting entry and a later rename: the device gets only the needed ones and judges the group as the sender does). The existing parts test now also checks that the offer is exactly `hashes_to_offer()`. Both fail without the change.

Local runs on this commit:

- `cargo fmt --check` and `cargo clippy -D warnings` (fogata-state, fogata-net, fogata-core, all targets): clean.
- fogata-state: 210 passed. fogata-net: 342 passed and every integration test file passes. fogata-core: 323 passed, and every integration test file but two passes.
- The only local failures fail the same way on main on the test machine, because they need a folder to be read-only and root ignores that: `files::tests::what_an_earlier_run_left_and_cannot_go_is_left`, `transfer::tests::a_move_that_cannot_be_committed_stays_staged`, `tests/readmitted.rs` and `tests/sealed.rs`. All of these run in CI.

CI on this commit: CI (Linux, macOS, Windows and the Linux checks), call checks, the network lab and the Flatpak drop all pass. The macOS job was not started once for lack of a GitHub runner and passed when run again.
