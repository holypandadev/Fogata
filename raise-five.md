Branch: lab/raise-five at 7762c53, on main a67856f. No merges.

Before: a manager's device raises its groups to level four at most, because members could not follow a group that moved and the app did not offer the move. So no group gets the log ceiling, and two devices can still grow a group's log until nobody can join it.

After: a manager's device raises its groups to five like any other level: once every member's copy is seen judging at five, or anyway after the wait, when its person agrees to leave the others behind. At five a group's log has a ceiling, the app warns as it fills, and the first member can move the group to a fresh log, which the members follow.

## How

- `Level::RAISED_TO` is `Level::Five` (`fogata-state`, `state.rs`).
- `docs/architecture/5-group-management.md` now says how a group gets to five.
- Lab: `group-move` again has the first member remove both devices that filled the log while the others are away. That needed members to be found after two key changes, which main now has.

## Proof

- fogata-net `group.rs`: `a_group_goes_up_to_five_once_every_member_judges_at_five` (a group at four goes to five once every member judges at five, and stays at four while they judge at four). Fails without the change.
- `cargo fmt --check` and `cargo clippy -D warnings` on fogata-state, fogata-net, fogata-core and the CLI (all targets): clean.
- fogata-state, fogata-net, fogata-core and the CLI: 1,170 passed, none failed.
- Network lab, run locally: `group-move` passes with both removals. B and C follow the move, all three hold the same new log of 1,138 bytes against 613 KB in the old one, and the messages after the move reach the others.

CI on this commit: CI (Linux, macOS, Windows and the Linux checks), call checks, the whole network lab (with `group-move` removing both), the Flatpak drop and the lab checks all pass.
