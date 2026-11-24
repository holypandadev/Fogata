Branch: lab/move-lab at 15245b2, on top of lab/level-five-corpus (71598bc). No merges.

Before: nothing outside the tests can move a group, the network lab never sees a move, and the app has nothing to show how full a group's log is or who may move it.

After: the command line moves a group (`/move` in `fogata chat`), and a chat open on a group that moved goes on in its new log, in its room too. The network lab has `group-move`: two members fill a level-five log to its ceiling, the first member removes one of them and moves the group, and the others follow it across their NATs and talk in the new log. Each group's view says how full its log is and whether this device may move it.

## How

- Core: `Core::moved_to(group)`, the group that shows in place of one that moved, once its new log runs here. `GroupView::fill` (from level five) and `GroupView::may_move`.
- Network: `GroupSummary::fill`.
- Command line: `/move`; following a move on `GroupsChanged`; lab-only `lab-level` (raises a group here to a level at once) and `lab-flood-log` (two devices write renames in turn, each on the other's last, until the log refuses one).
- Lab: `group-move` in `tools/netlab/netlab.sh`, in the default list, timed in `shards.py`. It passes only when A moved, B and C followed, the new log on each is under 64 KiB, and the messages said after the move reach the others.
- Docs: `docs/cli.md`.

## Proof

- fogata-core `tests/moved.rs`: `a_group_shows_its_fill_and_who_may_move_it` (fill from level five only, may_move only for the first member); `moved_to` checked in the kept-before-a-stop and taken-in-the-same-run tests.
- `lab-flood-log` checked offline: 3,579 entries written before the log refused one, which put it at the 3,584-entry ceiling. That is 612 KB counted and a 627 KB log file.
- `cargo fmt --check` and `cargo clippy -D warnings` on fogata-net, fogata-core, fogata-ffi and the CLI, with and without `lab`: clean. CLI tests pass; fogata-core `tests/moved.rs` 8 passed.
- Network lab, run locally on 15245b2's script: `group-move` passes. B and C follow the move about 100 s in, all three hold the same new log of 1,465 bytes, and the messages after the move cross.

## Known limit, measured

When the first member removes two members while offline, it writes two key changes, and the others, knowing only the key from before both, never reached it behind cone NATs. The scenario removes one. `lab/behind-rotations` fixes this on its own (members announce under the older keys they look up); once both are in, the scenario can remove both flooders again.
